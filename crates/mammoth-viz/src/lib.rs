//! Terminal block matrices, capacity views and a live dashboard.
#![forbid(unsafe_code)]
pub mod style;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use mammoth_core::{Backend, BlockPlacement, ClusterReport, Error, Result};
use owo_colors::OwoColorize;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Layout},
    style::{Color, Style},
    widgets::{Block, Paragraph, Row, Table, Wrap},
    Terminal,
};
use std::{
    io::{IsTerminal, Write},
    sync::Arc,
    time::Duration,
};

pub fn blocks(layout: &[BlockPlacement]) -> String {
    if layout.is_empty() {
        return "Inlined file: bytes are stored in namespace metadata; no blocks.\n".into();
    }
    let nodes: std::collections::BTreeSet<_> =
        layout.iter().flat_map(|b| b.replicas.iter().map(|r| r.node.0.as_str())).collect();
    let mut table = comfy_table::Table::new();
    let mut header = vec!["block".to_string(), "bytes".into()];
    header.extend(nodes.iter().map(|s| s.to_string()));
    table.set_header(header);
    let color = std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none();
    for b in layout {
        let mut row = vec![format!("{} · {}", b.index, b.id.0), b.len.to_string()];
        for n in &nodes {
            let tone = b
                .replicas
                .iter()
                .find(|r| r.node.0 == *n)
                .map(|r| style::tone_for_replica(r.state))
                .unwrap_or(style::Tone::Muted);
            let symbol = tone.symbol().to_string();
            row.push(if color { symbol.style(tone.style()).to_string() } else { symbol });
        }
        table.add_row(row);
    }
    format!("{table}\n● primary · ▸ replica · ✕ damaged/missing · · absent\n")
}
pub fn cluster(r: &ClusterReport) -> String {
    let mut table = comfy_table::Table::new();
    table.set_header([
        "worker",
        "rack",
        "state",
        "used bytes",
        "model capacity",
        "blocks",
        "usage",
    ]);
    for n in &r.nodes {
        let fraction = if n.capacity == 0 { 0.0 } else { n.used as f64 / n.capacity as f64 };
        let bars = (fraction.clamp(0.0, 1.0) * 20.0).round() as usize;
        table.add_row(vec![
            n.id.0.clone(),
            n.rack.clone(),
            format!("{:?}", n.state),
            n.used.to_string(),
            n.capacity.to_string(),
            n.blocks.to_string(),
            format!("{}{} {:.2}%", "█".repeat(bars), "·".repeat(20 - bars), fraction * 100.0),
        ]);
    }
    format!("{} · {} bytes stored\n{table}\n", r.name, r.used)
}
struct Restore;
impl Drop for Restore {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen);
    }
}
pub async fn top(backend: Arc<dyn Backend>) -> Result<()> {
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return Err(Error::InvalidInput(
            "top requires a terminal; use top --once for a snapshot".into(),
        ));
    }
    enable_raw_mode()?;
    let _restore = Restore;
    execute!(std::io::stdout(), EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(std::io::stdout()))?;
    let (tx, mut rx) = tokio::sync::watch::channel(None);
    let poller = tokio::spawn(async move {
        loop {
            if tx.send(Some(backend.cluster_report().await.map_err(|e| e.to_string()))).is_err() {
                break;
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });
    let result = async {
        let mut snapshot = None;
        loop {
            if rx.has_changed().unwrap_or(false) {
                if let Some(value) = rx.borrow_and_update().as_ref() {
                    snapshot = Some(value.clone());
                }
            }
            terminal.draw(|frame| {
                let areas = Layout::vertical([
                    Constraint::Length(3),
                    Constraint::Min(4),
                    Constraint::Length(2),
                ])
                .split(frame.area());
                let heading = match &snapshot {
                    Some(Ok(report)) => format!(
                        "{} · {} stored · {} healthy blocks · {} missing",
                        report.name,
                        size(report.used),
                        report.health.healthy,
                        report.health.missing
                    ),
                    Some(Err(error)) => format!("Report unavailable: {error}"),
                    None => "Loading cluster report…".into(),
                };
                frame.render_widget(
                    Paragraph::new(heading)
                        .wrap(Wrap { trim: true })
                        .block(Block::bordered().title(" Mammoth · local storage ")),
                    areas[0],
                );
                if let Some(Ok(report)) = &snapshot {
                    let rows = report.nodes.iter().map(|node| {
                        Row::new([
                            node.id.0.clone(),
                            node.rack.clone(),
                            format!("{:?}", node.state),
                            size(node.used),
                            size(node.capacity),
                            node.blocks.to_string(),
                        ])
                    });
                    let table = Table::new(
                        rows,
                        [
                            Constraint::Length(7),
                            Constraint::Length(9),
                            Constraint::Length(10),
                            Constraint::Length(11),
                            Constraint::Length(12),
                            Constraint::Min(6),
                        ],
                    )
                    .header(
                        Row::new(["worker", "rack", "state", "used", "capacity*", "blocks"])
                            .style(Style::default().fg(Color::Cyan))
                            .bottom_margin(1),
                    )
                    .block(Block::bordered().title(" Worker directories "));
                    frame.render_widget(table, areas[1]);
                }
                frame.render_widget(
                    Paragraph::new("q / Esc: quit · refresh: 2s · *capacity is a reference value"),
                    areas[2],
                );
            })?;
            let input = tokio::task::spawn_blocking(|| -> std::io::Result<Option<Event>> {
                if event::poll(Duration::from_millis(100))? {
                    Ok(Some(event::read()?))
                } else {
                    Ok(None)
                }
            })
            .await
            .map_err(|e| Error::Io(std::io::Error::other(e)))??;
            if let Some(Event::Key(key)) = input {
                if key.kind == KeyEventKind::Press
                    && (matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
                        || key.code == KeyCode::Char('c')
                            && key.modifiers.contains(event::KeyModifiers::CONTROL))
                {
                    break;
                }
            }
        }
        terminal.show_cursor()?;
        std::io::stdout().flush()?;
        Ok(())
    }
    .await;
    poller.abort();
    result
}

fn size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut index = 0;
    while value >= 1024.0 && index < UNITS.len() - 1 {
        value /= 1024.0;
        index += 1;
    }
    format!("{value:.1} {}", UNITS[index])
}
