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

fn main() -> std::process::ExitCode {
    let cli = cli::Cli::parse();
    match run(cli) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            output::print_error(&e);
            std::process::ExitCode::FAILURE
        }
    }
}

fn run(_cli: cli::Cli) -> mammoth_core::Result<()> {
    // M1: wire each subcommand to a `Backend` (LocalBackend first).
    // Every command takes `&dyn Backend`, so none of them change when
    // `ClusterBackend` lands in M5.
    unimplemented!("command dispatch — see docs/ROADMAP.md, milestone M1")
}
