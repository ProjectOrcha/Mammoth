#!/usr/bin/env python3
"""Collect a reproducible single-host Linux result. Never substitute Mac scores."""
import argparse
import hashlib
import json
import os
import platform
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]


def capture(command):
    try:
        result = subprocess.run(command, cwd=ROOT, text=True, capture_output=True, timeout=30)
        return {"command": command, "exit_code": result.returncode,
                "stdout": result.stdout.strip(), "stderr": result.stderr.strip()}
    except (OSError, subprocess.TimeoutExpired) as error:
        return {"command": command, "error": str(error)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True, help="New directory on the disk to measure")
    parser.add_argument("--size", default="32MiB", help="Bytes per file; CLI bounds still apply")
    parser.add_argument("--files", type=int, default=8)
    parser.add_argument("--ops", type=int, default=10000)
    parser.add_argument("--concurrency", type=int, default=4)
    parser.add_argument("--iterations", type=int, default=5)
    parser.add_argument("--read-cache", default="256MiB")
    parser.add_argument("--compute-memory", default="32MiB")
    args = parser.parse_args()
    if platform.system() != "Linux":
        parser.error("Run this collector on Linux. No benchmark has been started.")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    sources = [ROOT / "Cargo.toml", ROOT / "Cargo.lock"]
    sources += sorted((ROOT / "crates").rglob("*.rs"))
    sources += sorted((ROOT / "crates").rglob("Cargo.toml"))
    record = {
        "scope": "single Linux host, six simulated worker directories; no distributed or Hadoop/Spark result",
        "timestamp_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "platform": platform.platform(), "cpu_count": os.cpu_count(),
        "revision": capture(["git", "rev-parse", "HEAD"]),
        "working_tree": capture(["git", "status", "--porcelain"]),
        "rust": capture(["rustc", "-Vv"]),
        "cpu": capture(["lscpu"]), "memory": capture(["free", "-b"]),
        "disks": capture(["lsblk", "-J", "-o", "NAME,TYPE,SIZE,ROTA,FSTYPE,MOUNTPOINTS"]),
        "filesystem": capture(["df", "-T", str(output)]),
        "load_before": capture(["uptime"]),
        "source_sha256": {str(path.relative_to(ROOT)): hashlib.sha256(path.read_bytes()).hexdigest() for path in sources},
        "cache_policy": "OS caches retained; read starts with empty Mammoth cache; read_cached reuses it; compute starts with empty Mammoth cache",
        "durability": "SQLite WAL synchronous=FULL; synced block data, headers and directory publication",
    }
    record_path = output / "environment.json"
    record_path.write_text(json.dumps(record, indent=2) + "\n")
    build_command = ["cargo", "build", "--release", "--locked", "-p", "mammoth-cli", "--message-format=json-render-diagnostics"]
    print("Building the release engine before measurement...", flush=True)
    build = subprocess.run(build_command, cwd=ROOT, text=True, capture_output=True)
    (output / "build.log").write_text(build.stderr + "\n" + build.stdout)
    record["build_command"] = build_command
    record["build_exit_code"] = build.returncode
    record["build_environment"] = {name: os.environ.get(name) for name in
                                   ["RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_TARGET", "CARGO_TARGET_DIR"]}
    record_path.write_text(json.dumps(record, indent=2) + "\n")
    if build.returncode:
        sys.exit(f"Release build failed; inspect {output / 'build.log'}. No benchmark was started.")
    artifacts = [json.loads(line) for line in build.stdout.splitlines() if line.startswith("{")]
    executables = [Path(item["executable"]) for item in artifacts
                   if item.get("reason") == "compiler-artifact" and item.get("target", {}).get("name") == "mammoth"
                   and item.get("executable")]
    if len(executables) != 1:
        sys.exit("Cannot identify the compiled Mammoth executable; no benchmark was started.")
    binary = executables[0].resolve()
    record["binary_sha256"] = hashlib.sha256(binary.read_bytes()).hexdigest()
    command = [str(binary), "--local-root", str(output / "store"),
               "bench", "suite", "--size", args.size, "--files", str(args.files),
               "--ops", str(args.ops), "--concurrency", str(args.concurrency),
               "--iterations", str(args.iterations), "--warmups", "1", "--replication", "1,3",
               "--read-cache", args.read_cache, "--compute-memory", args.compute_memory,
               "--block-size", "4MiB", "--seed", "42", "--report", str(output / "report.json"),
               "--output", "table"]
    record["command"] = command
    record_path.write_text(json.dumps(record, indent=2) + "\n")
    with (output / "run.log").open("w") as log:
        result = subprocess.run(command, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT)
    record["exit_code"] = result.returncode
    record["load_after"] = capture(["uptime"])
    record_path.write_text(json.dumps(record, indent=2) + "\n")
    if result.returncode:
        sys.exit(f"Benchmark failed; inspect {output / 'run.log'}. No successful result is claimed.")
    report = json.loads((output / "report.json").read_text())
    if not (report["verified"] and report["cleanup_complete"] and report["environment"]["os"] == "linux"
            and report["environment"].get("engine") == "local-memory-parallel-v3"):
        sys.exit("Report verification failed; do not publish this run.")
    print((output / "run.log").read_text())
    print(f"Verified Linux report and source/machine record: {output}")


if __name__ == "__main__":
    main()
