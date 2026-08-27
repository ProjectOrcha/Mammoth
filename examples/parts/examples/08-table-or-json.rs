//! 08 · One `Render` trait, four output formats, zero `if json` branches.
//!
//!     cargo run -q -p mammoth-parts --example 08-table-or-json
//!     cargo run -q -p mammoth-parts --example 08-table-or-json | cat
//!     cargo run -q -p mammoth-parts --example 08-table-or-json -- --output csv
//!
//! Run it twice: once straight to your terminal, once through `| cat`. Same
//! program, same arguments, different output — because a table piped into
//! another program is useless and JSON read by a human is unpleasant.
//!
//! This is `crates/mammoth-cli/src/output.rs`. Write it once in chapter 2, and
//! every command you add afterwards speaks JSON for free.

use std::io::IsTerminal;

use clap::{Parser, ValueEnum};
use serde::Serialize;

// ─────────────────────────────────────────────────────────────────────────────
// ① The trait. Two methods: how to look, and how to be parsed.
// ─────────────────────────────────────────────────────────────────────────────

trait Render {
    /// Human-facing.
    fn to_table(&self) -> comfy_table::Table;
    /// Machine-facing. **These field names are a public API.** Renaming one
    /// breaks somebody's script as surely as renaming a command would.
    fn to_json(&self) -> serde_json::Value;
}

// ─────────────────────────────────────────────────────────────────────────────
// ② The data. `Serialize` does the JSON half for free.
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct FileStatus {
    path: String,
    len: u64,
    replication: u8,
    blocks: u32,
    inlined: bool,
}

struct Listing(Vec<FileStatus>);

impl Render for Listing {
    fn to_table(&self) -> comfy_table::Table {
        let mut t = comfy_table::Table::new();
        // NOTHING is the preset Mammoth uses: no borders, just aligned columns.
        // It is the one that survives being pasted into a chat window.
        t.load_preset(comfy_table::presets::NOTHING);
        t.set_header(vec!["SIZE", "REPL", "BLOCKS", "NAME"]);
        for f in &self.0 {
            t.add_row(vec![
                human(f.len),
                f.replication.to_string(),
                if f.inlined { "inline".into() } else { f.blocks.to_string() },
                f.path.clone(),
            ]);
        }
        t
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(&self.0).unwrap_or(serde_json::Value::Null)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ③ The resolver. `auto` is the whole idea: never emit a table into a pipe.
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum OutputFormat {
    Auto,
    Table,
    Json,
    Csv,
}

impl OutputFormat {
    fn resolve(self) -> OutputFormat {
        match self {
            // `is_terminal()` asks the OS whether stdout is a tty. A pipe, a
            // file and a CI log are all "not a terminal".
            OutputFormat::Auto if std::io::stdout().is_terminal() => OutputFormat::Table,
            OutputFormat::Auto => OutputFormat::Json,
            other => other,
        }
    }
}

fn emit<T: Render>(v: &T, fmt: OutputFormat) {
    match fmt.resolve() {
        OutputFormat::Table => println!("{}", v.to_table()),
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&v.to_json()).expect("json"))
        }
        OutputFormat::Csv => {
            // A third format costs one arm here and nothing in any command.
            let rows = v.to_json();
            if let Some(arr) = rows.as_array() {
                if let Some(first) = arr.first().and_then(|r| r.as_object()) {
                    println!("{}", first.keys().cloned().collect::<Vec<_>>().join(","));
                }
                for row in arr {
                    let cells: Vec<String> = row
                        .as_object()
                        .map(|o| o.values().map(scalar).collect())
                        .unwrap_or_default();
                    println!("{}", cells.join(","));
                }
            }
        }
        OutputFormat::Auto => unreachable!("resolve() never returns Auto"),
    }
}

fn scalar(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// 367001600 → "350.0 MB"
fn human(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let (mut v, mut u) = (bytes as f64, 0);
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    format!("{v:.1} {}", UNITS[u])
}

#[derive(Parser)]
#[command(about = "Table on a terminal, JSON in a pipe")]
struct Args {
    #[arg(long, value_enum, default_value = "auto")]
    output: OutputFormat,
}

fn main() {
    let args = Args::parse();

    let listing = Listing(vec![
        FileStatus { path: "hello.txt".into(), len: 19, replication: 3, blocks: 0, inlined: true },
        FileStatus {
            path: "sales.csv".into(),
            len: 350_000,
            replication: 3,
            blocks: 3,
            inlined: false,
        },
        FileStatus {
            path: "events.parquet".into(),
            len: 900_000,
            replication: 3,
            blocks: 9,
            inlined: false,
        },
    ]);

    emit(&listing, args.output);

    // Only print the commentary when a human is watching. Anything else would
    // corrupt the JSON a script is parsing — which is the whole point.
    if std::io::stdout().is_terminal() && args.output == OutputFormat::Auto {
        println!("  You are on a terminal, so `auto` chose a table. Now run:");
        println!();
        println!("    cargo run -q -p mammoth-parts --example 08-table-or-json | cat");
        println!();
        println!("  Same code, same flags. JSON, because stdout is a pipe.");
    }
}
