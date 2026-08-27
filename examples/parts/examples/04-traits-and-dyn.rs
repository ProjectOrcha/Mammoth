//! 04 · Traits, `impl Trait for Type`, and `&dyn Trait`.
//!
//!     cargo run -p mammoth-parts --example 04-traits-and-dyn
//!
//! This is the single most important idea in Mammoth's architecture, shrunk to
//! one file. Read chapter 4 of the guide alongside it.
//!
//! The whole trick: write your code against a **trait** rather than a concrete
//! type, and you can swap the implementation underneath without touching a line
//! of the code above it. `print_report` below is called twice — once against a
//! fake in-memory store, once against something pretending to be a real
//! cluster — and it is compiled exactly once.

use std::collections::BTreeMap;

/// A trait is a promise: "any type that implements me provides these methods."
/// Other languages call this an interface.
///
/// This is `mammoth_core::Backend` with five of its seven methods removed.
trait Store {
    /// `&self` — every method here reads, none of them mutate.
    fn name(&self) -> &str;

    /// List the files under a directory.
    fn list(&self, dir: &str) -> Vec<String>;

    /// How many bytes one file holds.
    fn len(&self, path: &str) -> Option<u64>;

    /// A method with a **default body**. Implementors get it for free and may
    /// override it. Most traits with more than three methods have a few.
    fn describe(&self) -> String {
        format!("{} store", self.name())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Implementation 1 — everything in a map in memory.
// ─────────────────────────────────────────────────────────────────────────────

struct MemoryStore {
    files: BTreeMap<String, u64>,
}

impl Store for MemoryStore {
    fn name(&self) -> &str {
        "memory"
    }

    fn list(&self, dir: &str) -> Vec<String> {
        self.files.keys().filter(|p| p.starts_with(dir)).cloned().collect()
    }

    fn len(&self, path: &str) -> Option<u64> {
        self.files.get(path).copied()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Implementation 2 — pretends to talk to a cluster over a network. Different
// data, different code, *same trait*.
// ─────────────────────────────────────────────────────────────────────────────

struct ClusterStore {
    masters: Vec<String>,
}

impl Store for ClusterStore {
    fn name(&self) -> &str {
        "cluster"
    }

    fn list(&self, dir: &str) -> Vec<String> {
        // Imagine a gRPC call here.
        vec![format!("{dir}/events.parquet"), format!("{dir}/sales.csv")]
    }

    fn len(&self, path: &str) -> Option<u64> {
        Some(path.len() as u64 * 1_000_000)
    }

    /// Overriding the default body.
    fn describe(&self) -> String {
        format!("cluster store, masters: {}", self.masters.join(","))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// The payoff. This function is written once and works against both.
// ─────────────────────────────────────────────────────────────────────────────

/// `&dyn Store` is a **trait object**: a reference to *something* implementing
/// `Store`, with the exact type decided at run time.
///
/// Two consequences worth understanding:
///
///   1. This function physically cannot call anything outside the trait. That
///      is not discipline, it is a compile-time guarantee.
///   2. The `dyn` needs a reference (`&dyn`) or a box (`Box<dyn>`), because the
///      compiler does not know how big the concrete value is.
fn print_report(store: &dyn Store, dir: &str) {
    println!("\n  {}", store.describe());
    for path in store.list(dir) {
        let len = store.len(&path).unwrap_or(0);
        println!("    {path:<32} {len:>12} bytes");
    }
}

/// The other way to accept a trait: `impl Trait`, which is **static** dispatch.
/// The compiler stamps out a separate copy of this function for each concrete
/// type you call it with — faster, but the code cannot be stored in a `Vec`
/// alongside other implementations.
fn count_files(store: &impl Store, dir: &str) -> usize {
    store.list(dir).len()
}

fn main() {
    let memory = MemoryStore {
        files: BTreeMap::from([
            ("/data/hello.txt".to_string(), 19),
            ("/data/sales.csv".to_string(), 350_000),
            ("/warehouse/events.parquet".to_string(), 900_000),
        ]),
    };

    let cluster = ClusterStore { masters: vec!["m1:7000".into(), "m2:7000".into()] };

    println!("── one function, two backends ──");
    print_report(&memory, "/data");
    print_report(&cluster, "/data");

    println!("\n── a Vec of different implementations ──");
    // This is what `dyn` buys you that `impl Trait` does not: a heterogeneous
    // collection. Both values are boxed, so both are the same size here.
    let stores: Vec<Box<dyn Store>> = vec![
        Box::new(MemoryStore { files: BTreeMap::new() }),
        Box::new(ClusterStore { masters: vec!["m1:7000".into()] }),
    ];
    for s in &stores {
        println!("  {}", s.describe());
    }

    println!("\n── static dispatch, for comparison ──");
    println!("  count_files(&memory, \"/data\")  = {}", count_files(&memory, "/data"));
    println!("  count_files(&cluster, \"/data\") = {}", count_files(&cluster, "/data"));

    println!();
    println!("  Now the point. In Mammoth the trait is `Backend`, `MemoryStore` is");
    println!("  `LocalBackend` (chapters 5–6) and `ClusterStore` is `ClusterBackend`");
    println!("  (milestone M5). `print_report` is every CLI command, the viz code and");
    println!("  the HTTP gateway — written months before `ClusterBackend` exists, and");
    println!("  unchanged on the day it lands.");
}
