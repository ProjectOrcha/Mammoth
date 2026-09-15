//! The palette. Colour by *meaning*, decided in exactly one place.
//!
//! The rule this file exists to enforce: no other file in the workspace may
//! name a colour. Everything asks for a [`Tone`]. When somebody decides that
//! `Warn` should be orange, they change one line here and the CLI, the TUI and
//! the web UI all follow.

use owo_colors::OwoColorize;
use owo_colors::{AnsiColors, Style};
use std::{
    io::IsTerminal,
    sync::atomic::{AtomicU8, Ordering},
};

static COLOR: AtomicU8 = AtomicU8::new(0);

/// None follows the terminal and NO_COLOR; Some forces the requested mode.
pub fn set_color(value: Option<bool>) {
    COLOR.store(
        match value {
            None => 0,
            Some(true) => 1,
            Some(false) => 2,
        },
        Ordering::Relaxed,
    );
    // Native tables and ratatui use crossterm, which also checks NO_COLOR.
    // Apply our resolved preference there so --color always really overrides it.
    crossterm::style::force_color_output(color_enabled(false));
}
pub fn color_enabled(stderr: bool) -> bool {
    match COLOR.load(Ordering::Relaxed) {
        1 => true,
        2 => false,
        _ => {
            (if stderr { std::io::stderr().is_terminal() } else { std::io::stdout().is_terminal() })
                && std::env::var_os("NO_COLOR").is_none_or(|value| value.is_empty())
                && std::env::var("TERM").map_or(true, |term| term != "dumb")
        }
    }
}
pub fn paint(text: &str, tone: Tone) -> String {
    if color_enabled(false) {
        text.style(tone.style()).to_string()
    } else {
        text.into()
    }
}
pub fn paint_error(text: &str, tone: Tone) -> String {
    if color_enabled(true) {
        text.style(tone.style()).to_string()
    } else {
        text.into()
    }
}
pub fn terminal_width() -> usize {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .or_else(|| crossterm::terminal::size().ok().map(|(width, _)| width as usize))
        .unwrap_or(80)
        .clamp(24, 160)
}
pub fn table() -> comfy_table::Table {
    let mut table = comfy_table::Table::new();
    table
        .load_preset(comfy_table::presets::UTF8_FULL)
        .set_width(terminal_width() as u16)
        .set_content_arrangement(comfy_table::ContentArrangement::Dynamic);
    if color_enabled(false) {
        table.enforce_styling();
    }
    table
}
pub fn cell(text: &str, tone: Tone) -> comfy_table::Cell {
    use comfy_table::{Attribute, Cell, Color};
    let cell = Cell::new(text);
    if !color_enabled(false) {
        return cell;
    }
    let color = match tone {
        Tone::Ok => Color::Green,
        Tone::Warn => Color::Yellow,
        Tone::Critical => Color::Red,
        Tone::Accent => Color::Cyan,
        Tone::Heading => Color::White,
        Tone::Muted => Color::DarkGrey,
    };
    let cell = cell.fg(color);
    if matches!(tone, Tone::Heading | Tone::Critical) {
        cell.add_attribute(Attribute::Bold)
    } else {
        cell
    }
}
pub fn clean(text: &str) -> String {
    text.chars()
        .flat_map(
            |ch| if ch.is_control() { ch.escape_default().collect::<Vec<_>>() } else { vec![ch] },
        )
        .collect()
}
pub fn clip(text: &str, width: usize) -> String {
    use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
    let text = clean(text);
    if text.width() <= width {
        return text;
    }
    if width == 0 {
        return String::new();
    }
    let mut used = 0;
    let mut result = String::new();
    for ch in text.chars() {
        used += ch.width().unwrap_or(0);
        if used >= width {
            break;
        }
        result.push(ch);
    }
    result.push('…');
    result
}

/// What a piece of output *means*. Never "red", never "green".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// Healthy, complete, at target replication.
    Ok,
    /// Degraded but still serving. Attention, not panic.
    Warn,
    /// Data at risk, or an outright failure.
    Critical,
    /// Neutral emphasis: totals, the selected row, the number that matters.
    Accent,
    /// Structure: column headers, section rules.
    Heading,
    /// Units, hints, absent values — everything the eye should slide over.
    Muted,
}

