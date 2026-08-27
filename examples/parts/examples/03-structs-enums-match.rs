//! 03 · Structs, enums, `match`, and iterators.
//!
//!     cargo run -p mammoth-parts --example 03-structs-enums-match
//!
//! Enums in Rust are not numbered constants. Each variant can carry its own
//! data, and `match` forces you to handle every one — the compiler fails the
//! build if you forget a case. That is a large part of why Rust programs do not
//! crash in production the way equivalent Java programs do.

fn main() {
    let cluster = fake_cluster();

    structs_and_methods(&cluster);
    enums_and_match(&cluster);
    iterators(&cluster);
    derives(&cluster);
}

// ─────────────────────────────────────────────────────────────────────────────
// A struct groups data. `#[derive(...)]` writes the boring code for you.
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct Node {
    id: String,
    rack: String,
    used: u64,
    capacity: u64,
    state: NodeState,
}

/// An `impl` block hangs methods off the struct.
impl Node {
    /// `&self` — reads the node, does not change it.
    fn fraction_used(&self) -> f64 {
        if self.capacity == 0 {
            0.0
        } else {
            self.used as f64 / self.capacity as f64
        }
    }

    /// Not all methods take `self`. This one is an *associated function*, the
    /// nearest thing Rust has to a constructor. You call it `Node::new(...)`.
    fn new(id: &str, rack: &str, used: u64, capacity: u64, state: NodeState) -> Self {
        Self { id: id.to_string(), rack: rack.to_string(), used, capacity, state }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// An enum is "one of these". Variants may carry data.
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
enum NodeState {
    Healthy,
    /// A variant with data attached: *why* it is only warning.
    Warn(&'static str),
    /// And here, *how long* it has been gone.
    Dead {
        since_minutes: u32,
    },
}

fn structs_and_methods(cluster: &[Node]) {
    println!("\n── structs ──");
    for n in cluster {
        println!("  {:<4} {:<12} {:>5.1}% used", n.id, n.rack, n.fraction_used() * 100.0);
    }
}

fn enums_and_match(cluster: &[Node]) {
    println!("\n── enums and match ──");
    for n in cluster {
        // `match` is exhaustive. Add a variant to NodeState and this stops
        // compiling until you handle it — which is exactly what you want.
        let (symbol, note) = match n.state {
            NodeState::Healthy => ("●", String::new()),
            NodeState::Warn(why) => ("◐", format!("  ({why})")),
            NodeState::Dead { since_minutes } => ("✕", format!("  (dead {since_minutes}m)")),
        };
        println!("  {symbol} {:<4}{note}", n.id);
    }
    println!();
    println!("  Try deleting one arm of that match and rebuilding. The compiler");
    println!("  will name the variant you forgot. Runtime bugs turned into build errors.");
}

// ─────────────────────────────────────────────────────────────────────────────
// Iterators. Most Mammoth code that looks clever is just a chain of these.
// ─────────────────────────────────────────────────────────────────────────────

fn iterators(cluster: &[Node]) {
    println!("\n── iterators ──");

    // filter + count. `matches!(value, Pattern)` is the short way to ask
    // "is it this shape?" without writing a whole match.
    let dead = cluster.iter().filter(|n| matches!(n.state, NodeState::Dead { .. })).count();
    show(&format!("{} nodes, {dead} dead", cluster.len()), ".filter().count()");

    // map + sum
    let used: u64 = cluster.iter().map(|n| n.used).sum();
    let capacity: u64 = cluster.iter().map(|n| n.capacity).sum();
    show(&format!("{used} / {capacity} bytes used"), ".map().sum()");

    // max_by, which needs a comparator because f64 is not totally ordered
    let fullest = cluster
        .iter()
        .max_by(|a, b| a.fraction_used().total_cmp(&b.fraction_used()))
        .expect("cluster is never empty here");
    show(&format!("fullest node: {}", fullest.id), ".max_by()");

    // collect into a new Vec
    let names: Vec<&str> = cluster.iter().map(|n| n.id.as_str()).collect();
    show(&format!("{names:?}"), ".map().collect()");

    println!();
    println!("  Every one of those is lazy: nothing runs until .count(), .sum(),");
    println!("  .collect() or a `for` loop asks for the next item. Chaining ten of");
    println!("  them still makes one pass over the data.");
}

fn derives(cluster: &[Node]) {
    println!("\n── #[derive(...)] ──");

    let n = &cluster[0];

    // Debug comes from #[derive(Debug)] and is what `{:?}` prints.
    println!("  {{:?}}   {n:?}");
    // `{:#?}` is the same thing, pretty-printed over several lines.
    // Invaluable while debugging; `dbg!(&value)` is the same thing plus the
    // file and line it was printed from.
    println!("  {{:#?}}  same value, pretty-printed:");
    println!("{n:#?}");

    // Clone comes from #[derive(Clone)].
    let copy = n.clone();
    println!("  cloned {} independently", copy.id);
}

/// Print a result and the iterator chain that produced it, in two columns.
fn show(result: &str, chain: &str) {
    println!("  {result:<42}{chain}");
}

fn fake_cluster() -> Vec<Node> {
    let gb = 1024 * 1024 * 1024;
    vec![
        Node::new("w1", "/dc1/rack-a", 114 * gb, 160 * gb, NodeState::Healthy),
        Node::new("w2", "/dc1/rack-a", 93 * gb, 160 * gb, NodeState::Healthy),
        Node::new("w3", "/dc1/rack-b", 151 * gb, 160 * gb, NodeState::Warn("94% full")),
        Node::new("w4", "/dc1/rack-b", 0, 160 * gb, NodeState::Dead { since_minutes: 12 }),
    ]
}
