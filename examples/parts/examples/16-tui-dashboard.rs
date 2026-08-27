//! 16 · `mammoth top` — htop for your cluster.
//!
//!     cargo run -q -p mammoth-parts --example 16-tui-dashboard          # q quits, tab switches
//!     cargo run -q -p mammoth-parts --example 16-tui-dashboard -- --check
//!
//! Example 15 was the mechanics. This is the shape of the real screen: a
//! summary strip, a table of workers with inline gauges and sparklines, a
//! replication-health panel, and a footer of keys. All of it fake data, all of
//! it the real layout.
//!
//! ## The one architectural rule
//!
//! **State, then draw. Never draw and fetch in the same breath.**
//!
//!     struct App { ...plain data... }      ← what is true right now
//!     fn tick(&mut self)                   ← the only thing that changes it
//!     fn draw(frame, &App)                 ← reads it; changes nothing
//!
//! Ratatui redraws the whole screen on every frame. If `draw` did I/O you would
//! be hitting the cluster ten times a second, and a slow master would freeze
//! the UI. Keep `draw` pure and the worst a slow backend can do is show you
//! slightly old numbers.

use std::collections::VecDeque;
use std::io;
use std::time::Duration;

use clap::Parser;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Cell, Gauge, Paragraph, Row, Sparkline, Table, Tabs,
};
use ratatui::{Frame, Terminal};

// ═════════════════════════════════════════════════════════════════════════════
// The palette. Identical meanings to example 11 — one product, one language.
// ═════════════════════════════════════════════════════════════════════════════

mod tone {
    use ratatui::style::Color;

    pub const OK: Color = Color::Green;
    pub const WARN: Color = Color::Yellow;
    pub const CRITICAL: Color = Color::Red;
    pub const ACCENT: Color = Color::Cyan;
    pub const MUTED: Color = Color::DarkGray;

