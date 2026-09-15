//! Local processing and directory migration commands.
use crate::{
    cli::{JobCommand, MigrateCommand, OutputFormat},
    output,
};
use mammoth_core::{Backend, Result};

pub async fn job(be: &dyn Backend, command: JobCommand, fmt: OutputFormat) -> Result<()> {
    let (input, output_path, kind) = match command {
        JobCommand::Wordcount { input, output_path } => {
            (input, output_path, mammoth_compute::JobKind::Wordcount)
        }
        JobCommand::Sort { input, output_path } => {
            (input, output_path, mammoth_compute::JobKind::Sort)
        }
    };
    output::emit(&mammoth_compute::run_local(be, input, output_path, kind).await?, fmt)
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
