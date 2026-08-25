# Chapter 8 — `viz blocks`: seeing your data

**What you'll build:** the feature that makes this project worth building.

**Time:** about 2 hours.

---

Hadoop's web UI shows you tables of numbers. It will tell you a file has 3
blocks and replication 3. It will not show you, at a glance, that all three
replicas of block 1 are sitting in the same rack and one power failure away from
gone.

`mammoth viz blocks` does. It is the moment this project stops feeling like
homework, and it is 150 lines.

## Step 1 · The drawing primitives

Drawing belongs in `mammoth-viz`, away from anything that does I/O. That makes
it trivially testable — which matters, because "the bar is one cell too wide" is
exactly the kind of bug that is invisible until a demo.

Replace `crates/mammoth-viz/src/lib.rs`:

```rust
//! Terminal charts: the drawing primitives behind `mammoth viz` and `mammoth top`.
//!
//! Nothing here does I/O or knows about a `Backend`. It turns numbers into
//! strings, which makes all of it trivially testable.

#![forbid(unsafe_code)]

use mammoth_core::types::ReplicaState;

/// Eighth-blocks give 8x the resolution of a plain `#` bar.
const EIGHTHS: [char; 8] = ['▏', '▎', '▍', '▌', '▋', '▊', '▉', '█'];

/// A proportional bar, `width` cells wide, at 1/8-cell resolution.
///
/// ```
/// use mammoth_viz::bar;
/// assert_eq!(bar(1.0, 4), "████");
/// assert_eq!(bar(0.0, 4), "░░░░");
/// assert_eq!(bar(0.5, 4), "██░░");
/// ```
pub fn bar(fraction: f64, width: usize) -> String {
    let f = fraction.clamp(0.0, 1.0);
    let total_eighths = (f * width as f64 * 8.0).round() as usize;
    let full = total_eighths / 8;
    let rem = total_eighths % 8;

    let mut s = String::with_capacity(width * 3);
    for _ in 0..full.min(width) {
        s.push('█');
    }
    let mut used = full.min(width);
    if rem > 0 && used < width {
        s.push(EIGHTHS[rem - 1]);
        used += 1;
    }
    for _ in used..width {
        s.push('░');
    }
    s
}

/// The symbol for one replica in the block matrix.
pub fn replica_symbol(state: ReplicaState) -> char {
    match state {
        ReplicaState::Primary => '●',
        ReplicaState::Replica => '◐',
        ReplicaState::Corrupt => '✕',
    }
}

/// Shown under every block matrix.
pub const LEGEND: &str = "● primary   ◐ replica   ✕ corrupt   · absent";

/// Population standard deviation, as a percentage. The imbalance metric.
pub fn std_dev_pct(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let var = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
    var.sqrt() * 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_is_always_exactly_width_cells() {
        for pct in 0..=100 {
            let b = bar(pct as f64 / 100.0, 16);
            assert_eq!(b.chars().count(), 16, "at {pct}%");
        }
    }

    #[test]
    fn bar_clamps_out_of_range() {
        assert_eq!(bar(-1.0, 4), "░░░░");
        assert_eq!(bar(2.0, 4), "████");
    }

    #[test]
    fn std_dev_of_identical_values_is_zero() {
        assert_eq!(std_dev_pct(&[0.5, 0.5, 0.5]), 0.0);
    }
}
```

```bash
cargo test -p mammoth-viz
```

```
running 3 tests
test tests::bar_clamps_out_of_range ... ok
test tests::std_dev_of_identical_values_is_zero ... ok
test tests::bar_is_always_exactly_width_cells ... ok

   Doc-tests mammoth_viz
test crates/mammoth-viz/src/lib.rs - bar (line 15) ... ok
```

### Why eighth-blocks

A bar drawn with `#` characters has a resolution of one cell. At 16 cells wide,
71% and 75% look identical. Unicode gives you `▏▎▍▌▋▊▉█` — eight widths in one
cell — so the same bar resolves to about 0.8%. Free precision.

### Why `bar_is_always_exactly_width_cells` matters

That test loops over every percentage from 0 to 100 and asserts the result is
exactly 16 characters. Without it, an off-by-one at some specific percentage
makes your table columns jump around, and you will not notice until someone is
watching.