    /// One place decides what "nearly full" means.
    pub fn for_fill(f: f64) -> Color {
        match f {
            f if f >= 0.90 => CRITICAL,
            f if f >= 0.75 => WARN,
            _ => OK,
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// State
// ═════════════════════════════════════════════════════════════════════════════

#[derive(Clone, Copy, PartialEq)]
enum NodeState {
    Healthy,
    Warn,
    Dead,
}

impl NodeState {
    fn colour(self) -> Color {
        match self {
            NodeState::Healthy => tone::OK,
            NodeState::Warn => tone::WARN,
            NodeState::Dead => tone::CRITICAL,
        }
    }
    /// Colour is never the only channel, in a TUI either.
    fn symbol(self) -> &'static str {
        match self {
            NodeState::Healthy => "●",
            NodeState::Warn => "◐",
            NodeState::Dead => "✕",
        }
    }
}

struct Node {
    id: &'static str,
    rack: &'static str,
    state: NodeState,
    used: u64,
    capacity: u64,
    blocks: u64,
    /// A rolling window, newest last. `VecDeque` because you push one end and
    /// pop the other every tick.
    throughput: VecDeque<u64>,
}

impl Node {
    fn fill(&self) -> f64 {
        if self.capacity == 0 {
            0.0
        } else {
            self.used as f64 / self.capacity as f64
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Nodes,
    Health,
}

struct App {
    tick: u64,
    tab: Tab,
    selected: usize,
    nodes: Vec<Node>,
    quit: bool,
}

impl App {
    fn new() -> Self {
        let gb = 1024 * 1024 * 1024;
        let node = |id, rack, state, used_gb: u64, blocks| Node {
            id,
            rack,
            state,
            used: used_gb * gb,
            capacity: 160 * gb,
            blocks,
            throughput: (0..24).map(|i| 40 + (i * 37) % 260).collect(),
        };
        Self {
            tick: 0,
            tab: Tab::Nodes,
            selected: 0,
            nodes: vec![
                node("w1", "/dc1/rack-a", NodeState::Healthy, 114, 891_204),
                node("w2", "/dc1/rack-a", NodeState::Healthy, 93, 742_910),
                node("w3", "/dc1/rack-b", NodeState::Warn, 151, 1_204_776),
                node("w4", "/dc1/rack-b", NodeState::Healthy, 128, 1_002_311),
                node("w5", "/dc1/rack-c", NodeState::Healthy, 54, 431_008),
                node("w6", "/dc1/rack-c", NodeState::Dead, 0, 0),
            ],
            quit: false,
        }
    }

    /// The only place state changes. In the real `mammoth top` this is where
    /// the newest `cluster_report()` is folded in — on a background task, so a
    /// slow master never blocks a frame.
    fn tick(&mut self) {
        self.tick += 1;
        for (i, n) in self.nodes.iter_mut().enumerate() {
            // A dead worker serves nothing. Say so with a flat line rather than
            // leaving stale numbers on screen — a dashboard that lies about a
            // dead node is worse than no dashboard.
            let sample = if n.state == NodeState::Dead {
                0
            } else {
                let wobble = ((self.tick as f64 * 0.3 + i as f64).sin() * 90.0) as i64;
                (120 + wobble).max(0) as u64
            };
            n.throughput.push_back(sample);
            if n.throughput.len() > 24 {
                n.throughput.pop_front();
            }
        }
    }

    fn on_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('q') | KeyCode::Esc => self.quit = true,
            KeyCode::Tab => {
                self.tab = if self.tab == Tab::Nodes { Tab::Health } else { Tab::Nodes }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.selected = (self.selected + 1) % self.nodes.len()
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = (self.selected + self.nodes.len() - 1) % self.nodes.len()
            }
            _ => {}
        }
    }

    fn used(&self) -> u64 {
        self.nodes.iter().map(|n| n.used).sum()
    }
    fn capacity(&self) -> u64 {
        self.nodes.iter().filter(|n| n.state != NodeState::Dead).map(|n| n.capacity).sum()
    }
    fn live(&self) -> usize {
        self.nodes.iter().filter(|n| n.state != NodeState::Dead).count()
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// Drawing
// ═════════════════════════════════════════════════════════════════════════════

fn draw(frame: &mut Frame, app: &App) {
    let [header, tabs, body, footer] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    draw_header(frame, header, app);
    draw_tabs(frame, tabs, app);
    match app.tab {
        Tab::Nodes => draw_nodes(frame, body, app),
        Tab::Health => draw_health(frame, body, app),
    }
    draw_footer(frame, footer);
}

fn draw_header(frame: &mut Frame, area: Rect, app: &App) {
    let fill = app.used() as f64 / app.capacity().max(1) as f64;
    let dead = app.nodes.len() - app.live();

    let line = Line::from(vec![
        Span::styled("mammoth", Style::default().fg(tone::ACCENT).add_modifier(Modifier::BOLD)),
        Span::styled("  prod-eu-1  ", Style::default().fg(tone::MUTED)),
        Span::raw(format!("{} / {}  ", human(app.used()), human(app.capacity()))),
        Span::styled(
            format!("{:.0}% used", fill * 100.0),
            Style::default().fg(tone::for_fill(fill)),
        ),
        Span::styled("   ·   ", Style::default().fg(tone::MUTED)),
        Span::styled(format!("{} live", app.live()), Style::default().fg(tone::OK)),
        if dead > 0 {
            Span::styled(
                format!("   {dead} dead"),
                Style::default().fg(tone::CRITICAL).add_modifier(Modifier::BOLD),
            )
        } else {
            Span::raw("")
        },
    ]);

    frame.render_widget(
        Paragraph::new(line).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(tone::MUTED))
                .title(Span::styled(" cluster ", Style::default().fg(tone::MUTED))),
        ),
        area,
    );
}

fn draw_tabs(frame: &mut Frame, area: Rect, app: &App) {
    let selected = match app.tab {
        Tab::Nodes => 0,
        Tab::Health => 1,
    };
    frame.render_widget(
        Tabs::new(vec![" nodes ", " health "])
            .select(selected)
            .style(Style::default().fg(tone::MUTED))
            .highlight_style(
                Style::default().fg(Color::Black).bg(tone::ACCENT).add_modifier(Modifier::BOLD),
            )
            .divider(""),
        area,
    );
}

fn draw_nodes(frame: &mut Frame, area: Rect, app: &App) {
    // A `Table` where one column is a sparkline is not possible — a Table cell
    // holds text. So the sparkline lives in its own column of the layout,
    // rendered row by row beside the table. This is the usual way to get a
    // widget "inside" a table in ratatui.
    let [table_area, spark_area] =
        Layout::horizontal([Constraint::Min(48), Constraint::Length(28)]).areas(area);

    let rows: Vec<Row> = app
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let fill = n.fill();
            let selected = i == app.selected;
            let base = if selected {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            Row::new(vec![
                Cell::from(n.state.symbol()).style(Style::default().fg(n.state.colour())),
                Cell::from(n.id),
                Cell::from(n.rack).style(Style::default().fg(tone::MUTED)),
                Cell::from(text_bar(fill, 14)).style(Style::default().fg(tone::for_fill(fill))),
                Cell::from(format!("{:>3.0}%", fill * 100.0))
                    .style(Style::default().fg(tone::for_fill(fill))),
                Cell::from(format!("{:>9}", thousands(n.blocks)))
                    .style(Style::default().fg(tone::MUTED)),
            ])
            .style(base)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(1),
            Constraint::Length(4),
            Constraint::Length(12),
            Constraint::Length(14),
            Constraint::Length(5),
            Constraint::Length(10),
        ],
    )
    .header(
        Row::new(vec!["", "NODE", "RACK", "USED", "", "BLOCKS"])
            .style(Style::default().fg(tone::MUTED).add_modifier(Modifier::BOLD)),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(tone::MUTED))
            .title(Span::styled(" workers ", Style::default().fg(tone::MUTED))),
    );
    frame.render_widget(table, table_area);

