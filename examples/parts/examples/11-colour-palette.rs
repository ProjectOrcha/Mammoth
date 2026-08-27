//! 11 · One palette, shared by the CLI and the TUI.
//!
//!     cargo run -q -p mammoth-parts --example 11-colour-palette
//!     cargo run -q -p mammoth-parts --example 11-colour-palette | cat
//!
//! The mistake everyone makes first is writing `.red()` at the point of use.
//! Six weeks later "red" means four different things, half of `viz` is yellow
//! for no reason, and changing the scheme means grepping thirty files.
//!
//! The fix is one module that maps **meaning** to colour — `Tone::Critical`,
//! not `red` — and never naming a colour anywhere else. This file is that
//! module, plus proof that it drives both the CLI and the TUI from one source.
//!
//! Copy `mod palette` below into `crates/mammoth-viz/src/style.rs` when you do
//! chapter 8a. It is written to be lifted.

use owo_colors::{OwoColorize, Stream};

use crate::palette::{Tone, TONES};

// ═════════════════════════════════════════════════════════════════════════════
// The module. This is the whole thing — about sixty lines.
// ═════════════════════════════════════════════════════════════════════════════

mod palette {
    use owo_colors::{AnsiColors, Style};

    /// What a piece of output *means*. Never "red", never "green".
    ///
    /// Adding a meaning here is a deliberate act: you have to decide what it
    /// looks like in one place, and every screen picks it up.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Tone {
        /// Healthy, complete, at target. `●`
        Ok,
        /// Degraded but serving. Needs attention, not panic. `◐`
        Warn,
        /// Data at risk, or an outright failure. `✕`
        Critical,
        /// Neutral emphasis: totals, selected rows, the number that matters.
        Accent,
        /// Structure: headers, column titles, section rules.
        Heading,
        /// Units, hints, absent values — everything the eye should slide over.
        Muted,
    }

    /// Every tone, in the order the legend prints them.
    pub const TONES: [Tone; 6] =
        [Tone::Ok, Tone::Warn, Tone::Critical, Tone::Accent, Tone::Heading, Tone::Muted];

    impl Tone {
        /// The ANSI colour. Deliberately from the basic sixteen: those render
        /// identically over SSH, inside tmux, on a Windows console and in a CI
        /// log, and — crucially — they follow the *user's* terminal theme, so a
        /// light-background terminal stays readable. 24-bit colour does not.
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

        /// The full style, colour plus weight.
        pub fn style(self) -> Style {
            let s = Style::new().color(self.colour());
            match self {
                Tone::Heading | Tone::Critical => s.bold(),
                Tone::Muted => s.dimmed(),
                _ => s,
            }
        }

        /// **The half people forget.** Every tone must survive losing its
        /// colour, so each one owns a symbol too. Pipe the output, print it in
        /// black and white, hand it to someone with a red/green deficiency —
        /// the meaning has to still be there.
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

    /// The same six tones, for `ratatui`. The TUI in examples 15 and 16 asks
    /// for `Tone::Warn` and gets the colour the CLI already uses — which is why
    /// `mammoth top` and `mammoth viz cluster` look like one program.
    #[allow(dead_code)] // `style` is what examples 15 and 16 call.
    pub mod tui {
        use ratatui::style::{Color, Modifier, Style};

        use super::Tone;

        pub fn colour(tone: Tone) -> Color {
            match tone {
                Tone::Ok => Color::Green,
                Tone::Warn => Color::Yellow,
                Tone::Critical => Color::Red,
                Tone::Accent => Color::Cyan,
                Tone::Heading => Color::White,
                Tone::Muted => Color::DarkGray,
            }
        }

        pub fn style(tone: Tone) -> Style {
            let s = Style::default().fg(colour(tone));
            match tone {
                Tone::Heading | Tone::Critical => s.add_modifier(Modifier::BOLD),
                Tone::Muted => s.add_modifier(Modifier::DIM),
                _ => s,
            }
        }
    }

    /// Pick a tone from a fill fraction. One place decides what "nearly full"
    /// means, so the CLI heatmap, the TUI gauges and the web UI cannot drift.
    pub fn tone_for_fill(fraction: f64) -> Tone {
        match fraction {
            f if f >= 0.90 => Tone::Critical,
            f if f >= 0.75 => Tone::Warn,
            _ => Tone::Ok,
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// Using it.
// ═════════════════════════════════════════════════════════════════════════════

fn main() {
    heading("the six tones");
    for tone in TONES {
        let sample = format!("{} {:<9}", tone.symbol(), tone.name());
        println!(
            "  {}  {}",
            sample.if_supports_color(Stream::Stdout, |t| t.style(tone.style())),
            describe(tone).if_supports_color(Stream::Stdout, |t| t.style(Tone::Muted.style()))
        );
    }

    heading("a heatmap that colours itself");
    for (id, used, cap) in
        [("w1", 114u64, 160u64), ("w2", 93, 160), ("w3", 151, 160), ("w4", 128, 160)]
    {
        let f = used as f64 / cap as f64;
        let tone = palette::tone_for_fill(f);
        println!(
            "  {id}  {}  {}  {}",
            bar(f, 24).if_supports_color(Stream::Stdout, |t| t.style(tone.style())),
            format!("{:>3.0}%", f * 100.0)
                .if_supports_color(Stream::Stdout, |t| t.style(tone.style())),
            format!("{used} / {cap} GB")
                .if_supports_color(Stream::Stdout, |t| t.style(Tone::Muted.style())),
        );
    }
    println!();
    println!("  No `.red()` anywhere above. `tone_for_fill` decided, once.");

    heading("the same six tones, as ratatui asks for them");
    for tone in TONES {
        println!(
            "  {:<10}  cli: {:<13}  tui: {:?}",
            tone.name(),
            format!("{:?}", tone.colour()),
            palette::tui::colour(tone)
        );
    }
    println!();
    println!("  One enum, two renderers. When someone decides `Warn` should be");
    println!("  orange, they change one line and every screen follows.");

    heading("prove it degrades");
    println!("  Run this again through `| cat`. Every symbol survives; only the");
    println!("  colour goes. That is the test — if piped output loses meaning,");
    println!("  you leaned on colour where you needed a symbol.");
}

fn describe(tone: Tone) -> &'static str {
    match tone {
        Tone::Ok => "healthy · at target replication · node up",
        Tone::Warn => "under-replicated · node near full · slow disk",
        Tone::Critical => "corrupt · missing · node dead · write refused",
        Tone::Accent => "totals, the selected row, the number that matters",
        Tone::Heading => "column titles, section rules",
        Tone::Muted => "units, hints, absent values",
    }
}

/// Example 12 explains this properly.
fn bar(fraction: f64, width: usize) -> String {
    let filled = (fraction.clamp(0.0, 1.0) * width as f64).round() as usize;
    format!("{}{}", "█".repeat(filled), "░".repeat(width - filled))
}

fn heading(s: &str) {
    println!();
    println!("── {} ──", s.if_supports_color(Stream::Stdout, |t| t.style(Tone::Heading.style())));
}
