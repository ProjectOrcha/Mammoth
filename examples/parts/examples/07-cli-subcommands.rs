//! 07 · A real command tree: subcommands, nested subcommands, global flags.
//!
//!     cargo run -p mammoth-parts --example 07-cli-subcommands -- --help
//!     cargo run -p mammoth-parts --example 07-cli-subcommands -- ls /data --long
//!     cargo run -p mammoth-parts --example 07-cli-subcommands -- put ./big.log /data/big.log --block-size 64MB
//!     cargo run -p mammoth-parts --example 07-cli-subcommands -- viz --help
//!     cargo run -p mammoth-parts --example 07-cli-subcommands -- viz blocks /data/big.log
//!     cargo run -p mammoth-parts --example 07-cli-subcommands -- --json stat /data/big.log
//!
//! This is `crates/mammoth-cli/src/cli.rs` with twenty commands removed. Every
//! pattern in the real file is here: a global flag, an enum of subcommands, an
//! args struct per command, a nested subcommand, and the `match` that
//! dispatches. Chapter 7 of the guide wires the same shape to a real backend.

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

// ─────────────────────────────────────────────────────────────────────────────
// ① The root. Global flags live here.
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(version, about = "Distributed storage that doesn't need a JVM")]
struct Cli {
    /// Output format. `global = true` means it is accepted *after* any
    /// subcommand too: `mammoth ls /data --json` as well as `mammoth --json ls /data`.
    #[arg(long, global = true, value_enum, default_value = "auto")]
    output: OutputFormat,

    /// A shorthand that conflicts with the long form, so `--json --output table`
    /// is rejected rather than silently picking one.
    #[arg(long, global = true, conflicts_with = "output")]
    json: bool,

    /// `env` means this can also come from an environment variable.
    #[arg(short, long, global = true, env = "MAMMOTH_CONFIG")]
    config: Option<PathBuf>,

    /// Repeat for more detail: -v, -vv, -vvv. `ArgAction::Count` counts them.
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    verbose: u8,

    /// The subcommand itself. Required — running with no command prints help.
    #[command(subcommand)]
    command: Command,
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum OutputFormat {
    /// Table on a terminal, JSON when piped.
    Auto,
    /// Always a human-readable table.
    Table,
    /// Always JSON.
    Json,
}

// ─────────────────────────────────────────────────────────────────────────────
// ② The commands. One variant per verb; the doc comment is the help text.
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Subcommand)]
enum Command {
    /// List a directory.
    Ls(LsArgs),
    /// Upload a local file.
    Put(PutArgs),
    /// Stream a file to stdout.
    Cat(PathArgs),
    /// Metadata for one path.
    Stat(PathArgs),
    /// Visualize how data is spread across the cluster.
    Viz {
        /// A subcommand inside a subcommand. `mammoth viz blocks /x`.
        #[command(subcommand)]
        what: VizCommand,
    },
}

/// Arguments get their own struct when there is more than one of them. The
/// struct keeps `Command` readable and lets a command's arguments be passed
/// around as one value.
#[derive(clap::Args)]
struct LsArgs {
    /// Directory to list.
    #[arg(default_value = "/")]
    path: PathBuf,
    /// Long format: permissions, owner, size, replication, blocks.
    #[arg(short, long)]
    long: bool,
}

#[derive(clap::Args)]
struct PutArgs {
    /// Local file to upload. Positional, and required.
    src: PathBuf,
    /// Destination path inside the cluster. Positional, second.
    dst: PathBuf,
    /// Override the block size for this file, e.g. `512MB`.
    #[arg(long)]
    block_size: Option<String>,
    /// Override the replication factor for this file.
    #[arg(long, value_parser = clap::value_parser!(u8).range(1..=10))]
    replication: Option<u8>,
}

/// One struct, reused by every command that takes a single path. `Cat(PathArgs)`
/// and `Stat(PathArgs)` share it — do this rather than repeating yourself once
/// you have three such commands.
#[derive(clap::Args)]
struct PathArgs {
    /// Path inside the cluster.
    path: PathBuf,
}

#[derive(Subcommand)]
enum VizCommand {
    /// Where this file's blocks live, as a block × node matrix.
    Blocks {
        /// Path to inspect.
        path: PathBuf,
    },
    /// Per-node capacity heatmap, grouped by rack.
    Cluster,
}

// ─────────────────────────────────────────────────────────────────────────────
// ③ Dispatch. In the real CLI every arm calls into `commands::` and hands it a
//    `&dyn Backend`. Here they just print what they would have done.
// ─────────────────────────────────────────────────────────────────────────────

fn main() {
    let cli = Cli::parse();

    let fmt = if cli.json { OutputFormat::Json } else { cli.output };
    let fmt_name = match fmt {
        OutputFormat::Auto => "auto",
        OutputFormat::Table => "table",
        OutputFormat::Json => "json",
    };

    println!();
    println!("  globals   output={fmt_name}  verbose={}  config={:?}", cli.verbose, cli.config);
    println!();

    match cli.command {
        Command::Ls(a) => {
            println!("  → backend.list({})   long={}", a.path.display(), a.long);
        }
        Command::Put(a) => {
            println!("  → backend.write({})", a.dst.display());
            println!("      from        {}", a.src.display());
            println!("      block_size  {}", a.block_size.as_deref().unwrap_or("(default)"));
            println!(
                "      replication {}",
                a.replication.map(|r| r.to_string()).unwrap_or_else(|| "(default)".into())
            );
        }
        Command::Cat(a) => println!("  → backend.read({}, 0..EOF)", a.path.display()),
        Command::Stat(a) => println!("  → backend.stat({})", a.path.display()),
        Command::Viz { what } => match what {
            VizCommand::Blocks { path } => {
                println!("  → backend.block_layout({})", path.display())
            }
            VizCommand::Cluster => println!("  → backend.cluster_report()"),
        },
    }

    println!();
    println!("  Every arm above maps to exactly one `Backend` method. That is not a");
    println!("  coincidence — it is what makes the CLI portable to a real cluster.");
}
