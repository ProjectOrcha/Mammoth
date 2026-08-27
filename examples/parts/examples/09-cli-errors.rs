//! 09 · Printing errors like a good CLI: stderr, exit codes, and a way out.
//!
//!     cargo run -q -p mammoth-parts --example 09-cli-errors -- not-enough-workers
//!     cargo run -q -p mammoth-parts --example 09-cli-errors -- not-found --json
//!     cargo run -q -p mammoth-parts --example 09-cli-errors -- ok
//!
//! Then check what the shell saw:
//!
//!     cargo run -q -p mammoth-parts --example 09-cli-errors -- not-found; echo "exit=$?"
//!
//! And prove the error went to stderr, not stdout:
//!
//!     cargo run -q -p mammoth-parts --example 09-cli-errors -- not-found 2>/dev/null
//!     cargo run -q -p mammoth-parts --example 09-cli-errors -- not-found 1>/dev/null
//!
//! Four rules, and every one of them is somebody's bad afternoon if you skip it:
//!
//!   1. errors go to **stderr**, so `mammoth ls /data > out.txt` leaves out.txt
//!      clean and the error still reaches the human
//!   2. failure exits **non-zero**, so `set -e` and CI can tell
//!   3. every error carries a **stable code**, so scripts match on `E0301`
//!      rather than on wording you will want to change
//!   4. every error suggests **the next command to run**

use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use owo_colors::OwoColorize;

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("no such path: {0}")]
    NotFound(PathBuf),
    #[error("not enough healthy workers for replication {wanted}: only {available} available")]
    NotEnoughWorkers { wanted: u8, available: u8 },
}

impl Error {
    fn code(&self) -> &'static str {
        match self {
            Error::NotFound(_) => "E0101",
            Error::NotEnoughWorkers { .. } => "E0301",
        }
    }

    fn hints(&self) -> Vec<String> {
        match self {
            Error::NotFound(p) => vec![format!(
                "list the parent:     mammoth ls {}",
                p.parent().unwrap_or(std::path::Path::new("/")).display()
            )],
            Error::NotEnoughWorkers { available, .. } => vec![
                format!("lower replication:   mammoth put <src> <dst> --replication {available}"),
                "check node health:   mammoth node list".into(),
            ],
        }
    }

    fn docs_url(&self) -> String {
        format!("https://projectorcha.github.io/Mammoth/errors/{}", self.code())
    }
}

/// The human form. Note **eprintln!** throughout — `println!` here would be the
/// bug that quietly corrupts every pipeline that uses this command.
fn print_error(e: &Error) {
    // Colour only when stderr is a terminal. A redirected error log full of
    // `\x1b[31m` is nobody's idea of readable.
    let colour = std::io::stderr().is_terminal();
    let tag = format!("error[{}]", e.code());

    eprintln!();
    if colour {
        eprintln!("  {}: {e}", tag.red().bold());
    } else {
        eprintln!("  {tag}: {e}");
    }
    eprintln!();

    let hints = e.hints();
    if !hints.is_empty() {
        eprintln!("  what you can do:");
        for h in hints {
            if colour {
                eprintln!("    {} {h}", "·".cyan());
            } else {
                eprintln!("    · {h}");
            }
        }
        eprintln!();
    }
    eprintln!("  docs: {}", e.docs_url());
    eprintln!();
}

/// The machine form. A script that asked for `--json` wants the *error* in JSON
/// too, not a paragraph of prose it has to regex.
fn print_error_json(e: &Error) {
    let payload = serde_json::json!({
        "error": {
            "code": e.code(),
            "message": e.to_string(),
            "hints": e.hints(),
            "docs": e.docs_url(),
        }
    });
    eprintln!("{}", serde_json::to_string_pretty(&payload).expect("json"));
}

#[derive(Parser)]
#[command(about = "Three ways for a command to end")]
struct Args {
    /// ok | not-found | not-enough-workers
    which: String,
    /// Emit the error as JSON instead of prose.
    #[arg(long)]
    json: bool,
}

fn run(args: &Args) -> Result<(), Error> {
    match args.which.as_str() {
        "ok" => {
            println!("  ✔ /data/hello.txt   19 B · inlined");
            Ok(())
        }
        "not-found" => Err(Error::NotFound(PathBuf::from("/data/nope.txt"))),
        _ => Err(Error::NotEnoughWorkers { wanted: 3, available: 2 }),
    }
}

/// `main` returning `ExitCode` is the tidy way to control the exit status.
/// The alternative, `std::process::exit(1)`, skips destructors — which means
/// unflushed buffers and un-removed lock files.
fn main() -> ExitCode {
    let args = Args::parse();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS, // 0
        Err(e) => {
            if args.json {
                print_error_json(&e);
            } else {
                print_error(&e);
            }
            ExitCode::FAILURE // 1
        }
    }
}
