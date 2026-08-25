//! Command tree (Part V §5.2). Kept in its own module so `xtask docs` can
//! reflect over it with `clap_markdown` and regenerate the CLI reference.

use std::path::PathBuf;

use clap::{ArgAction, Parser, Subcommand, ValueEnum};

/// Banner shown by `mammoth quickstart` and `mammoth --version --verbose`.
pub const BANNER: &str = include_str!("../assets/banner.txt");

#[derive(Parser)]
#[command(
    name = "mammoth",
    version,
    about = "Distributed storage that doesn't need a JVM",
    long_about = None,
)]
pub struct Cli {
    /// Path to mammoth.toml.
    #[arg(short, long, env = "MAMMOTH_CONFIG", global = true)]
    pub config: Option<PathBuf>,

    /// Master addresses, comma separated.
    #[arg(long, env = "MAMMOTH_MASTERS", global = true, value_delimiter = ',')]
    pub masters: Vec<String>,

    /// Output format.
    #[arg(long, global = true, value_enum, default_value = "auto")]
    pub output: OutputFormat,

    /// Repeat for more detail: -v, -vv, -vvv.
    #[arg(short, long, global = true, action = ArgAction::Count)]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: Command,
}

/// `auto` resolves to `table` on a TTY and `json` when piped.
#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    /// Table on a terminal, JSON when piped.
    Auto,
    /// Always a human-readable table.
    Table,
    /// Always JSON.
    Json,
    /// Always YAML.
    Yaml,
    /// Always CSV.
    Csv,
}

#[derive(Subcommand)]
pub enum Command {
    // --- lifecycle ---
    /// Create a new cluster: config, IDs, certs.
    Init,
    /// One-command demo cluster with sample data, then open the UI.
    Quickstart,
    /// Run a node.
    Serve {
        /// master | worker | gateway | all
        #[arg(long)]
        role: String,
    },
    /// Launch or open the web GUI.
    Ui,
    /// Diagnose config, ports, disks, clock and ulimits.
    Doctor {
        /// Apply the fixes that are safe to apply automatically.
        #[arg(long)]
        fix: bool,
    },

    // --- filesystem ---
    /// List a directory.
    Ls,
    /// Upload a local file.
    Put,
    /// Download to a local file.
    Get,
    /// Stream a file to stdout.
    Cat,
    /// Last lines of a file.
    Tail,
    /// First lines of a file.
    Head,
    /// Create a directory.
    Mkdir,
    /// Remove a path.
    Rm,
    /// Move or rename.
    Mv,
    /// Copy within the cluster.
    Cp,
    /// Metadata for one path.
    Stat,
    /// Disk usage by path.
    Du,
    /// Cluster capacity summary.
    Df,
    /// Search the namespace.
    Find,
    /// Change mode bits.
    Chmod,
    /// Change owner or group.
    Chown,
    /// Change the replication factor.
    Setrep,
    /// Print or verify a file checksum.
    Checksum,

    /// Visualize how data is spread across the cluster.
    Viz {
        #[command(subcommand)]
        what: VizCommand,
    },
    /// Live TUI dashboard — htop for your cluster.
    Top,

    // --- operations ---
    /// Inspect and manage workers.
    Node,
    /// Raft membership and leadership.
    Cluster,
    /// Cluster administration.
    Admin,
    /// Submit and inspect jobs.
    Job,
    /// Migrate data in, and upgrade Mammoth itself.
    Migrate,
    /// Built-in benchmarks.
    Bench,
    /// Inspect and validate configuration.
    Config,
    /// Manage auth tokens.
    Token,
    /// Translate old Hadoop invocations: `mammoth compat hdfs dfs -ls /`.
    Compat,
    /// Generate a shell completion script.
    Completions {
        /// bash | zsh | fish | powershell
        shell: String,
    },
}

/// `mammoth viz …` — the feature that makes Mammoth feel different (Part VII).
#[derive(Subcommand)]
pub enum VizCommand {
    /// Where this file's blocks live, as a block × node matrix.
    Blocks {
        /// Path to inspect.
        path: PathBuf,
    },
    /// Per-node capacity heatmap, grouped by rack.
    Cluster,
    /// The rack / zone tree.
    Topology,
    /// Hotspots and partition-size imbalance.
    Skew {
        /// Directory to analyze.
        path: Option<PathBuf>,
        /// Group by partition directory rather than by file.
        #[arg(long)]
        by_partition: bool,
    },
    /// Which directories are eating the space.
    Treemap {
        /// Root of the treemap.
        path: Option<PathBuf>,
        /// How many levels to descend.
        #[arg(long, default_value_t = 2)]
        depth: u8,
    },
    /// Replication health, optionally refreshing live.
    Health {
        /// Refresh every 2s until interrupted.
        #[arg(long)]
        live: bool,
    },
    /// Live data movement between clients, replication, balancer and shuffle.
    Flow,
}
