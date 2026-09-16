# Client compatibility

The implemented check starts an isolated Mammoth service and exercises its S3
subset with real Boto3 and DuckDB clients. It verifies uploaded/downloaded bytes,
ranges, conditions, listing, copy/delete, DuckDB reads and clean shutdown.

```bash
cargo build --locked -p mammoth-cli
uv run --no-project --with boto3 --with duckdb python tests/compat/s3_clients.py
```

Use `COMPAT_MAMMOTH_BINARY=target/release/mammoth` to test a release build.
The script uses temporary data and random loopback ports. It does not validate
SigV4 authentication, multipart upload or complete AWS S3 compatibility.

Hadoop WebHDFS parity, composite checksums and migration cutover tests remain
future work. The [same-Mac benchmark harness](../../bench-suite/compare/README.md)
measures HDFS and Spark with their own native clients; those measurements do not
prove protocol compatibility with Mammoth.
