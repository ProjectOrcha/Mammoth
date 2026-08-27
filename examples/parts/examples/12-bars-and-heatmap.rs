//! 12 · Drawing with characters: bars, sparklines, and a coloured heatmap.
//!
//!     cargo run -q -p mammoth-parts --example 12-bars-and-heatmap
//!     cargo run -q -p mammoth-parts --example 12-bars-and-heatmap | cat
//!
//! Everything here is a **pure function from numbers to a String**. No I/O, no
//! backend, no async. That is deliberate: it means every one of them can be
//! unit-tested, and "the bar is one cell too wide" becomes a failing test
//! rather than a thing you notice mid-demo.
//!
//! This is `crates/mammoth-viz/src/lib.rs`, which chapter 8 has you write.

use owo_colors::{OwoColorize, Stream};

fn main() {
    plain_bars();
    eighths();
    coloured_bars();
    sparklines();
    the_heatmap();
    tests_note();
}

// ─────────────────────────────────────────────────────────────────────────────
// ① A bar is repeat + repeat. The resolution is the problem.
// ─────────────────────────────────────────────────────────────────────────────

fn crude_bar(fraction: f64, width: usize) -> String {
    let filled = (fraction.clamp(0.0, 1.0) * width as f64).round() as usize;
    format!("{}{}", "#".repeat(filled), "-".repeat(width - filled))
}

fn plain_bars() {
    heading("① the crude version");
    for pct in [70, 71, 74, 75] {
        println!("  {pct}%  {}", crude_bar(pct as f64 / 100.0, 16));
    }
    println!();
    println!("  70% and 74% draw identically. At 16 cells you only have 16 steps,");
    println!("  so anything inside a 6-point band collapses to the same picture.");
}

// ─────────────────────────────────────────────────────────────────────────────
// ② Eighth-blocks: eight sub-positions inside one cell, for free.
// ─────────────────────────────────────────────────────────────────────────────

/// `▏▎▍▌▋▊▉█` are eight widths of a partial cell. Using them multiplies your
/// resolution by eight at no cost — the bar is still `width` characters.
const EIGHTHS: [char; 8] = ['▏', '▎', '▍', '▌', '▋', '▊', '▉', '█'];

