//! The `mammoth` binary.
//!
//! Design principles (Part V §5.1):
//!   1. verbs are POSIX, not Hadoop — `mammoth ls /data`, not `hdfs dfs -ls /data`
//!   2. everything has `--json`; human tables on a TTY, JSON when piped
//!   3. errors teach — what broke, why, and the next command to run
//!   4. progress bars on anything over a second, auto-disabled when piped
//!   5. `mammoth doctor` checks the things beginners get wrong

// Scaffold: the command tree is defined before its implementations.
#![allow(dead_code)]

mod cli;
mod commands;
mod output;

use clap::Parser;

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let cli = cli::Cli::parse();
    match run(cli).await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            output::print_error(&e);
            std::process::ExitCode::FAILURE
        }
    }
}

async fn run(cli: cli::Cli) -> mammoth_core::Result<()> {
    let root = cli.local_root.unwrap_or_else(mammoth_mcp::default_root);
    match cli.command {
        cli::Command::Memory(args) => {
            if !cli.masters.is_empty() {
                return Err(mammoth_core::Error::Config(
                    "memory uses --local-root, not --masters".into(),
                ));
            }
            let value = mammoth_mcp::run_memory(root, args)
                .await
                .map_err(|e| mammoth_core::Error::Config(e.to_string()))?;
            println!(
                "{}",
                serde_json::to_string_pretty(&value)
                    .map_err(|e| mammoth_core::Error::Config(e.to_string()))?
            );
            Ok(())
        }
        cli::Command::Mcp(args) => {
            if !cli.masters.is_empty() {
                return Err(mammoth_core::Error::Config(
                    "MCP uses --local-root, not --masters".into(),
                ));
            }
            mammoth_mcp::serve(root, args)
                .await
                .map_err(|e| mammoth_core::Error::Config(e.to_string()))
        }
        _ => Err(mammoth_core::Error::NotImplemented(
            "legacy storage command execution — use mammoth memory for agent context",
        )),
    }
}