/// Every tone, in the order a legend prints them.
pub const TONES: [Tone; 6] =
    [Tone::Ok, Tone::Warn, Tone::Critical, Tone::Accent, Tone::Heading, Tone::Muted];

impl Tone {
    pub fn tui_style(self) -> ratatui::style::Style {
        use ratatui::style::{Color, Modifier, Style};
        if !color_enabled(false) {
            return Style::default();
        }
        let color = match self {
            Tone::Ok => Color::Green,
            Tone::Warn => Color::Yellow,
            Tone::Critical => Color::Red,
            Tone::Accent => Color::Cyan,
            Tone::Heading => Color::White,
            Tone::Muted => Color::DarkGray,
        };
        let style = Style::default().fg(color);
        if matches!(self, Tone::Heading | Tone::Critical) {
            style.add_modifier(Modifier::BOLD)
        } else {
            style
        }
    }
    /// The ANSI colour. From the basic sixteen on purpose — see chapter 8a §1.
    pub fn colour(self) -> AnsiColors {
        match self {
            Tone::Ok => AnsiColors::Green,
            Tone::Warn => AnsiColors::Yellow,
            Tone::Critical => AnsiColors::Red,
            Tone::Accent => AnsiColors::Cyan,
            Tone::Heading => AnsiColors::White,
            Tone::Muted => AnsiColors::BrightBlack,
        }
    }

    /// Colour plus weight.
    pub fn style(self) -> Style {
        let s = Style::new().color(self.colour());
        match self {
            Tone::Heading | Tone::Critical => s.bold(),
            Tone::Muted => s.dimmed(),
            _ => s,
        }
    }

    /// **The half people forget.** Every tone owns a symbol, so that losing the
    /// colour never loses the meaning.
    pub fn symbol(self) -> char {
        match self {
            Tone::Ok => '●',
            Tone::Warn => '◐',
            Tone::Critical => '✕',
            Tone::Accent => '▸',
            Tone::Heading => '─',
            Tone::Muted => '·',
        }
    }

    /// Name, for legends and for `--json`.
    pub fn name(self) -> &'static str {
        match self {
            Tone::Ok => "ok",
            Tone::Warn => "warn",
            Tone::Critical => "critical",
            Tone::Accent => "accent",
            Tone::Heading => "heading",
            Tone::Muted => "muted",
        }
    }
}

/// Which tone a fill fraction deserves.
///
/// One place decides what "nearly full" means, so the CLI heatmap, the TUI
/// gauges and the web dashboard cannot disagree about it.
pub fn tone_for_fill(fraction: f64) -> Tone {
    match fraction {
        f if f >= 0.90 => Tone::Critical,
        f if f >= 0.75 => Tone::Warn,
        _ => Tone::Ok,
    }
}

/// Which tone a replica state deserves.
pub fn tone_for_replica(state: mammoth_core::types::ReplicaState) -> Tone {
    use mammoth_core::types::ReplicaState;
    match state {
        ReplicaState::Primary => Tone::Ok,
        ReplicaState::Replica => Tone::Accent,
        ReplicaState::Corrupt => Tone::Critical,
    }
}

/// Which tone a node state deserves.
pub fn tone_for_node(state: mammoth_core::types::NodeState) -> Tone {
    use mammoth_core::types::NodeState;
    match state {
        NodeState::Healthy => Tone::Ok,
        NodeState::Warn | NodeState::Decommissioning | NodeState::Maintenance => Tone::Warn,
        NodeState::Dead => Tone::Critical,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tone_has_a_distinct_symbol() {
        // If two tones share a symbol, monochrome output becomes ambiguous —
        // which defeats the entire point of having symbols.
        let mut symbols: Vec<char> = TONES.iter().map(|t| t.symbol()).collect();
        symbols.sort_unstable();
        symbols.dedup();
        assert_eq!(symbols.len(), TONES.len());
    }

    #[test]
    fn fill_thresholds_are_where_we_think() {
        assert_eq!(tone_for_fill(0.74), Tone::Ok);
        assert_eq!(tone_for_fill(0.75), Tone::Warn);
        assert_eq!(tone_for_fill(0.89), Tone::Warn);
        assert_eq!(tone_for_fill(0.90), Tone::Critical);
    }
}
