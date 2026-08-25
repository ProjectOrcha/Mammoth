//! Write this once, use it everywhere (Part V §5.4).
//!
//! Two rules that keep the CLI scriptable: `auto` never emits a table into a
//! pipe, and nothing emits ANSI escapes unless stdout is a terminal.

use std::io::IsTerminal;

use mammoth_core::Error;
use owo_colors::OwoColorize;

use crate::cli::OutputFormat;

/// Anything the CLI can print.
pub trait Render {
    /// Human-facing table form.
    fn to_table(&self) -> comfy_table::Table;
    /// Machine-facing form. Field names here are a public API — treat them as such.
    fn to_json(&self) -> serde_json::Value;
}

impl OutputFormat {
    /// Resolve `auto` against the terminal; every other variant passes through.
    pub fn resolve(self) -> OutputFormat {
        match self {
            OutputFormat::Auto if std::io::stdout().is_terminal() => OutputFormat::Table,
            OutputFormat::Auto => OutputFormat::Json,
            other => other,
        }
    }
}

/// Print a value in the requested format.
pub fn emit<T: Render>(v: &T, fmt: OutputFormat) {
    match fmt.resolve() {
        OutputFormat::Table => println!("{}", v.to_table()),
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&v.to_json()).expect("json"))
        }
        OutputFormat::Yaml | OutputFormat::Csv => {
            unimplemented!("yaml and csv renderers — milestone M2")
        }
        OutputFormat::Auto => unreachable!("resolve() never returns Auto"),
    }
}

/// Print an error the way principle 3 demands: what broke, why, what to do next.
///
/// ```text
/// error[E0301]: not enough healthy workers for replication 3
///
///   only 2 workers are available, but this file requires 3 replicas
///
///   what you can do:
///     · lower replication:   mammoth put ./big.bin /data/big.bin --replication 2
///     · check node health:   mammoth node list
///
///   docs: https://projectorcha.github.io/Mammoth/errors/E0301
/// ```
pub fn print_error(e: &Error) {
    let color = std::io::stderr().is_terminal();
    let tag = format!("error[{}]", e.code());
    if color {
        eprintln!("\n  {}: {e}\n", tag.red().bold());
    } else {
        eprintln!("\n  {tag}: {e}\n");
    }

    let hints = e.hints();
    if !hints.is_empty() {
        eprintln!("  what you can do:");
        for h in hints {
            eprintln!("    · {h}");
        }
        eprintln!();
    }
    eprintln!("  docs: {}\n", e.docs_url());
}
