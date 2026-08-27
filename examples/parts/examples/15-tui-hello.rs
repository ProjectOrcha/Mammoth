//! 15 · The smallest ratatui program: raw mode, a draw loop, and getting out.
//!
//!     cargo run -q -p mammoth-parts --example 15-tui-hello           # interactive; q to quit
//!     cargo run -q -p mammoth-parts --example 15-tui-hello -- --check   # headless, prints one frame
//!
//! A TUI is not a program that prints. It is a program that **owns the screen**:
//! it takes the terminal out of its normal line-by-line mode, draws a full
//! picture many times a second, and must put everything back exactly as it
//! found it — including when it panics.
//!
//! ## The three things a TUI does that a CLI does not
//!
//! 1. **Alternate screen.** Like `less` or `vim`: your shell scrollback is
//!    untouched, and on exit the old screen comes straight back.
//! 2. **Raw mode.** Keystrokes arrive one at a time, unbuffered, un-echoed —
//!    so `q` quits immediately instead of waiting for Enter, and Ctrl-C is
//!    yours to handle. It also means **if you exit without restoring, the
//!    user's shell is broken**: no echo, no line editing. Test your panic path.
//! 3. **A frame loop.** Every frame redraws everything from your state.
//!    Ratatui diffs against the previous frame and writes only the cells that
//!    changed, so this is far cheaper than it sounds.

use std::io;
use std::time::Duration;

use clap::Parser;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph};
use ratatui::{Frame, Terminal};

#[derive(Parser)]
#[command(about = "The smallest useful ratatui program")]
struct Args {
    /// Render exactly one frame to an in-memory buffer and print it. No
    /// terminal required — this is how you test a TUI, and how CI runs it.
    #[arg(long)]
    check: bool,
}

/// Everything the screen is a function of. Keep this small and keep it plain
/// data: the draw function must be able to run without touching anything else.
struct App {
    ticks: u64,
    fill: f64,
    quit: bool,
}

impl App {
    fn new() -> Self {
        Self { ticks: 0, fill: 0.42, quit: false }
    }

    /// One step of the world. Separated from drawing so it can be unit-tested.
    fn tick(&mut self) {
        self.ticks += 1;
        self.fill = ((self.ticks as f64) / 40.0).sin().abs();
    }

    fn on_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('q') | KeyCode::Esc => self.quit = true,
            KeyCode::Char('c') => self.fill = 0.0,
            _ => {}
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Drawing. A pure function of `&App` — no I/O, no mutation, no surprises.
// ─────────────────────────────────────────────────────────────────────────────

fn draw(frame: &mut Frame, app: &App) {
    // Layout splits a rectangle into rectangles. `Length` is fixed rows,
    // `Min(0)` soaks up whatever is left, `Percentage(n)` does what it says.
    let [header, body, footer] =
        Layout::vertical([Constraint::Length(3), Constraint::Min(0), Constraint::Length(1)])
            .areas(frame.area());

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("mammoth", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw("  ·  hello, terminal"),
        ]))
        .block(Block::default().borders(Borders::ALL).title(" 15-tui-hello ")),
        header,
    );

    frame.render_widget(
        Gauge::default()
            .block(Block::default().borders(Borders::ALL).title(" a gauge "))
            .gauge_style(Style::default().fg(tone_for(app.fill)))
            .ratio(app.fill.clamp(0.0, 1.0))
            .label(format!("{:.0}%  ·  tick {}", app.fill * 100.0, app.ticks)),
        body,
    );

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" q ", Style::default().fg(Color::Black).bg(Color::Cyan)),
            Span::styled(" quit   ", Style::default().fg(Color::DarkGray)),
            Span::styled(" c ", Style::default().fg(Color::Black).bg(Color::Cyan)),
            Span::styled(" clear", Style::default().fg(Color::DarkGray)),
        ])),
        footer,
    );
}

/// The same thresholds the CLI palette uses (example 11). One rule, two
/// renderers — that is how `mammoth top` and `mammoth viz cluster` stay
/// recognisably the same product.
fn tone_for(fill: f64) -> Color {
    match fill {
        f if f >= 0.90 => Color::Red,
        f if f >= 0.75 => Color::Yellow,
        _ => Color::Green,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Running it.
// ─────────────────────────────────────────────────────────────────────────────

fn main() -> io::Result<()> {
    let args = Args::parse();
    if args.check {
        return check();
    }

    // `ratatui::init()` does four things: enters the alternate screen, turns on
    // raw mode, hides the cursor, and — the important one — installs a panic
    // hook that undoes all three before the panic message prints. Without that
    // hook, one `unwrap()` on a None leaves the user with a dead shell.
    let mut terminal = ratatui::init();
    let result = run(&mut terminal);
    // Always restore, on every path out. If you take one thing from this file:
    // the restore must not be skippable.
    ratatui::restore();
    result
}

fn run(terminal: &mut ratatui::DefaultTerminal) -> io::Result<()> {
    let mut app = App::new();
    loop {
        terminal.draw(|frame| draw(frame, &app))?;

        // `poll` with a timeout is what makes this both responsive to keys and
        // alive on its own. Block forever on `read()` and the gauge freezes
        // between keystrokes; use a bare loop with no poll and you burn a core.
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // Windows sends both Press and Release. Without this check,
                // every keystroke registers twice — a classic first TUI bug.
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

/// Render one frame into a plain in-memory buffer and print it as text.
///
/// This is the whole answer to "how do you test a TUI". `TestBackend` is a
/// grid of cells with no terminal behind it, so a test can assert on exactly
/// what would have been drawn — in CI, with no tty, deterministically.
fn check() -> io::Result<()> {
    let backend = ratatui::backend::TestBackend::new(64, 12);
    let mut terminal = Terminal::new(backend)?;
    let mut app = App::new();
    app.tick();

    terminal.draw(|frame| draw(frame, &app))?;

    // Read the cells back out as text.
    let buffer = terminal.backend().buffer().clone();
    println!("  one frame, {}x{}, rendered with no terminal at all:\n", 64, 12);
    for y in 0..buffer.area.height {
        let row: String =
            (0..buffer.area.width).map(|x| buffer[(x, y)].symbol().to_string()).collect();
        println!("  │{row}│");
    }
    println!();
    println!("  In a real test you would assert on this instead of printing it:");
    println!();
    println!("      assert!(buffer_text.contains(\"mammoth\"));");
    Ok(())
}
