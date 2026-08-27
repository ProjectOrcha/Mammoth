//! 13 · The block × node matrix, in colour — the picture the project exists for.
//!
//!     cargo run -q -p mammoth-parts --example 13-block-matrix
//!     cargo run -q -p mammoth-parts --example 13-block-matrix -- --unsafe-placement
//!     cargo run -q -p mammoth-parts --example 13-block-matrix | cat
//!
//! This runs on **fake data**, deliberately. It uses the real
//! `mammoth_core::BlockPlacement` type, so on the day `LocalBackend::block_layout`
//! starts returning real placements you delete `fake_layout()` and change one
//! line. That trick — build your layer against hand-made data of the right type
//! — is what lets three people work in parallel from week one. See
//! `docs/guide/TEAM-PLAN.md`.
//!
//! Try `--unsafe-placement`. It puts all three replicas of one block in a single
//! rack, which is the failure this whole visualization exists to make obvious.

use std::collections::{BTreeMap, BTreeSet};

use clap::Parser;
use mammoth_core::types::{BlockId, BlockPlacement, NodeId, Replica, ReplicaState};
use owo_colors::{AnsiColors, OwoColorize, Stream};

#[derive(Parser)]
#[command(about = "Draw a file's blocks and their replicas")]
struct Args {
    /// Put every replica of block 2 in one rack, to see the warning fire.
    #[arg(long)]
    r#unsafe_placement: bool,
}

fn main() {
    let args = Args::parse();
    let layout = fake_layout(args.unsafe_placement);

    draw("/warehouse/events.parquet", 900_000, 3, &layout);
}

// ─────────────────────────────────────────────────────────────────────────────
// Colour, from meaning. Same three states the CLI, the TUI and the web UI use.
// ─────────────────────────────────────────────────────────────────────────────

/// Symbol **and** colour for one replica. The symbol is not decoration: it is
/// what survives `| cat`, a monochrome terminal, and a red/green deficiency.
fn cell(state: Option<ReplicaState>) -> String {
    let (symbol, colour) = match state {
        Some(ReplicaState::Primary) => ('●', AnsiColors::Green),
        Some(ReplicaState::Replica) => ('◐', AnsiColors::Cyan),
        Some(ReplicaState::Corrupt) => ('✕', AnsiColors::Red),
        None => ('·', AnsiColors::BrightBlack),
    };
    // `{:^6}` centres in six columns — the one format spec that keeps the whole
    // matrix aligned. Do the centring on the *plain* character and colour the
    // result, or the escape codes get counted as width and the grid shears.
    let padded = format!("{symbol:^6}");
    format!("{}", padded.if_supports_color(Stream::Stdout, |t| t.color(colour)))
}

const LEGEND: &str = "● primary   ◐ replica   ✕ corrupt   · absent";

// ─────────────────────────────────────────────────────────────────────────────
// The drawing itself.
// ─────────────────────────────────────────────────────────────────────────────

fn draw(path: &str, len: u64, replication: u8, layout: &[BlockPlacement]) {
    println!();
    println!(
        "  {}   {} · {} blocks · replication {replication}",
        path.if_supports_color(Stream::Stdout, |t| t.bold()),
        human(len),
        layout.len(),
    );
    println!();

    // Every node that appears anywhere in this file's placement, sorted so the
    // columns are stable between runs.
    let nodes: Vec<String> = {
        let mut seen: BTreeSet<String> = BTreeSet::new();
        for b in layout {
            for r in &b.replicas {
                seen.insert(r.node.0.clone());
            }
        }
        seen.into_iter().collect()
    };

    // Header row.
    print!("         ");
    for n in &nodes {
        let head = format!("{n:^6}");
        print!("{}", head.if_supports_color(Stream::Stdout, |t| t.dimmed()));
    }
    println!();

    // One row per block.
    for b in layout {
        let label = format!("  blk {:<3}", b.index + 1);
        print!("{}", label.if_supports_color(Stream::Stdout, |t| t.dimmed()));
        for n in &nodes {
            let state = b.replicas.iter().find(|r| &r.node.0 == n).map(|r| r.state);
            print!("{}", cell(state));
        }
        println!();
    }

    println!();
    println!("  {}", LEGEND.if_supports_color(Stream::Stdout, |t| t.dimmed()));
    println!();

    // Which nodes are in which rack — the context that makes the matrix mean
    // something. Two replicas on two nodes in the same cabinet is one replica.
    let mut by_rack: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for b in layout {
        for r in &b.replicas {
            by_rack.entry(r.rack.as_str()).or_default().insert(&r.node.0);
        }
    }
    let racks: Vec<String> = by_rack
        .iter()
        .map(|(rack, nodes)| {
            format!("{} ∈ {rack}", nodes.iter().copied().collect::<Vec<_>>().join(" "))
        })
        .collect();
    println!("  racks:   {}", racks.join("    "));

    warn_on_placement(layout);
    println!();
}

