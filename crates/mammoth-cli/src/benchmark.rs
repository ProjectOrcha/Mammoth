use crate::{cli::OutputFormat, output};
use mammoth_bench::{Options, Report};
use mammoth_core::Result;
use std::path::Path;

pub async fn run(
    root: &Path,
    options: Options,
    export: Option<&Path>,
    format: OutputFormat,
) -> Result<()> {
    let report = mammoth_bench::run(&root.join("benchmarks"), options).await?;
    if let Some(path) = export {
        mammoth_bench::write_report(path, &report)?;
    }
    show(&report, format)
}

fn show(report: &Report, format: OutputFormat) -> Result<()> {
    if format.resolve() != OutputFormat::Table {
        return output::emit(report, format);
    }
    output::chart(&format!("Benchmark · verified · {}\n{}\n{} measured iterations · {} excluded warmups · {} concurrent clients\n",report.id,report.scope,report.options.iterations,report.options.warmups,report.options.concurrency))?;
    output::chart(&format!(
        "Engine: {} · durable WAL · verified memory cache · parallel compute\n",
        report.environment["engine"].as_str().unwrap_or("unrecorded / historical")
    ))?;
    let rows: Vec<_> = report.summary.iter().map(|row| serde_json::json!({
        "phase":row.phase,"replicas":row.replication,"median":format!("{:.2}",row.median),
        "range":format!("{:.2}–{:.2}",row.min,row.max),"unit":row.unit,"p99 ms":format!("{:.3}",row.median_p99_ms)
    })).collect();
    output::emit(&rows, format)?;
    output::chart(&format!(
        "Read cache: {} bytes · Memory target per job: {} bytes\n",
        report.options.read_cache_size, report.options.compute_memory_budget
    ))?;
    let evidence: Vec<_> = report.samples.iter().filter(|sample| sample.cache.is_some() || sample.compute.is_some()).map(|sample| {
        let spilled: u64 = sample.compute.as_ref().and_then(|c| c["jobs"].as_array()).map(|jobs| jobs.iter().filter_map(|j| j["spilled_bytes"].as_u64()).sum()).unwrap_or(0);
        serde_json::json!({"phase":sample.phase,"replicas":sample.replication,"run":sample.iteration,"cache hits":sample.cache.as_ref().and_then(|c| c["hits"].as_u64()),"cache misses":sample.cache.as_ref().and_then(|c| c["misses"].as_u64()),"spilled bytes":spilled})
    }).collect();
    if !evidence.is_empty() {
        output::emit(&evidence, format)?;
    }
    output::chart("Read starts with an empty Mammoth cache; read_cached reuses it. Reads follow writes (OS cache retained). Metadata latencies are local backend calls.\nThis run measures Mammoth only. The separate measured Mac/HDFS/Spark comparison is documented in docs/BENCHMARKS.md. No physical network/shuffle measurements. Full JSON saved in <local-root>/benchmarks/reports/.")
}
