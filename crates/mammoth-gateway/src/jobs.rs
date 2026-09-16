//! Bounded text jobs submitted by the dashboard. History lasts for this service session.
use crate::{now_ms, Gateway};
use mammoth_compute::{run_with_output_policy, JobKind};
use mammoth_core::{Error, Result};
use serde_json::{json, Value};
use std::{collections::VecDeque, path::PathBuf, sync::Arc};
use tokio::sync::Mutex;

#[derive(Clone, Default)]
pub struct JobStore(Arc<Mutex<VecDeque<Value>>>);

impl JobStore {
    pub async fn wait_idle(&self) {
        while self.0.lock().await.iter().any(|job| job["state"] == "running") {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    }

    pub async fn list(&self) -> Vec<Value> {
        self.0
            .lock()
            .await
            .iter()
            .cloned()
            .map(|mut job| {
                if job["state"] == "running" {
                    job["elapsed_s"] =
                        json!((now_ms() - job["submitted"].as_i64().unwrap_or(0)) as f64 / 1000.0);
                }
                if let Some(task) = job["tasks"].as_array_mut().and_then(|tasks| tasks.first_mut())
                {
                    if task["state"] == "running" {
                        task["dur_s"] = json!(
                            (now_ms() - task["submitted"].as_i64().unwrap_or(0)) as f64 / 1000.0
                        );
                    }
                }
                job
            })
            .collect()
    }
}

pub async fn submit(
    state: &Gateway,
    kind: &str,
    input: PathBuf,
    output: PathBuf,
    overwrite: bool,
) -> Result<Value> {
    let kind = match kind {
        "wordcount" => JobKind::Wordcount,
        "sort" => JobKind::Sort,
        _ => return Err(Error::InvalidInput("choose wordcount or sort".into())),
    };
    let input = PathBuf::from(mammoth_core::path::normalize(&input)?);
    let output = PathBuf::from(mammoth_core::path::normalize(&output)?);
    if input == output {
        return Err(Error::InvalidInput("job output must differ from input".into()));
    }
    let source = state.backend.stat(&input).await?;
    if source.is_dir {
        return Err(Error::InvalidInput("select a UTF-8 file".into()));
    }
    match state.backend.stat(&output).await {
        Ok(target) if target.is_dir || !overwrite => return Err(Error::AlreadyExists(output)),
        Ok(_) | Err(Error::NotFound(_)) => {}
        Err(error) => return Err(error),
    }
    let options = state.dashboard.compute_options()?;
    let mut jobs = state.jobs.0.lock().await;
    if jobs.iter().filter(|job| job["state"] == "running").count() >= 2 {
        return Err(Error::InvalidInput(
            "two jobs are already running; wait for one to finish".into(),
        ));
    }
    if jobs.iter().any(|job| job["state"] == "running" && job["output"] == json!(output)) {
        return Err(Error::InvalidInput("another job is writing this output path".into()));
    }
    let id = format!(
        "job-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    let job = json!({"id":id,"name":format!("{} · {}", if matches!(kind, JobKind::Sort) {"Sort"} else {"Word count"}, input.display()),"kind":kind,"input":input,"output":output,"execution":"local","user":source.owner,"state":"running","submitted":now_ms(),"elapsed_s":0,"progress":0,"locality":1,"stages":[{"id":"local","name":"Read → process → write","kind":"local","deps":[],"tasks":1,"done":0}],"tasks":[{"id":"local-1","stage":"local","node":"local","state":"running","start_s":0,"dur_s":0,"submitted":now_ms(),"local":true}]});
    jobs.push_front(job.clone());
    while jobs.len() > 100 {
        if let Some(index) = jobs.iter().rposition(|job| job["state"] != "running") {
            jobs.remove(index);
        } else {
            break;
        }
    }
    drop(jobs);
    let state = state.clone();
    tokio::spawn(async move {
        let started = std::time::Instant::now();
        let result =
            run_with_output_policy(state.backend.as_ref(), input, output, kind, options, overwrite)
                .await;
        let mut jobs = state.jobs.0.lock().await;
        if let Some(job) = jobs.iter_mut().find(|job| job["id"] == id) {
            job["elapsed_s"] = json!(started.elapsed().as_secs_f64());
            job["state"] = json!(if result.is_ok() { "succeeded" } else { "failed" });
            job["progress"] = json!(if result.is_ok() { 1 } else { 0 });
            job["stages"][0]["done"] = json!(if result.is_ok() { 1 } else { 0 });
            job["tasks"][0]["state"] = json!(if result.is_ok() { "done" } else { "failed" });
            job["tasks"][0]["dur_s"] = json!(started.elapsed().as_secs_f64());
            match result {
                Ok(result) => job["metrics"] = json!(result.metrics),
                Err(error) => job["error"] = json!(error.to_string()),
            }
        }
    });
    Ok(job)
}
