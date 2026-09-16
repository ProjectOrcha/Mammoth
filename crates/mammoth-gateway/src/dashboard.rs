//! Benchmark history and configuration drafts for a local service.
use mammoth_bench::Options;
use mammoth_core::{config::Config, Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::Mutex;

#[derive(Clone, Default)]
pub struct Dashboard {
    config: Option<Config>,
    directory: Option<PathBuf>,
    active: Arc<Mutex<Option<Value>>>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub block_size: String,
    pub inline_threshold: String,
    pub replication: u8,
    pub ui_listen: String,
    pub s3_listen: String,
    #[serde(default = "default_read_cache")]
    pub read_cache_size: String,
    #[serde(default = "default_compute_memory")]
    pub compute_memory_budget: String,
    #[serde(default)]
    pub spill_directory: String,
}
fn default_read_cache() -> String {
    Config::default().read.cache_size
}
fn default_compute_memory() -> String {
    Config::default().compute.memory_budget
}
impl Dashboard {
    pub fn compute_options(&self) -> Result<mammoth_compute::Options> {
        mammoth_compute::Options::from_config(
            &self.config.as_ref().map(|c| c.compute.clone()).unwrap_or_default(),
        )
    }

    pub fn new(config: Config, directory: PathBuf) -> Self {
        Self { config: Some(config), directory: Some(directory), ..Self::default() }
    }
    pub fn available(&self) -> bool {
        self.directory.is_some()
    }
    pub async fn list(&self) -> Result<Value> {
        let directory = self
            .directory
            .clone()
            .ok_or(Error::NotImplemented("benchmark storage for this gateway"))?;
        let reports = tokio::task::spawn_blocking(move || mammoth_bench::history(&directory))
            .await
            .map_err(|e| Error::Io(std::io::Error::other(e)))??;
        Ok(
            json!({"active":self.active.lock().await.clone(),"reports":reports,"defaults":Options::default()}),
        )
    }
    pub async fn submit(&self, options: Options) -> Result<Value> {
        options.validate()?;
        let directory = self
            .directory
            .clone()
            .ok_or(Error::NotImplemented("benchmark storage for this gateway"))?;
        let mut active = self.active.lock().await;
        if active.as_ref().is_some_and(|v| v["state"] == "running") {
            return Err(Error::InvalidInput("a benchmark is already running".into()));
        }
        let lock = mammoth_bench::acquire(&directory)?;
        let value = json!({"state":"running","started_ms":crate::now_ms(),"options":options});
        *active = Some(value.clone());
        let state = self.clone();
        tokio::spawn(async move {
            let result = mammoth_bench::run_locked(&directory, options, lock).await;
            let value = match result {
                Ok(report) => json!({"state":"succeeded","report_id":report.id}),
                Err(error) => json!({"state":"failed","error":error.to_string()}),
            };
            *state.active.lock().await = Some(value);
        });
        Ok(value)
    }
    pub async fn wait_idle(&self) {
        while self.active.lock().await.as_ref().is_some_and(|v| v["state"] == "running") {
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    }
    pub fn configuration(&self, draft: Option<Settings>) -> Result<Value> {
        let mut config = self
            .config
            .clone()
            .ok_or(Error::NotImplemented("active configuration for this gateway"))?;
        if let Some(settings) = draft {
            config.storage.block_size = settings.block_size;
            config.storage.inline_threshold = settings.inline_threshold;
            config.storage.replication = settings.replication;
            config.gateway.ui_listen = settings.ui_listen;
            config.gateway.s3_listen = settings.s3_listen;
            config.read.cache_size = settings.read_cache_size;
            config.compute.memory_budget = settings.compute_memory_budget;
            config.compute.spill_directory = settings.spill_directory;
        }
        config.validate_local_service()?;
        if config.gateway.ui_listen == config.gateway.s3_listen
            && !config.gateway.ui_listen.ends_with(":0")
        {
            return Err(Error::Config(
                "dashboard and S3 must use different listener addresses".into(),
            ));
        }
        let settings = Settings {
            block_size: config.storage.block_size.clone(),
            inline_threshold: config.storage.inline_threshold.clone(),
            replication: config.storage.replication,
            ui_listen: config.gateway.ui_listen.clone(),
            s3_listen: config.gateway.s3_listen.clone(),
            read_cache_size: config.read.cache_size.clone(),
            compute_memory_budget: config.compute.memory_budget.clone(),
            spill_directory: config.compute.spill_directory.clone(),
        };
        // Export only settings the local service applies. Configured roadmap fields
        // such as TLS/auth are not represented as active capabilities.
        let toml = toml::to_string_pretty(&json!({"storage":{"block_size":settings.block_size,"inline_threshold":settings.inline_threshold,"replication":settings.replication},"gateway":{"ui_listen":settings.ui_listen,"s3_listen":settings.s3_listen},"read":{"cache_size":settings.read_cache_size},"compute":{"memory_budget":settings.compute_memory_budget,"spill_directory":settings.spill_directory}})).map_err(|e|Error::Config(e.to_string()))?;
        Ok(json!({"settings":settings,"toml":toml,"restart_required":true,
            "notes":["Download the validated file, then restart with mammoth --config /path/to/mammoth.toml --local-root /path/to/store serve --role all.","Storage settings apply to new writes. Existing files keep their layout.","Benchmarks have independent settings and use isolated temporary stores.","TLS, authentication, physical worker placement and distributed services are not implemented by the local service."]}))
    }
}
