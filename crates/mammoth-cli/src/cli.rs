//! Command tree shared with generated documentation.
use clap::{ArgAction, CommandFactory, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
pub const BANNER: &str = include_str!("../assets/banner.txt");
#[derive(Parser)]
#[command(
    name = "mammoth",
    version,
    before_help = BANNER,
    about = "Durable context memory for coding agents, with a local MCP server"
)]
pub struct Cli {
    #[arg(short, long, env = "MAMMOTH_CONFIG", global = true)]
    pub config: Option<PathBuf>,
    /// HTTP gateway address for remote filesystem access.
    #[arg(long, env = "MAMMOTH_MASTERS", global = true, value_delimiter = ',')]
    pub masters: Vec<String>,
    /// Local store directory. Defaults to ~/.mammoth/local.
    #[arg(long, env = "MAMMOTH_LOCAL_ROOT", global = true)]
    pub local_root: Option<PathBuf>,
    #[arg(long, global = true, value_enum, default_value = "auto")]
    pub output: OutputFormat,
    #[arg(long, global = true, conflicts_with = "output")]
    pub json: bool,
    /// Color for human output; auto respects NO_COLOR and redirected output.
    #[arg(long, global = true, value_enum, default_value = "auto")]
    pub color: clap::ColorChoice,
    #[arg(short,long,global=true,action=ArgAction::Count)]
    pub verbose: u8,
    #[command(subcommand)]
    pub command: Command,
}
#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Auto,
    Table,
    Json,
    Yaml,
    Csv,
}
impl Cli {
    /// Root help includes the otherwise hidden nested commands from the same
    /// tree used to generate the website reference.
    pub fn help_command() -> clap::Command {
        let nested = Self::catalog()
            .into_iter()
            .filter(|(name, _)| name.contains(' '))
            .map(|(name, about)| format!("  {name:<24} {about}"))
            .collect::<Vec<_>>()
            .join("\n");
        Self::command().after_help(format!("Nested commands:\n{nested}\n\nExamples:\n  mammoth memory --project my-app recall\n  mammoth mcp --project my-app\n\nUse mammoth <command> --help for options, or mammoth commands for the full list.\nDocs: https://projectorcha.github.io/Mammoth/cli/"))
    }
    pub fn catalog() -> Vec<(String, String)> {
        fn walk(command: &clap::Command, prefix: &str, entries: &mut Vec<(String, String)>) {
            for child in command.get_subcommands().filter(|child| child.get_name() != "help") {
                let name = format!("{prefix}{}", child.get_name());
                let mut about = child.get_about().map(ToString::to_string).unwrap_or_default();
                let aliases = child.get_visible_aliases().collect::<Vec<_>>();
                if !aliases.is_empty() {
                    about += &format!(" (alias: {})", aliases.join(", "));
                }
                entries.push((name.clone(), about));
                walk(child, &format!("{name} "), entries);
            }
        }
        let mut entries = vec![];
        walk(&Self::command(), "", &mut entries);
        entries
    }
    pub fn format(&self) -> OutputFormat {
        if self.json {
            OutputFormat::Json
        } else {
            self.output
        }
    }
}
#[derive(Subcommand)]
pub enum Command {
    /// Save, recall, and manage durable project context (JSON output).
    Memory(mammoth_mcp::MemoryArgs),
    /// Serve project-scoped memory tools over MCP stdio.
    Mcp(mammoth_mcp::McpArgs),
    /// Version and build information.
    Version,
    /// Print the Mammoth terminal logo.
    Logo,
    /// List every command and subcommand with descriptions.
    Commands,
    /// Initialize the local store and write a starter config.
    Init,
    /// Start the local dashboard and S3 server with sample data.
    Quickstart {
        #[arg(long, default_value = "127.0.0.1:8080")]
        ui_listen: String,
        #[arg(long, default_value = "127.0.0.1:9000")]
        s3_listen: String,
        #[arg(long)]
        no_sample: bool,
        /// Allow non-loopback listeners in an isolated development network. No authentication.
        #[arg(long)]
        allow_remote: bool,
    },
    /// Run the local service in the foreground.
    Serve {
        #[arg(long,default_value="all",value_parser=["all","gateway","master","worker"])]
        role: String,
        #[arg(long)]
        ui_listen: Option<String>,
        #[arg(long)]
        s3_listen: Option<String>,
        /// Allow non-loopback listeners in an isolated development network. No authentication.
        #[arg(long)]
        allow_remote: bool,
    },
    /// Inspect the service running with this local store.
    Status,
    /// Stop this local store's dashboard and S3 service, retaining its files.
    #[command(visible_alias = "shutdown")]
    Stop {
        /// Seconds to wait for active requests and jobs to finish.
        #[arg(long, default_value_t = 30, value_parser = clap::value_parser!(u64).range(1..))]
        timeout: u64,
    },
    /// Print the configured dashboard address.
    Ui,
    /// Validate configuration and inspect local storage health.
    Doctor {
        #[arg(long)]
        fix: bool,
        #[arg(long)]
        node: Option<String>,
    },
    /// List direct children of a directory.
    Ls {
        #[arg(default_value = "/")]
        path: PathBuf,
    },
    /// Upload a local file; use - for stdin.
    Put {
        src: PathBuf,
        dst: PathBuf,
        #[arg(long)]
        replication: Option<u8>,
        #[arg(long)]
        block_size: Option<String>,
        /// Allow a source containing 0 bytes; otherwise leave the destination unchanged.
        #[arg(long)]
        allow_empty: bool,
    },
    /// Download a file; existing destinations require --force.
    Get {
        src: PathBuf,
        dst: PathBuf,
        #[arg(long)]
        force: bool,
    },
    /// Write raw file bytes to stdout.
    Cat { path: PathBuf },
    /// Print the final N lines.
    Tail {
        path: PathBuf,
        #[arg(short = 'n', long, default_value_t = 10)]
        lines: usize,
    },
    /// Print the first N lines.
    Head {
        path: PathBuf,
        #[arg(short = 'n', long, default_value_t = 10)]
        lines: usize,
    },
    /// Create a directory; -p also creates its parents.
    Mkdir {
        path: PathBuf,
        #[arg(short = 'p', long)]
        parents: bool,
    },
    /// Remove a file or directory; -r includes its descendants.
    Rm {
        path: PathBuf,
        #[arg(short = 'r', long)]
        recursive: bool,
    },
    /// Move or rename a stored path.
    Mv { src: PathBuf, dst: PathBuf },
    /// Copy a file or directory; -r copies a directory tree.
    Cp {
        src: PathBuf,
        dst: PathBuf,
        #[arg(short = 'r', long)]
        recursive: bool,
    },
    /// Inspect a path's size, permissions, checksum and storage layout.
    Stat { path: PathBuf },
    /// Total logical file bytes under a path.
    Du {
        #[arg(default_value = "/")]
        path: PathBuf,
    },
    /// Show stored replica bytes and worker capacities.
    Df,
    /// Find files and directories, optionally filtering names.
    Find {
        #[arg(default_value = "/")]
        path: PathBuf,
        #[arg(long)]
        name: Option<String>,
    },
    /// Set descriptive POSIX mode bits (local mode does not enforce ACLs).
    Chmod { mode: String, path: PathBuf },
    /// Set descriptive ownership as owner or owner:group.
    Chown { owner: String, path: PathBuf },
    /// Set the number of whole-file replicas.
    Setrep { replication: u8, path: PathBuf },
    /// Verify contents and display their CRC32C.
    Checksum { path: PathBuf },
    /// Terminal charts: blocks, capacity, topology, skew, size tree and health.
    Viz {
        #[command(subcommand)]
        what: VizCommand,
    },
    /// Interactive cluster dashboard. Press q to quit.
    Top {
        #[arg(long)]
        once: bool,
    },
    /// List and inspect worker directories, or repair their replicas.
    Node {
        #[command(subcommand)]
        command: NodeCommand,
    },
    /// Inspect cluster capacity and replica health.
    Cluster {
        #[command(subcommand)]
        command: ClusterCommand,
    },
    /// Report health, repair replicas, collect unused blocks and inspect safemode.
    Admin {
        #[command(subcommand)]
        command: AdminCommand,
    },
    /// Execute a local data-processing job.
    Job {
        #[command(subcommand)]
        command: JobCommand,
    },
    /// Import or export a directory tree.
    Migrate {
        #[command(subcommand)]
        command: MigrateCommand,
    },
    /// Run verified local I/O, metadata and parallel compute benchmarks.
    Bench {
        /// Workload to measure. All workers are simulated on this host.
        #[arg(value_enum, default_value = "suite")]
        workload: BenchWorkload,
        /// Bytes per I/O file. Inline storage is disabled for this benchmark.
        #[arg(long, default_value = "8MiB")]
        size: String,
        #[arg(long, default_value_t = 8)]
        files: usize,
        #[arg(long, default_value_t = 4)]
        concurrency: usize,
        /// Files per metadata phase: create, stat, rename and delete.
        #[arg(long, default_value_t = 200)]
        ops: usize,
        #[arg(long, default_value_t = 3)]
        iterations: usize,
        #[arg(long, default_value_t = 1)]
        warmups: usize,
        /// Compare replica counts in separate isolated stores.
        #[arg(long, value_delimiter = ',', default_value = "1,3")]
        replication: Vec<u8>,
        #[arg(long, default_value = "4MiB")]
        block_size: String,
        /// Verified read-cache capacity; 0 disables caching. Maximum 1 GiB.
        #[arg(long, default_value = "256MiB")]
        read_cache: String,
        /// Working-set target per compute job, with automatic disk spilling.
        #[arg(long, default_value = "32MiB")]
        compute_memory: String,
        #[arg(long, default_value_t = 42)]
        seed: u64,
        /// Also save the complete JSON report here. Reports are always kept under the local root.
        #[arg(long)]
        report: Option<PathBuf>,
    },
    /// Show, validate or generate a configuration.
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    /// Generate a shell completion script.
    Completions { shell: String },
    /// Translate basic hdfs dfs commands into Mammoth commands.
    Compat {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}
#[derive(Clone, Copy, clap::ValueEnum)]
pub enum BenchWorkload {
    Suite,
    Dfsio,
    Metadata,
    Compute,
}
#[derive(Subcommand)]
pub enum VizCommand {
    /// Block-by-worker matrix with primary, replica and corruption markers.
    Blocks { path: PathBuf },
    /// Worker capacity bars and health (also called heatmap).
    #[command(visible_alias = "heatmap")]
    Cluster,
    /// Rack and worker tree.
    Topology,
    /// File-size bars, median, p99 and maximum; optionally group by partition.
    Skew {
        path: Option<PathBuf>,
        #[arg(long)]
        by_partition: bool,
    },
    /// Namespace size tree with proportional bars and directory totals.
    Treemap {
        path: Option<PathBuf>,
        #[arg(long, default_value_t = 2)]
        depth: u8,
    },
    /// Replica-health bars; --live refreshes in place.
    Health {
        #[arg(long)]
        live: bool,
    },
    /// Report network-flow availability (unavailable in local mode).
    Flow,
}
#[derive(Subcommand)]
pub enum NodeCommand {
    /// List all workers and their capacities.
    List,
    /// Inspect one worker by ID, such as w1.
    Inspect { id: String },
    /// Restore damaged or missing replicas from verified copies.
    Repair,
}
#[derive(Subcommand)]
pub enum ClusterCommand {
    /// Show cluster capacity, workers and replica health.
    Status,
}
#[derive(Subcommand)]
pub enum AdminCommand {
    /// Show the cluster report.
    Report,
    /// Restore damaged or missing replicas from verified copies.
    Repair,
    /// Remove unreferenced block data.
    Gc,
    /// Inspect the current read-only safemode state.
    Safemode,
}
#[derive(Subcommand)]
pub enum ConfigCommand {
    /// Print the effective configuration.
    Show,
    /// Check configuration values.
    Validate,
    /// Print a starter TOML configuration.
    Template,
}
#[derive(Subcommand)]
pub enum JobCommand {
    /// Count UTF-8 words and save the counts to a stored file.
    Wordcount {
        input: PathBuf,
        output_path: PathBuf,
        /// Replace an existing result. Otherwise publication fails if the path exists.
        #[arg(long)]
        overwrite: bool,
    },
    /// Sort UTF-8 lines and save them to a stored file.
    Sort {
        input: PathBuf,
        output_path: PathBuf,
        /// Replace an existing result. Otherwise publication fails if the path exists.
        #[arg(long)]
        overwrite: bool,
    },
}
#[derive(Subcommand)]
pub enum MigrateCommand {
    /// Import a local file or directory into Mammoth.
    Import { source: PathBuf, destination: PathBuf },
    /// Export a stored file or directory to the local filesystem.
    Export { source: PathBuf, destination: PathBuf },
}
