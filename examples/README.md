# Examples

Two kinds, for two different readers.

## Using Mammoth

Short, copy-paste demos of what the finished product does. No Rust required.

| | | |
| --- | --- | --- |
| [01-hello-mammoth](01-hello-mammoth/) | Put a file in, read it back | 2 min |
| [02-see-your-blocks](02-see-your-blocks/) | Draw a file's blocks and replicas | 5 min |
| [03-kill-a-node](03-kill-a-node/) | Stop a worker, watch the cluster heal | 10 min |
| [04-duckdb-over-s3](04-duckdb-over-s3/) | Query Mammoth from DuckDB, Spark, Polars | 10 min |
| [05-wordcount](05-wordcount/) | The distributed compute "hello world" | M7 |

## Building Mammoth

[**parts/**](parts/) — sixteen runnable Rust programs, one idea each: ownership,
traits, async, streams, the clap command tree, table-or-JSON output, colour, the
block matrix, progress bars, and a live TUI dashboard.

```bash
cargo run -q -p mammoth-parts --example 04-traits-and-dyn
```

These are the companion to [the build guide](../docs/guide/). Every chapter that
introduces a pattern has an example here that runs it in isolation.
