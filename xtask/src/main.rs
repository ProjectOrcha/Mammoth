//! `cargo xtask <task>` — repository automation with no extra tooling to install.
//!
//! Tasks:
//!   build-ui   npm ci && npm run build in ui/, so rust-embed has something to embed
//!   docs       regenerate web/src/content/docs/cli/reference.md from the clap tree
//!   assets     copy assets/logo/* into ui/public/ and web/public/
//!   dist       cargo dist build
//!
//! `docs` runs in CI and the build fails if the committed reference differs —
//! that is what keeps the CLI documentation from drifting (Part XIII).

use std::process::ExitCode;

fn main() -> ExitCode {
    let task = std::env::args().nth(1);
    match task.as_deref() {
        Some("build-ui") => todo!("shell out to npm in ui/ — milestone M3"),
        Some("docs") => todo!("clap_markdown::help_markdown::<mammoth_cli::Cli>() — M2"),
        Some("assets") => todo!("copy assets/logo/* into ui/public and web/public"),
        Some("dist") => todo!("cargo dist build"),
        Some(other) => {
            eprintln!("unknown task: {other}");
            usage();
            ExitCode::FAILURE
        }
        None => {
            usage();
            ExitCode::FAILURE
        }
    }
}

fn usage() {
    eprintln!("usage: cargo xtask <build-ui|docs|assets|dist>");
}
