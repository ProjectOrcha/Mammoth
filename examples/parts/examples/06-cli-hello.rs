//! 06 · The smallest possible clap CLI.
//!
//!     cargo run -p mammoth-parts --example 06-cli-hello -- --help
//!     cargo run -p mammoth-parts --example 06-cli-hello -- /data/sales.csv
//!     cargo run -p mammoth-parts --example 06-cli-hello -- /data --long --repeat 3
//!     cargo run -p mammoth-parts --example 06-cli-hello -- --oops
//!
//! Note the bare `--`. Everything before it is for cargo; everything after it
//! is for the program. You will forget this and wonder why `--help` printed
//! cargo's help. Everyone does, once.
//!
//! Forty lines, and you get: parsing, validation, `--help`, `--version`, typo
//! suggestions, and a non-zero exit code on bad input. Hand-rolling argument
//! parsing is never worth it.

use std::path::PathBuf;

use clap::Parser;

/// The doc comment on the struct becomes the `about` line in `--help`.
/// This is the whole reason Mammoth's `cli.rs` is so heavily commented: the
/// comments *are* the user-facing help text.
#[derive(Parser)]
#[command(version, about = "The smallest useful clap program")]
struct Args {
    /// Path to list. A field with no `#[arg]` attribute is **positional**.
    path: PathBuf,

    /// Long format. A `bool` with `short, long` becomes the flag `-l` / `--long`.
    #[arg(short, long)]
    long: bool,

    /// How many times to print it. `default_value_t` uses the value's own
    /// `Display`, so no quotes are needed.
    #[arg(short, long, default_value_t = 1)]
    repeat: u8,

    /// An optional value. Absent on the command line becomes `None`.
    #[arg(long)]
    owner: Option<String>,
}

fn main() {
    // `parse()` handles --help and --version itself and exits. If the arguments
    // are wrong it prints an error and exits with code 2 — you never see it
    // return junk.
    let args = Args::parse();

    for _ in 0..args.repeat {
        if args.long {
            println!(
                "  -rw-r--r--  {:<8} {}",
                args.owner.as_deref().unwrap_or("local"),
                args.path.display()
            );
        } else {
            println!("  {}", args.path.display());
        }
    }

    println!();
    println!("  Now try these, and read what clap does for free:");
    println!("    --help          the generated help, built from the doc comments");
    println!("    --version       from `version` in #[command(...)]");
    println!("    --lnog          a typo — clap suggests `--long`");
    println!("    (no path)       a required argument missing, exit code 2");
}
