//! Command tree shared with generated documentation.
use clap::{ArgAction, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
pub const BANNER: &str = include_str!("../assets/banner.txt");
#[derive(Parser)]
#[command(
    name = "mammoth",
    version,
    about = "Durable storage with a local cluster, CLI, dashboard and S3 API"
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
    /// Version and build information.
    Version,
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
    },
    /// Download a file; existing destinations require --force.
    Get {
        src: PathBuf,
        dst: PathBuf,
        #[arg(long)]
        force: bool,
    },
    /// Write raw file bytes to stdout.
    Cat {
        path: PathBuf,
    },
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
    Mkdir {
        path: PathBuf,
        #[arg(short = 'p', long)]
        parents: bool,
    },
    Rm {
        path: PathBuf,
        #[arg(short = 'r', long)]
        recursive: bool,
    },
    Mv {
        src: PathBuf,
        dst: PathBuf,
    },
    Cp {
        src: PathBuf,
        dst: PathBuf,
        #[arg(short = 'r', long)]
        recursive: bool,
    },
    Stat {
        path: PathBuf,
    },
    Du {
        #[arg(default_value = "/")]
        path: PathBuf,
    },
    Df,
    Find {
        #[arg(default_value = "/")]
        path: PathBuf,
        #[arg(long)]
        name: Option<String>,
    },
    /// Set descriptive POSIX mode bits (local mode does not enforce ACLs).
    Chmod {
        mode: String,
        path: PathBuf,
    },
    Chown {
        owner: String,
        path: PathBuf,
    },
    Setrep {
        replication: u8,
        path: PathBuf,
    },
    /// Verify contents and display their CRC32C.
    Checksum {
        path: PathBuf,
    },
    Viz {
        #[command(subcommand)]
        what: VizCommand,
    },
    /// Interactive cluster dashboard. Press q to quit.
    Top {
        #[arg(long)]
        once: bool,
    },
    Node {
        #[command(subcommand)]
        command: NodeCommand,
    },
    Cluster {
        #[command(subcommand)]
        command: ClusterCommand,
    },
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
    /// Measure a local write/read round trip, then remove the benchmark file.
    Bench {
        #[arg(long, default_value = "8MiB")]
        size: String,
    },
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    /// Generate a shell completion script.
    Completions {
        shell: String,
    },
    /// Translate basic hdfs dfs commands into Mammoth commands.
    Compat {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}
#[derive(Subcommand)]
pub enum VizCommand {
    Blocks {
        path: PathBuf,
    },
    Cluster,
    Topology,
    Skew {
        path: Option<PathBuf>,
        #[arg(long)]
        by_partition: bool,
    },
    Treemap {
        path: Option<PathBuf>,
        #[arg(long, default_value_t = 2)]
        depth: u8,
    },
    Health {
        #[arg(long)]
        live: bool,
    },
    Flow,
}
#[derive(Subcommand)]
pub enum NodeCommand {
    List,
    Inspect { id: String },
    Repair,
}
#[derive(Subcommand)]
pub enum ClusterCommand {
    Status,
}
#[derive(Subcommand)]
pub enum AdminCommand {
    Report,
    Repair,
    Gc,
    Safemode,
}
#[derive(Subcommand)]
pub enum ConfigCommand {
    Show,
    Validate,
    Template,
}
#[derive(Subcommand)]
pub enum JobCommand {
    Wordcount { input: PathBuf, output_path: PathBuf },
    Sort { input: PathBuf, output_path: PathBuf },
}
#[derive(Subcommand)]
pub enum MigrateCommand {
    Import { source: PathBuf, destination: PathBuf },
    Export { source: PathBuf, destination: PathBuf },
}