/// The check that earns the whole feature: flag any block whose replicas do not
/// span at least two racks.
fn warn_on_placement(layout: &[BlockPlacement]) {
    for b in layout {
        let racks: BTreeSet<&String> = b.replicas.iter().map(|r| &r.rack).collect();
        if b.replicas.len() > 1 && racks.len() < 2 {
            let rack = racks.iter().next().map(|s| s.as_str()).unwrap_or("?");
            println!();
            let head = format!("  ⚠ blk {} has every replica in one rack ({rack})", b.index + 1);
            // Chaining two styles inside the closure (`|t| t.yellow().bold()`)
            // does not compile — the first call makes a temporary the second
            // borrows. Build a `Style` and apply it in one go instead.
            let warn = owo_colors::Style::new().yellow().bold();
            println!("{}", head.if_supports_color(Stream::Stdout, |t| t.style(warn)));
            println!("    losing that rack loses this block");
            println!("    fix: mammoth admin balancer start");
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Fake data of the real type. Delete this when `block_layout` works.
// ─────────────────────────────────────────────────────────────────────────────

fn replica(node: &str, rack: &str, state: ReplicaState) -> Replica {
    Replica { node: NodeId(node.to_string()), rack: rack.to_string(), state }
}

fn fake_layout(unsafe_placement: bool) -> Vec<BlockPlacement> {
    let a = "/dc1/rack-a";
    let b = "/dc1/rack-b";
    let c = "/dc1/rack-c";
    use ReplicaState::{Corrupt, Primary, Replica as Sec};

    let mut blocks = vec![
        BlockPlacement {
            id: BlockId(1001),
            index: 0,
            len: 128 * 1024,
            replicas: vec![replica("w3", b, Primary), replica("w5", c, Sec), replica("w6", c, Sec)],
        },
        BlockPlacement {
            id: BlockId(1002),
            index: 1,
            len: 128 * 1024,
            replicas: vec![replica("w4", b, Primary), replica("w1", a, Sec), replica("w2", a, Sec)],
        },
        BlockPlacement {
            id: BlockId(1003),
            index: 2,
            len: 128 * 1024,
            // One bad copy, so the ✕ path is exercised. The master would be
            // re-replicating this one from a good copy right now.
            replicas: vec![
                replica("w5", c, Primary),
                replica("w1", a, Sec),
                replica("w2", a, Corrupt),
            ],
        },
        BlockPlacement {
            id: BlockId(1004),
            index: 3,
            len: 22 * 1024, // the last block of a file is usually partial
            replicas: vec![replica("w1", a, Primary), replica("w3", b, Sec), replica("w4", b, Sec)],
        },
    ];

    if unsafe_placement {
        blocks[1].replicas =
            vec![replica("w3", b, Primary), replica("w4", b, Sec), replica("w7", b, Sec)];
    }
    blocks
}

fn human(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let (mut v, mut u) = (bytes as f64, 0);
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    format!("{v:.1} {}", UNITS[u])
}
