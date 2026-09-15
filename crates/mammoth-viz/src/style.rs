//! The palette. Colour by *meaning*, decided in exactly one place.
//!
//! The rule this file exists to enforce: no other file in the workspace may
//! name a colour. Everything asks for a [`Tone`]. When somebody decides that
//! `Warn` should be orange, they change one line here and the CLI, the TUI and
//! the web UI all follow.

use owo_colors::{AnsiColors, Style};

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
