use clap::Parser;
#[derive(Parser)]
#[command(
    name = "mammoth-mcp",
    version,
    about = "Durable project context for coding agents over MCP stdio"
)]
struct Cli {
    #[arg(long, env = "MAMMOTH_LOCAL_ROOT")]
    local_root: Option<std::path::PathBuf>,
    #[command(flatten)]
    server: mammoth_mcp::McpArgs,
}
#[tokio::main]
async fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    match mammoth_mcp::serve(cli.local_root.unwrap_or_else(mammoth_mcp::default_root), cli.server)
        .await
    {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Mammoth MCP: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