Note it counts `.chars()`, not `.len()`. `█` is three bytes in UTF-8, so
`.len()` would be wrong.

### The doctest

The `/// ```` block on `bar` is a **doctest** — Rust compiles and runs it as
part of `cargo test`. Documentation that cannot go stale, because the build
fails if it does.

## Step 2 · The block matrix

Create `crates/mammoth-cli/src/commands/viz.rs`:

```rust
//! `mammoth viz` — where your data actually is.

use std::collections::BTreeMap;
use std::path::Path;

use mammoth_core::types::{BlockPlacement, ClusterReport};
use mammoth_core::{Backend, Result};
use mammoth_viz::{bar, replica_symbol, std_dev_pct, LEGEND};

use crate::commands::fs::human;

/// `mammoth viz blocks <path>` — the block x node matrix.
pub async fn blocks(be: &dyn Backend, path: &Path) -> Result<()> {
    let status = be.stat(path).await?;
    let layout = be.block_layout(path).await?;

    println!();
    if status.inlined {
        println!("  {}   {} · inlined — no blocks", path.display(), human(status.len));
        println!();
        println!("  This file is below the inline threshold, so its bytes live in");
        println!("  the metadata store. No block IDs, no replicas, no bookkeeping.");
        println!();
        return Ok(());
    }

    println!(
        "  {}   {} · {} blocks · replication {}",
        path.display(),
        human(status.len),
        layout.len(),
        status.replication.unwrap_or(0)
    );
    println!();

    // Every node that appears anywhere in this file's placement, in order.
    let nodes: Vec<String> = {
        let mut seen: Vec<String> = Vec::new();
        for b in &layout {
            for r in &b.replicas {
                if !seen.contains(&r.node.0) {
                    seen.push(r.node.0.clone());
                }
            }
        }
        seen.sort();
        seen
    };

    print!("         ");
    for n in &nodes {
        print!("{n:^6}");
    }
    println!();

    for b in &layout {
        print!("  blk {:<3}", b.index + 1);
        for n in &nodes {
            let cell = b
                .replicas
                .iter()
                .find(|r| &r.node.0 == n)
                .map(|r| replica_symbol(r.state))
                .unwrap_or('·');
            print!("{cell:^6}");
        }
        println!();
    }

    println!();
    println!("  {LEGEND}");
    println!();

    // Rack grouping, plus a warning when a rack goes unused.
    let mut by_rack: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for b in &layout {
        for r in &b.replicas {
            let entry = by_rack.entry(r.rack.as_str()).or_default();
            if !entry.contains(&r.node.0.as_str()) {
                entry.push(&r.node.0);
            }
        }
    }
    let racks: Vec<String> = by_rack
        .iter()
        .map(|(rack, nodes)| {
            let mut n = nodes.clone();
            n.sort();
            format!("{} ∈ {}", n.join(" "), rack)
        })
        .collect();
    println!("  racks:   {}", racks.join("    "));

    warn_on_placement(&layout);
    println!();
    Ok(())
}

/// Flag blocks whose replicas do not span at least two racks.
fn warn_on_placement(layout: &[BlockPlacement]) {
    for b in layout {
        let racks: std::collections::BTreeSet<_> = b.replicas.iter().map(|r| &r.rack).collect();
        if b.replicas.len() > 1 && racks.len() < 2 {
            println!();
            println!("  ⚠ blk {} has every replica in one rack ({})", b.index + 1, {
                let mut it = racks.into_iter();
                it.next().map(|s| s.as_str()).unwrap_or("?")
            });
            println!("    losing that rack loses this block");
            println!("    fix: mammoth admin balancer start");
        }
    }
}
```

**`{n:^6}` centres a value in six columns.** That one format spec is what keeps
the matrix aligned. `{:<3}` left-aligns the block number.

## Step 3 · The cluster heatmap

Add to the same file:

```rust
/// `mammoth viz cluster` — the per-node capacity heatmap.
pub async fn cluster(be: &dyn Backend) -> Result<()> {
    let report = be.cluster_report().await?;
    print_cluster(&report);
    Ok(())
}

