//! Terminal block matrices, capacity views and a live dashboard.
#![forbid(unsafe_code)]
mod render;
pub mod style;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use mammoth_core::{Backend, Error, Result};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Layout},
    widgets::{Block, Paragraph, Row, Table, TableState, Wrap},
    Terminal,
};
pub use render::{blocks, cluster, health, size, skew, topology, treemap};
use std::{
    io::{IsTerminal, Write},
    sync::Arc,
    time::Duration,
};

struct Restore;
impl Drop for Restore {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen);
    }
}
pub async fn top(backend: Arc<dyn Backend>) -> Result<()> {
    live(backend, false).await
}
pub async fn health_live(backend: Arc<dyn Backend>) -> Result<()> {
    live(backend, true).await
}
async fn live(backend: Arc<dyn Backend>, health_only: bool) -> Result<()> {
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
        let mut selection = TableState::default().with_selected(0);
        let mut row_count = 0usize;
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
                        .style(style::Tone::Heading.tui_style())
                        .block(Block::bordered().title(" Mammoth · local storage ")),
                    areas[0],
                );
                if let Some(Ok(report)) = &snapshot {
                    if health_only {
                        let h = &report.health;
                        let entries = [("Healthy", h.healthy, style::Tone::Ok), ("Under-replicated", h.under_replicated, style::Tone::Warn), ("Critical", h.critical, style::Tone::Critical), ("Over-replicated", h.over_replicated, style::Tone::Warn), ("Corrupt", h.corrupt, style::Tone::Critical), ("Missing", h.missing, style::Tone::Critical)];
                        row_count = entries.len();
                        let max = entries.iter().map(|(_, value, _)| *value).max().unwrap_or(0).max(1);
                        let rows = entries.into_iter().map(|(name, value, tone)| Row::new([name.to_string(), value.to_string(), "█".repeat(((value as f64 / max as f64) * 20.0).round() as usize)]).style(if value > 0 { tone } else { style::Tone::Muted }.tui_style()));
                        let table = Table::new(rows, [Constraint::Percentage(40), Constraint::Percentage(20), Constraint::Percentage(40)])
                            .header(Row::new(["state", "blocks", "relative count"]).style(style::Tone::Heading.tui_style()))
                            .highlight_symbol("▸ ")
                            .block(Block::bordered().title(" Replica health "));
                        frame.render_stateful_widget(table, areas[1], &mut selection);
                    } else {
                    row_count = report.nodes.len();
                    let rows = report.nodes.iter().map(|node| {
                        Row::new([
                            node.id.0.clone(),
                            node.rack.clone(),
                            format!("{:?}", node.state),
                            size(node.used),
                            size(node.capacity),
                            node.blocks.to_string(),
                        ]).style(style::tone_for_node(node.state).tui_style())
                    });
                    let table = Table::new(
                        rows,
                        [
                            Constraint::Percentage(12), Constraint::Percentage(17),
                            Constraint::Percentage(18), Constraint::Percentage(18),
                            Constraint::Percentage(20), Constraint::Percentage(15),
                        ],
                    )
                    .header(
                        Row::new(["worker", "rack", "state", "used", "capacity*", "blocks"])
                            .style(style::Tone::Heading.tui_style())
                            .bottom_margin(1),
                    )
                    .highlight_symbol("▸ ")
                    .block(Block::bordered().title(" Worker directories "));
                    frame.render_stateful_widget(table, areas[1], &mut selection);
                    }
                }
                frame.render_widget(
                    Paragraph::new("↑/↓ or j/k: scroll · q / Esc: quit · refresh: 2s\n*capacity is a reference value").style(style::Tone::Muted.tui_style()),
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
                if key.kind == KeyEventKind::Press {
                    let current = selection.selected().unwrap_or(0);
                    match key.code {
                        KeyCode::Down | KeyCode::Char('j') => selection.select(Some((current + 1).min(row_count.saturating_sub(1)))),
                        KeyCode::Up | KeyCode::Char('k') => selection.select(Some(current.saturating_sub(1))),
                        KeyCode::Home => selection.select(Some(0)),
                        KeyCode::End => selection.select(Some(row_count.saturating_sub(1))),
                        _ => {},
                    }
                }
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