    // The sparkline column, one row per node, aligned with the table's rows:
    // 1 for the top border + 1 for the header = the first data row is at +2.
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(tone::MUTED))
        .title(Span::styled(" MB/s, last 24s ", Style::default().fg(tone::MUTED)));
    let inner = block.inner(spark_area);
    frame.render_widget(block, spark_area);

    for (i, n) in app.nodes.iter().enumerate() {
        let y = inner.y + 1 + i as u16;
        if y >= inner.bottom() {
            break;
        }
        let row = Rect { x: inner.x, y, width: inner.width, height: 1 };
        let data: Vec<u64> = n.throughput.iter().copied().collect();
        frame.render_widget(
            Sparkline::default().data(&data).style(Style::default().fg(n.state.colour())),
            row,
        );
    }
}

fn draw_health(frame: &mut Frame, area: Rect, app: &App) {
    let [gauges, note] = Layout::vertical([Constraint::Length(10), Constraint::Min(0)]).areas(area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(tone::MUTED))
        .title(Span::styled(" replication ", Style::default().fg(tone::MUTED)));
    let inner = block.inner(gauges);
    frame.render_widget(block, gauges);

    // Numbers move while `w6` is dead; a real one comes from ClusterReport.
    let recovering = (app.tick % 1200) as f64 / 1200.0;
    let rows: [(&str, f64, Color); 4] = [
        ("healthy   (3/3)", 0.964 + 0.03 * recovering, tone::OK),
        ("under-repl (2/3)", 0.034 - 0.03 * recovering, tone::WARN),
        ("critical   (1/3)", 0.002, tone::CRITICAL),
        ("corrupt", 0.000_2, tone::CRITICAL),
    ];

    let areas = Layout::vertical([Constraint::Length(2); 4]).split(inner);
    for ((label, frac, colour), a) in rows.iter().zip(areas.iter()) {
        frame.render_widget(
            Gauge::default()
                .gauge_style(Style::default().fg(*colour))
                .ratio(frac.clamp(0.0, 1.0))
                .label(Span::styled(
                    format!("{label}   {:.2}%", frac * 100.0),
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                )),
            *a,
        );
    }

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                "  w6 has been dead for 12m — 1,216 blocks are being rebuilt",
                Style::default().fg(tone::WARN),
            )),
            Line::from(Span::styled(
                "  reads never failed: two copies of every block remained",
                Style::default().fg(tone::MUTED),
            )),
        ])
        .alignment(Alignment::Left),
        note,
    );
}

