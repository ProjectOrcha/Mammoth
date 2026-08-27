//! 10 · Colour in a terminal, and when you are allowed to use it.
//!
//!     cargo run -q -p mammoth-parts --example 10-colour-basics
//!     cargo run -q -p mammoth-parts --example 10-colour-basics | cat
//!     NO_COLOR=1 cargo run -q -p mammoth-parts --example 10-colour-basics
//!     cargo run -q -p mammoth-parts --example 10-colour-basics -- --color never
//!     cargo run -q -p mammoth-parts --example 10-colour-basics -- --color always | cat -v
//!
//! Run all five. The last one shows the raw escape codes, which is the fastest
//! way to see what is actually being written.
//!
//! ## What colour actually is
//!
//! There is no such thing as a "red string". Colour is a pair of control
//! sequences the terminal interprets, wrapped around ordinary text:
//!
//!     \x1b[31m  hello  \x1b[39m
//!     ^^^^^^^^         ^^^^^^^^
//!     set red          back to default
//!
//! `\x1b` is ESC. Every one of these is called an ANSI escape sequence. A
//! terminal renders them; `grep`, a log file and a CI web page do not — they
//! show the literal characters, which is why the gating below matters more than
//! the palette does.

use std::io::IsTerminal;

use clap::{Parser, ValueEnum};
use owo_colors::{OwoColorize, Stream};

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum ColourChoice {
    /// Colour when stdout is a terminal that wants it. The only sane default.
    Auto,
    /// Always, even into a pipe — for `less -R` and for CI logs that render it.
    Always,
    /// Never.
    Never,
}

#[derive(Parser)]
#[command(about = "Colour, and the three questions you must ask before using it")]
struct Args {
    #[arg(long, value_enum, default_value = "auto")]
    color: ColourChoice,
}

fn main() {
    let args = Args::parse();

    // ── The one decision ────────────────────────────────────────────────────
    //
    // owo-colors' `supports-colors` feature answers all three questions for
    // you, in the order every well-behaved CLI answers them:
    //
    //   1. did the user say so explicitly?      --color always | never
    //   2. is NO_COLOR set to anything?         https://no-color.org
    //   3. is stdout a terminal that can do it? not a pipe, not TERM=dumb
    //
    // `set_override` forces the answer for questions 2 and 3; leaving it unset
    // lets `if_supports_color` work them out per stream.
    match args.color {
        ColourChoice::Always => owo_colors::set_override(true),
        ColourChoice::Never => owo_colors::set_override(false),
        ColourChoice::Auto => owo_colors::unset_override(),
    }

    heading("the three questions");
    println!("  stdout is a terminal   {}", yes_no(std::io::stdout().is_terminal()));
    println!("  NO_COLOR is set        {}", yes_no(std::env::var_os("NO_COLOR").is_some()));
    println!(
        "  colour will be used    {}",
        yes_no("x".if_supports_color(Stream::Stdout, |t| t.red()).to_string().contains('\u{1b}'))
    );

    heading("the sixteen you can rely on");
    // These sixteen exist on every terminal made since about 1985, including
    // an SSH session into a router. Everything else degrades; these do not.
    let names = ["black", "red", "green", "yellow", "blue", "magenta", "cyan", "white"];
    print!("  normal   ");
    for (i, n) in names.iter().enumerate() {
        print!("{} ", paint(n, i as u8));
    }
    println!();
    print!("  bright   ");
    for (i, n) in names.iter().enumerate() {
        print!("{} ", paint(n, i as u8 + 8));
    }
    println!();

    heading("styles, which are not colours");
    println!(
        "  {}",
        "bold — for the one word that matters".if_supports_color(Stream::Stdout, |t| t.bold())
    );
    println!(
        "  {}",
        "dimmed — for units, hints, and things you may ignore"
            .if_supports_color(Stream::Stdout, |t| t.dimmed())
    );
    println!(
        "  {}",
        "italic — patchy support; do not depend on it"
            .if_supports_color(Stream::Stdout, |t| t.italic())
    );
    println!("  {}", "underline — for links".if_supports_color(Stream::Stdout, |t| t.underline()));
    println!(
        "  {}",
        "reversed — swaps fg and bg; good for a selected row"
            .if_supports_color(Stream::Stdout, |t| t.reversed())
    );

    heading("256 colours and true colour");
    // 256-colour, addressed by index. `XtermColors::from(n)` turns a run-time
    // number into a colour, which is what you need when the colour comes from
    // data rather than from a literal in your source.
    print!("  256       ");
    for i in [39u8, 45, 51, 87, 123, 159, 195] {
        let c = owo_colors::XtermColors::from(i);
        print!("{} ", format!("{i:>3}").if_supports_color(Stream::Stdout, |t| t.color(c)));
    }
    println!(" indices 16–231 are a 6×6×6 RGB cube, 232–255 a grey ramp");

    // 24-bit. Gorgeous on a modern terminal, ignored or approximated on an old
    // one — which is why the palette in example 11 uses the basic sixteen.
    print!("  truecolor ");
    for step in 0..12 {
        let g = 40 + step * 15;
        print!("{}", "  ".if_supports_color(Stream::Stdout, |t| t.on_truecolor(20, g, 90)));
    }
    println!("  24-bit, if $COLORTERM says truecolor");

    heading("the rule");
    println!(
        "  Colour must be the {} channel, never the only one.",
        "second".if_supports_color(Stream::Stdout, |t| t.bold())
    );
    println!();
    println!("  A red ✕ and a green ● differ in two ways: colour and shape. Drop");
    println!("  the colour — pipe it, or hand it to one of the ~8% of men with a");
    println!("  red/green deficiency — and the shape still carries the meaning.");
    println!();
    println!("  If your output is unreadable in `| cat`, it is unreadable. Go and");
    println!("  add the symbol, the word, or the number that colour was standing in for.");
}

/// The 16 ANSI colours by index, using owo-colors' dynamic (run-time) form.
/// The static form — `"x".red()` — is faster but the colour must be known when
/// you compile. Anything driven by data needs the dynamic form.
fn paint(text: &str, index: u8) -> String {
    use owo_colors::AnsiColors::*;
    let colour = match index % 8 {
        0 => Black,
        1 => Red,
        2 => Green,
        3 => Yellow,
        4 => Blue,
        5 => Magenta,
        6 => Cyan,
        _ => White,
    };
    let styled = text.if_supports_color(Stream::Stdout, |t| t.color(colour));
    if index >= 8 {
        format!("{}", styled.if_supports_color(Stream::Stdout, |t| t.bold()))
    } else {
        format!("{styled}")
    }
}

fn yes_no(b: bool) -> String {
    if b {
        format!("{}", "yes".if_supports_color(Stream::Stdout, |t| t.green()))
    } else {
        format!("{}", "no".if_supports_color(Stream::Stdout, |t| t.red()))
    }
}

fn heading(s: &str) {
    println!();
    println!("── {} ──", s.if_supports_color(Stream::Stdout, |t| t.bold()));
}
