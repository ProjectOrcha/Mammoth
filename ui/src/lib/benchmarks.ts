export interface BenchmarkOptions {
  workload: 'suite' | 'dfsio' | 'metadata' | 'compute';
  read_cache_size: number; compute_memory_budget: number;
  file_size: number; files: number; concurrency: number; operations: number;
  iterations: number; warmups: number; replications: number[]; block_size: number; seed: number;
}
export const benchmarkDefaults: BenchmarkOptions = {
  read_cache_size: 256 * 1048576, compute_memory_budget: 32 * 1048576,
  workload: 'suite', file_size: 8 * 1048576, files: 8, concurrency: 4, operations: 200,
  iterations: 3, warmups: 1, replications: [1, 3], block_size: 4 * 1048576, seed: 42,
};
export interface BenchmarkSummary {
  phase: string; replication: number; unit: string; median: number; min: number; max: number; median_p99_ms: number;
}
export interface BenchmarkReport {
  schema_version: number; id: string; timestamp_ms: number; scope: string;
  environment: { engine?: string; os: string; arch: string; profile: string; logical_cpus: number; data_directory: string };
  options: BenchmarkOptions; summary: BenchmarkSummary[]; samples: { phase: string; replication: number; iteration: number;
    cache?: { hits: number; misses: number; resident_bytes: number; capacity_bytes: number };
    compute?: { jobs: import('./types').ComputeMetrics[] }; [key: string]: unknown }[];
  verified: boolean; cleanup_complete: boolean; notes: string[];
}
export interface BenchmarkState {
  active: { state: 'running' | 'succeeded' | 'failed'; error?: string; report_id?: string; started_ms?: number } | null;
  reports: BenchmarkReport[]; defaults: BenchmarkOptions;
}
export interface StorageSettings {
  read_cache_size: string; compute_memory_budget: string; spill_directory: string;
  block_size: string; inline_threshold: string; replication: number; ui_listen: string; s3_listen: string;
}
export interface Configuration {
  settings: StorageSettings; toml: string; restart_required: boolean; notes: string[];
}
export function downloadText(name: string, content: string, type = 'application/json') {
  const url = URL.createObjectURL(new Blob([content], { type }));
  const link = document.createElement('a'); link.href = url; link.download = name;
  link.click(); setTimeout(() => URL.revokeObjectURL(url), 1000);
}
