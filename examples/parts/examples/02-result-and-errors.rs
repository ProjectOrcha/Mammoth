//! 02 · `Result`, `Option`, `?`, and errors that teach.
//!
//!     cargo run -p mammoth-parts --example 02-result-and-errors
//!
//! Rust has no exceptions. A function that can fail says so in its return type.
//! This example builds a miniature version of `mammoth_core::Error` so you can
//! see the whole shape — variants, codes, hints — in forty lines.

use std::path::PathBuf;

fn main() {
    options();
    results();
    the_question_mark();
    errors_that_teach();
}

// ─────────────────────────────────────────────────────────────────────────────
// Option<T> — a value that might not be there. Rust has no `null`.
// ─────────────────────────────────────────────────────────────────────────────

/// `None` for a directory, `Some(n)` for a file. Written as a function so the
/// values are not compile-time constants — clippy is right that
/// `None.unwrap_or(0)` is silly, and so would you be for writing it.
fn replication_of(path: &str) -> Option<u8> {
    if path.ends_with(".csv") {
        Some(3)
    } else {
        None
    }
}

fn options() {
    println!("\n── Option<T> ──");

    // A directory has no replication factor; a file does. The type says so.
    let file_replication = replication_of("/data/sales.csv");
    let dir_replication = replication_of("/data");

    // Four ways to get at the inside, in rough order of how often you want them:

    // 1. a default
    println!("  unwrap_or:   {}", dir_replication.unwrap_or(0));

    // 2. run some code only if it is there
    if let Some(r) = file_replication {
        println!("  if let:      replication is {r}");
    }

    // 3. transform what is inside, leaving None alone
    let shown: Option<String> = file_replication.map(|r| format!("{r}x"));
    println!("  map:         {shown:?}");

    // 4. match, when you want to handle both sides explicitly
    let label = match dir_replication {
        Some(r) => r.to_string(),
        None => "—".to_string(),
    };
    println!("  match:       {label}   (this is how `ls` prints directories)");
}

// ─────────────────────────────────────────────────────────────────────────────
// Result<T, E> — either Ok(value) or Err(problem).
// ─────────────────────────────────────────────────────────────────────────────

/// Parse `128MB`, `64KB`, `1024` into a byte count. A tiny version of
/// `mammoth_cli::commands::parse_size`.
fn parse_size(s: &str) -> Result<u64, Error> {
    let s = s.trim();
    let (digits, multiplier) = if let Some(rest) = s.strip_suffix("GB") {
        (rest, 1024 * 1024 * 1024)
    } else if let Some(rest) = s.strip_suffix("MB") {
        (rest, 1024 * 1024)
    } else if let Some(rest) = s.strip_suffix("KB") {
        (rest, 1024)
    } else {
        (s, 1)
    };

    digits
        .trim()
        .parse::<u64>()
        .map(|n| n * multiplier)
        .map_err(|_| Error::Config(format!("not a size: {s} (try 128MB)")))
}

fn results() {
    println!("\n── Result<T, E> ──");

    for input in ["128MB", "64KB", "4096", "banana"] {
        match parse_size(input) {
            Ok(bytes) => println!("  {input:<8} → {bytes} bytes"),
            Err(e) => println!("  {input:<8} → error[{}]: {e}", e.code()),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// `?` — the operator that makes error handling bearable.
// ─────────────────────────────────────────────────────────────────────────────

/// Without `?` this would be two nested `match` blocks. With it, the happy path
/// reads straight down the page and every `?` is a possible early return.
fn block_count(size: &str, file_len: u64) -> Result<u64, Error> {
    let block_size = parse_size(size)?; //  ← if Err, return it to the caller
    if block_size == 0 {
        return Err(Error::Config("block size cannot be zero".into()));
    }
    Ok(file_len.div_ceil(block_size))
}

fn the_question_mark() {
    println!("\n── the ? operator ──");

    println!("  350 MB in 128MB blocks → {:?}", block_count("128MB", 350 * 1024 * 1024));
    println!("  350 MB in 'banana'     → {:?}", block_count("banana", 350 * 1024 * 1024).is_err());
    println!();
    println!("  `?` means: if Err, stop and hand it to my caller; if Ok, unwrap and continue.");
}

// ─────────────────────────────────────────────────────────────────────────────
// The error type itself. This is `mammoth_core::Error` in miniature.
// ─────────────────────────────────────────────────────────────────────────────

/// `thiserror` turns the `#[error(...)]` attributes into a `Display` impl, so
/// `{e}` prints the sentence you wrote and you never write `impl Display`.
#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("no such path: {0}")]
    NotFound(PathBuf),

    #[error("not enough healthy workers for replication {wanted}: only {available} available")]
    NotEnoughWorkers { wanted: u8, available: u8 },

    #[error("config error: {0}")]
    Config(String),
}

impl Error {
    /// A **stable** code. The wording of a message may change; `E0301` may not.
    /// Scripts match on this, and the docs URL is built from it.
    fn code(&self) -> &'static str {
        match self {
            Error::NotFound(_) => "E0101",
            Error::NotEnoughWorkers { .. } => "E0301",
            Error::Config(_) => "E0001",
        }
    }

    /// The part that makes an error *teach*: the next command to actually run.
    fn hints(&self) -> Vec<String> {
        match self {
            Error::NotEnoughWorkers { available, .. } => vec![
                format!("lower replication:   mammoth put <src> <dst> --replication {available}"),
                "check node health:   mammoth node list".into(),
            ],
            Error::Config(_) => vec!["validate config:     mammoth config validate".into()],
            Error::NotFound(_) => vec!["list the parent:     mammoth ls /".into()],
        }
    }
}

fn errors_that_teach() {
    println!("\n── errors that teach ──");

    // This is exactly what `mammoth_cli::output::print_error` prints.
    for e in [
        Error::NotEnoughWorkers { wanted: 3, available: 2 },
        Error::NotFound(PathBuf::from("/data/nope.txt")),
    ] {
        println!();
        println!("  error[{}]: {e}", e.code());
        println!();
        println!("  what you can do:");
        for hint in e.hints() {
            println!("    · {hint}");
        }
        println!();
        println!("  docs: https://projectorcha.github.io/Mammoth/errors/{}", e.code());
    }
    println!();
    println!("  Compare that with a Java stack trace. That difference is most of");
    println!("  why this project exists.");
}