/// A proportional bar, exactly `width` cells wide, at 1/8-cell resolution.
///
/// The "exactly" is the part that matters. If this ever returns a different
/// number of characters, every column to the right of it jumps.
pub fn bar(fraction: f64, width: usize) -> String {
    let f = fraction.clamp(0.0, 1.0);
    let total_eighths = (f * width as f64 * 8.0).round() as usize;
    let full = total_eighths / 8;
    let rem = total_eighths % 8;

    let mut s = String::with_capacity(width * 3);
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

fn eighths() {
    heading("② eighth-blocks");
    for pct in [70, 71, 74, 75] {
        println!("  {pct}%  {}", bar(pct as f64 / 100.0, 16));
    }
    println!();
    println!("  Same width, ~0.8% resolution. Look at the cell where they differ.");
    println!();
    println!("  One trap: `█` is three bytes in UTF-8. Counting the width of a bar");
    println!("  with `.len()` gives 48 for a 16-cell bar. Always `.chars().count()`.");
    println!(
        "  A 16-cell bar is {} bytes and {} chars.",
        bar(1.0, 16).len(),
        bar(1.0, 16).chars().count()
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// ③ Colour by what the number means, not by where it is on the bar.
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
enum Tone {
    Ok,
    Warn,
    Critical,
}

impl Tone {
    fn for_fill(f: f64) -> Tone {
        match f {
            f if f >= 0.90 => Tone::Critical,
            f if f >= 0.75 => Tone::Warn,
            _ => Tone::Ok,
        }
    }
    fn colour(self) -> owo_colors::AnsiColors {
        match self {
            Tone::Ok => owo_colors::AnsiColors::Green,
            Tone::Warn => owo_colors::AnsiColors::Yellow,
            Tone::Critical => owo_colors::AnsiColors::Red,
        }
    }
    /// The non-colour channel. Without this, a piped heatmap is a wall of grey.
    fn flag(self) -> &'static str {
        match self {
            Tone::Ok => "  ",
            Tone::Warn => " !",
            Tone::Critical => " !!",
        }
    }
}

fn coloured_bar(fraction: f64, width: usize) -> String {
    let tone = Tone::for_fill(fraction);
    let drawn = bar(fraction, width);
    format!("{}", drawn.if_supports_color(Stream::Stdout, |t| t.color(tone.colour())))
}

fn coloured_bars() {
    heading("③ colour carries the threshold");
    for (id, f) in [("w1", 0.71), ("w2", 0.58), ("w3", 0.94), ("w4", 0.80)] {
        let tone = Tone::for_fill(f);
        println!("  {id}  {}  {:>3.0}%{}", coloured_bar(f, 24), f * 100.0, tone.flag());
    }
    println!();
    println!("  Green under 75, yellow to 90, red above. The eye finds w3 before");
    println!("  it has read a single number — and `!!` finds it in a pipe too.");
}

// ─────────────────────────────────────────────────────────────────────────────
// ④ Sparklines: a whole time series in one line.
// ─────────────────────────────────────────────────────────────────────────────

/// The same eight characters, stacked upward instead of sideways.
const SPARKS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

pub fn sparkline(values: &[f64]) -> String {
    let max = values.iter().cloned().fold(f64::MIN, f64::max);
    let min = values.iter().cloned().fold(f64::MAX, f64::min);
    let span = if (max - min).abs() < f64::EPSILON { 1.0 } else { max - min };
    values
        .iter()
        .map(|v| {
            let idx = (((v - min) / span) * 7.0).round() as usize;
            SPARKS[idx.min(7)]
        })
        .collect()
}

fn sparklines() {
    heading("④ sparklines");
    let throughput: Vec<f64> =
        vec![120.0, 180.0, 240.0, 210.0, 90.0, 60.0, 140.0, 260.0, 310.0, 290.0, 180.0, 150.0];
    println!("  read  MB/s  {}  now {} MB/s", sparkline(&throughput), throughput.last().unwrap());

    let latency: Vec<f64> = vec![4.0, 4.2, 4.1, 4.4, 12.0, 31.0, 28.0, 9.0, 4.6, 4.3, 4.1, 4.0];
    println!("  p99   ms    {}  now {} ms", sparkline(&latency), latency.last().unwrap());
    println!();
    println!("  Twelve numbers in twelve characters. `mammoth top` puts one of");
    println!("  these next to every worker.");
}

// ─────────────────────────────────────────────────────────────────────────────
// ⑤ Put it together: the cluster heatmap, grouped by rack.
// ─────────────────────────────────────────────────────────────────────────────

fn the_heatmap() {
    heading("⑤ mammoth viz cluster, in miniature");

    let racks: [(&str, Vec<(&str, f64)>); 3] = [
        ("/dc1/rack-a", vec![("w1", 0.71), ("w2", 0.58)]),
        ("/dc1/rack-b", vec![("w3", 0.94), ("w4", 0.80)]),
        ("/dc1/rack-c", vec![("w5", 0.34), ("w6", 0.41)]),
    ];

    println!();
    for (rack, nodes) in &racks {
        print!("  {rack:<14}");
        for (i, (id, f)) in nodes.iter().enumerate() {
            if i > 0 {
                print!("   ");
            }
            print!("{id:>4} {} {:>3.0}%", coloured_bar(*f, 16), f * 100.0);
        }
        println!();
    }

    let all: Vec<f64> = racks.iter().flat_map(|(_, n)| n.iter().map(|(_, f)| *f)).collect();
    let sigma = std_dev_pct(&all);
    println!();
    let verdict = if sigma < 10.0 { "healthy" } else { "run the balancer" };
    println!("  imbalance  σ = {sigma:.1}%   ({verdict}; healthy < 10%)");
}

/// Population standard deviation as a percentage — the imbalance metric.
/// One number that answers "is the balancer earning its keep?".
pub fn std_dev_pct(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let var = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
    var.sqrt() * 100.0
}

fn tests_note() {
    heading("why these are pure functions");
    // Run the property that matters, right here.
    let mut worst = 0usize;
    for pct in 0..=100 {
        let n = bar(pct as f64 / 100.0, 16).chars().count();
        if n != 16 {
            worst = worst.max(n);
        }
    }
    if worst == 0 {
        println!("  bar() returned exactly 16 cells for all 101 percentages ✔");
    } else {
        println!("  bar() returned {worst} cells somewhere — that is the bug ✕");
    }
    println!();
    println!("  That loop is the real test in `crates/mammoth-viz/src/lib.rs`:");
    println!();
    println!("      #[test]");
    println!("      fn bar_is_always_exactly_width_cells() {{");
    println!("          for pct in 0..=100 {{");
    println!("              assert_eq!(bar(pct as f64 / 100.0, 16).chars().count(), 16, \"at {{pct}}%\");");
    println!("          }}");
    println!("      }}");
    println!();
    println!("  A drawing bug you can catch in CI is a drawing bug you never demo.");
}

fn heading(s: &str) {
    println!();
    println!("── {} ──", s.if_supports_color(Stream::Stdout, |t| t.bold()));
}
