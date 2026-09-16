//! Local processing and directory migration commands.
use crate::{
    cli::{JobCommand, MigrateCommand, OutputFormat},
    output,
};
use mammoth_core::{Backend, Result};

pub async fn job(
    be: &dyn Backend,
    command: JobCommand,
    config: &mammoth_core::config::Compute,
    fmt: OutputFormat,
) -> Result<()> {
    let (input, output_path, kind, overwrite) = match command {
        JobCommand::Wordcount { input, output_path, overwrite } => {
            (input, output_path, mammoth_compute::JobKind::Wordcount, overwrite)
        }
        JobCommand::Sort { input, output_path, overwrite } => {
            (input, output_path, mammoth_compute::JobKind::Sort, overwrite)
        }
    };
    output::emit(
        &mammoth_compute::run_with_output_policy(
            be,
            input,
            output_path,
            kind,
            mammoth_compute::Options::from_config(config)?,
            overwrite,
        )
        .await?,
        fmt,
    )
}

pub async fn migrate(be: &dyn Backend, command: MigrateCommand, fmt: OutputFormat) -> Result<()> {
    let transfer = match command {
        MigrateCommand::Import { source, destination } => {
            mammoth_migrate::Transfer::Import { source, destination }
        }
        MigrateCommand::Export { source, destination } => {
            mammoth_migrate::Transfer::Export { source, destination }
        }
    };
    output::emit(&mammoth_migrate::transfer(be, transfer).await?, fmt)
}
