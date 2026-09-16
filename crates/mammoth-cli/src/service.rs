//! Local service lifecycle. An OS-held lock identifies a live instance; stop
//! requests are scoped to that instance, never to a reusable process ID.
use crate::{
    cli::{OutputFormat, BANNER},
    output,
};
use mammoth_core::{Backend, Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::net::TcpListener;

#[derive(Serialize, Deserialize)]
struct Record {
    id: String,
    pid: u32,
    ui: String,
    s3: String,
}

pub struct Service {
    root: PathBuf,
    _lock: File,
    record: Record,
    ui: Option<TcpListener>,
    s3: Option<TcpListener>,
}

impl Service {
    pub async fn bind(root: &Path, ui: &str, s3: &str) -> Result<Self> {
        let root = root.canonicalize()?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(root.join("service.lock"))?;
        fs2::FileExt::try_lock_exclusive(&lock).map_err(|error| {
            if error.kind() == std::io::ErrorKind::WouldBlock {
                Error::InvalidInput(format!("a service is already running for {}; use mammoth --local-root {:?} status or stop", root.display(), root))
            } else { Error::Io(error) }
        })?;
        let ui = TcpListener::bind(ui).await?;
        let s3 = TcpListener::bind(s3).await?;
        match std::fs::remove_file(root.join("service.json")) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let record = Record {
            id: format!(
                "{}-{}",
                std::process::id(),
                SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos()
            ),
            pid: std::process::id(),
            ui: ui.local_addr()?.to_string(),
            s3: s3.local_addr()?.to_string(),
        };
        Ok(Self { root, _lock: lock, record, ui: Some(ui), s3: Some(s3) })
    }

    pub async fn run(
        mut self,
        backend: Arc<dyn Backend>,
        mut config: mammoth_core::config::Config,
        fmt: OutputFormat,
    ) -> Result<()> {
        let mut file = tempfile::NamedTempFile::new_in(&self.root)?;
        serde_json::to_writer(&mut file, &self.record)
            .map_err(|error| Error::InvalidInput(error.to_string()))?;
        file.flush()?;
        file.persist(self.root.join("service.json")).map_err(|error| Error::Io(error.error))?;
        if matches!(fmt, OutputFormat::Auto | OutputFormat::Table) {
            eprintln!(
                "{}",
                mammoth_viz::style::paint_error(BANNER, mammoth_viz::style::Tone::Accent)
            );
            eprintln!("Mammoth dashboard: http://{}\nS3 endpoint: http://{}\nStore: {}\nPress Ctrl-C, or run: mammoth --local-root {:?} stop", self.record.ui, self.record.s3, self.root.display(), self.root);
        } else {
            output::emit(&status(&self.root)?, fmt)?;
        }
        let request = stop_path(&self.root, &self.record)?;
        let shutdown = async move {
            tokio::select! {
                _ = mammoth_gateway::shutdown_signal() => {},
                _ = async {
                    let mut timer = tokio::time::interval(Duration::from_millis(100));
                    loop {
                        timer.tick().await;
                        if tokio::fs::try_exists(&request).await.unwrap_or(false) { break; }
                    }
                } => {}
            }
        };
        config.gateway.ui_listen = self.record.ui.clone();
        config.gateway.s3_listen = self.record.s3.clone();
        mammoth_gateway::serve_configured_with_shutdown(
            backend,
            self.ui.take().unwrap(),
            self.s3.take().unwrap(),
            mammoth_gateway::Dashboard::new(config, self.root.join("benchmarks")),
            shutdown,
        )
        .await
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        // Keep the lock file's inode in place so other processes cannot acquire
        // a different lock during cleanup. The OS releases this lock on exit.
        let _ = std::fs::remove_file(self.root.join("service.json"));
        if let Ok(path) = stop_path(&self.root, &self.record) {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn running(root: &Path) -> Result<bool> {
    let lock = match OpenOptions::new().read(true).write(true).open(root.join("service.lock")) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    match fs2::FileExt::try_lock_exclusive(&lock) {
        Ok(()) => Ok(false),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(true),
        Err(error) => Err(error.into()),
    }
}

fn record(root: &Path) -> Result<Record> {
    serde_json::from_slice(&std::fs::read(root.join("service.json"))?)
        .map_err(|error| Error::InvalidInput(format!("invalid service record: {error}")))
}

fn stop_path(root: &Path, record: &Record) -> Result<PathBuf> {
    if record.id.is_empty() || !record.id.bytes().all(|byte| byte.is_ascii_digit() || byte == b'-')
    {
        return Err(Error::InvalidInput("invalid service instance ID".into()));
    }
    Ok(root.join(format!("service-stop-{}", record.id)))
}

pub fn status(root: &Path) -> Result<Value> {
    if !running(root)? {
        return Ok(json!({"state":"stopped", "root":root}));
    }
    match record(root) {
        Ok(record) => Ok(
            json!({"state":"running", "root":root, "pid":record.pid, "ui":format!("http://{}", record.ui), "s3":format!("http://{}", record.s3)}),
        ),
        Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(json!({"state":"starting", "root":root}))
        }
        Err(error) => Err(error),
    }
}

pub async fn stop(root: &Path, seconds: u64) -> Result<Value> {
    if !running(root)? {
        return Ok(json!({"state":"stopped", "root":root, "already_stopped":true}));
    }
    let stop = async {
        let record = loop {
            match record(root) {
                Ok(record) => break Some(record),
                Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                    if !running(root)? {
                        break None;
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Err(error) => return Err(error),
            }
        };
        if let Some(record) = record {
            let request = stop_path(root, &record)?;
            File::create(&request)?;
            while running(root)? {
                if let Ok(next) = self::record(root) {
                    if next.id != record.id {
                        break;
                    }
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            let _ = std::fs::remove_file(request);
        }
        Ok(json!({"state":"stopped", "root":root, "already_stopped":false}))
    };
    tokio::time::timeout(Duration::from_secs(seconds), stop).await
        .map_err(|_| Error::InvalidInput(format!("service has not stopped after {seconds}s; the stop request remains pending while active work finishes. Check status or retry with --timeout.")))?
}