fn draw_footer(frame: &mut Frame, area: Rect) {
    let key = Style::default().fg(Color::Black).bg(tone::ACCENT);
    let label = Style::default().fg(tone::MUTED);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" q ", key),
            Span::styled(" quit  ", label),
            Span::styled(" tab ", key),
            Span::styled(" switch view  ", label),
            Span::styled(" j/k ", key),
            Span::styled(" select node  ", label),
        ])),
        area,
    );
}

// ═════════════════════════════════════════════════════════════════════════════
// Helpers
// ═════════════════════════════════════════════════════════════════════════════

/// The same eighth-block bar as `mammoth-viz` — a TUI cell holds one character,
/// so a text bar is still the right tool inside a table row.
fn text_bar(fraction: f64, width: usize) -> String {
    const EIGHTHS: [char; 8] = ['▏', '▎', '▍', '▌', '▋', '▊', '▉', '█'];
    let f = fraction.clamp(0.0, 1.0);
    let total = (f * width as f64 * 8.0).round() as usize;
    let (full, rem) = (total / 8, total % 8);
    let mut s = String::new();
    for _ in 0..full.min(width) {
        s.push('█');
    }
    let mut used = full.min(width);
    if rem > 0 && used < width {
        s.push(EIGHTHS[rem - 1]);
        used += 1;
    }
    for _ in used..width {
        s.push('░');
    }
    s
}

fn human(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let (mut v, mut u) = (bytes as f64, 0);
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    format!("{v:.1} {}", UNITS[u])
}

fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

// ═════════════════════════════════════════════════════════════════════════════
// Running it
// ═════════════════════════════════════════════════════════════════════════════

#[derive(Parser)]
#[command(about = "htop for your cluster")]
struct Args {
    /// Render both tabs to an in-memory buffer and print them. No tty needed.
    #[arg(long)]
    check: bool,
}

fn main() -> io::Result<()> {
    let args = Args::parse();
    if args.check {
        return check();
    }

    let mut terminal = ratatui::init();
    let result = run(&mut terminal);
    ratatui::restore();
    result
}

fn run(terminal: &mut ratatui::DefaultTerminal) -> io::Result<()> {
    let mut app = App::new();
    loop {
        terminal.draw(|frame| draw(frame, &app))?;
        if event::poll(Duration::from_millis(120))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    app.on_key(key.code);
                }
            }
        }
        app.tick();
        if app.quit {
            return Ok(());
        }
    }
}

fn check() -> io::Result<()> {
    let mut app = App::new();
    for _ in 0..8 {
        app.tick();
    }
    for tab in [Tab::Nodes, Tab::Health] {
        app.tab = tab;
        let mut terminal = Terminal::new(ratatui::backend::TestBackend::new(80, 18))?;
        terminal.draw(|frame| draw(frame, &app))?;
        let buffer = terminal.backend().buffer().clone();
        println!();
        for y in 0..buffer.area.height {
            let row: String =
                (0..buffer.area.width).map(|x| buffer[(x, y)].symbol().to_string()).collect();
            println!("  {row}");
        }
    }
    println!();
    println!("  Both frames drawn with no terminal attached. Colour is in the buffer");
    println!("  too — assert on `buffer[(x, y)].fg` to test that the dead node is red.");
    Ok(())
}