fn print_cluster(report: &ClusterReport) {
    let pct = |used: u64, cap: u64| if cap == 0 { 0.0 } else { used as f64 / cap as f64 };

    println!();
    println!(
        "  CLUSTER STORAGE  ·  {} / {} used ({:.0}%)",
        human(report.used),
        human(report.capacity),
        pct(report.used, report.capacity) * 100.0
    );
    println!();

    let mut by_rack: BTreeMap<&str, Vec<&mammoth_core::types::NodeReport>> = BTreeMap::new();
    for n in &report.nodes {
        by_rack.entry(n.rack.as_str()).or_default().push(n);
    }

    for (rack, nodes) in &by_rack {
        print!("  {rack:<14}");
        for (i, n) in nodes.iter().enumerate() {
            if i > 0 {
                print!("   ");
            }
            let f = pct(n.used, n.capacity);
            print!("{:>4} {} {:>3.0}%", n.id.0, bar(f, 16), f * 100.0);
        }
        println!();
    }

    let fractions: Vec<f64> = report.nodes.iter().map(|n| pct(n.used, n.capacity)).collect();
    let sigma = std_dev_pct(&fractions);
    println!();
    println!("  imbalance  σ = {sigma:.1}%   (healthy < 10%)");
    println!();
}
```

## Step 4 · Dispatch

Register the module in `commands/mod.rs`:

```rust
pub mod fs;
pub mod viz;
```

and add the arm in `main.rs`, after the `Cat` arm:

```rust
        cli::Command::Viz { what } => match what {
            cli::VizCommand::Blocks { path } => commands::viz::blocks(&be, &path).await,
            cli::VizCommand::Cluster => commands::viz::cluster(&be).await,
            _ => unimplemented!("this viz view — see docs/ROADMAP.md, milestone M2"),
        },
```

```bash
cargo build -p mammoth-cli
```

## Check it works

```bash
export MAMMOTH_HOME=/tmp/mammoth-demo && rm -rf "$MAMMOTH_HOME"
```

```bash
head -c 350000 /dev/urandom > /tmp/sales.csv
```

```bash
./target/debug/mammoth put /tmp/sales.csv /data/sales.csv --block-size 128KB --inline-threshold 4KB
```

Now the thing you came for:

```bash
./target/debug/mammoth viz blocks /data/sales.csv
```

```
  /data/sales.csv   341.8 KB · 3 blocks · replication 3

           w1    w2    w3    w4    w6  
  blk 1    ◐     ◐     ·     ·     ●   
  blk 2    ●     ·     ◐     ◐     ·   
  blk 3    ·     ●     ◐     ◐     ·   

  ● primary   ◐ replica   ✕ corrupt   · absent

  racks:   w1 w2 ∈ /dc1/rack-a    w3 w4 ∈ /dc1/rack-b    w6 ∈ /dc1/rack-c
```

**Read that matrix.** Block 1's primary is on `w6` (rack-c), with replicas on
`w1` and `w2` (both rack-a). Block 2's primary is on `w1` (rack-a), replicas on
`w3` and `w4` (both rack-b). Every block spans two racks, and the two secondary
replicas always share a rack with each other.

That is the placement rule from chapter 5, and you can now *see* it. If you had
made a mistake in `place`, it would be obvious in one glance rather than buried
in a test failure.

A bigger file makes the pattern clearer still:

```bash
head -c 900000 /dev/urandom > /tmp/events.parquet
./target/debug/mammoth put /tmp/events.parquet /warehouse/events.parquet --block-size 100KB --inline-threshold 4KB
./target/debug/mammoth viz blocks /warehouse/events.parquet
```

```
  /warehouse/events.parquet   878.9 KB · 9 blocks · replication 3

           w1    w2    w3    w4    w5    w6  
  blk 1    ·     ·     ●     ·     ◐     ◐   
  blk 2    ·     ·     ·     ●     ◐     ◐   
  blk 3    ◐     ◐     ·     ·     ●     ·   
  blk 4    ◐     ◐     ·     ·     ·     ●   
  blk 5    ●     ·     ◐     ◐     ·     ·   
  blk 6    ·     ●     ◐     ◐     ·     ·   
  blk 7    ·     ·     ●     ·     ◐     ◐   
  blk 8    ·     ·     ·     ●     ◐     ◐   
  blk 9    ◐     ◐     ·     ·     ●     ·   

  ● primary   ◐ replica   ✕ corrupt   · absent

  racks:   w1 w2 ∈ /dc1/rack-a    w3 w4 ∈ /dc1/rack-b    w5 w6 ∈ /dc1/rack-c
```

Nine blocks spread evenly across six workers, every one of them rack-safe.

An inlined file has nothing to show, and says so instead of printing an empty
grid:

```bash
echo hello > /tmp/hello.txt
./target/debug/mammoth put /tmp/hello.txt /data/hello.txt
./target/debug/mammoth viz blocks /data/hello.txt
```

```
  /data/hello.txt   6 B · inlined — no blocks

  This file is below the inline threshold, so its bytes live in
  the metadata store. No block IDs, no replicas, no bookkeeping.
```

And the cluster view:

```bash
./target/debug/mammoth viz cluster
```

```
  CLUSTER STORAGE  ·  3.6 MB / 960.0 GB used (0%)

  /dc1/rack-a     w1 ░░░░░░░░░░░░░░░░   0%     w2 ░░░░░░░░░░░░░░░░   0%
  /dc1/rack-b     w3 ░░░░░░░░░░░░░░░░   0%     w4 ░░░░░░░░░░░░░░░░   0%
  /dc1/rack-c     w5 ░░░░░░░░░░░░░░░░   0%     w6 ░░░░░░░░░░░░░░░░   0%

  imbalance  σ = 0.0%   (healthy < 10%)
```

Correct, and boring — a few megabytes against a pretend 160 GB per worker
rounds to nothing. To watch the bars move, temporarily shrink `FAKE_CAPACITY`
in `crates/mammoth-local/src/lib.rs`:

```rust
const FAKE_CAPACITY: u64 = 4 * 1024 * 1024;   // 4 MB, just for the demo
```

rebuild, and run `viz cluster` again. Put it back afterwards.

## Commit it

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```

```bash
git add -A && git commit -m "feat(viz): add viz blocks and viz cluster"
```

**You have reached milestone M2.** You have a working single-machine filesystem,
a CLI that scripts cleanly, and the visualization nobody else in this space has.
That is a demo.

## Exercises

1. **`--json` for `viz blocks`.** Right now it always prints the matrix. Make it
   implement `Render` like the chapter 7 commands, so
   `mammoth viz blocks /x --json | jq '.[].replicas'` works.
2. **Colour.** Use `owo-colors` to make `●` green, `◐` yellow, `✕` red. Gate it
   on `std::io::stdout().is_terminal()` — never emit ANSI escapes into a pipe.
3. **`viz topology`.** The rack tree. `cluster_report` already has everything.
4. **Break the placement on purpose.** Edit `WORKERS` in
   `crates/mammoth-local/src/lib.rs` so all six are in `/dc1/rack-a`, re-put a
   file, and run `viz blocks`. You should see the `⚠` warning fire. Put it back.

That last one is worth doing. With the default six workers the warning never
fires — which is the correct outcome, but it means the code path is untested
until you deliberately break something.

## If it went wrong

**The matrix columns do not line up** — you used `{}` instead of `{:^6}`, or
your terminal font renders `●` and `◐` at different widths. Try a different
font before rewriting the code.

**`error[E0599]: no method named cmp` on sorting nodes** — `seen.sort()` needs
the element type to be `Ord`. `String` is; `&String` in a mixed collection may
not be. Collect owned `String`s as the code does.

**Boxes or question marks instead of `█` and `●`** — your terminal is not in
UTF-8 mode. On Linux, `export LANG=en_US.UTF-8`. On Windows, use Windows
Terminal rather than the old console host.

**`viz blocks` on a directory panics or prints nothing** — `block_layout` calls
`read_meta`, which expects a file. Add an `is_dir` check and return
`Error::WrongKind` with a message pointing at `mammoth ls`.

**The `⚠` warning never appears** — correct. See exercise 4.

---

**Next:** [Chapter 9 — The web UI and the gateway](09-web-ui.md)
