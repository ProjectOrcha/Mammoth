//! 05 · `async`, `.await`, `#[async_trait]` and streams of bytes.
//!
//!     cargo run -p mammoth-parts --example 05-async-and-streams
//!
//! Reading a disk or a network is slow. `async` lets one thread go and do
//! something else while it waits, instead of sitting there blocked. Every
//! method on `Backend` is async for exactly that reason: a real cluster read
//! spends nearly all of its time waiting for a worker to answer.

use std::pin::Pin;
use std::time::{Duration, Instant};

use bytes::Bytes;
use futures_util::{Stream, StreamExt};

/// `#[tokio::main]` starts the runtime that actually drives all of this. Without
/// it you get `` `main` function is not allowed to be `async` `` — that error is
/// always this line missing.
#[tokio::main]
async fn main() {
    three_rules().await;
    concurrency().await;
    async_traits().await;
    streams().await;
}

// ─────────────────────────────────────────────────────────────────────────────
// The three rules that cover everything in this codebase.
// ─────────────────────────────────────────────────────────────────────────────

/// Pretend to fetch a block from a worker. `sleep` here stands in for a network
/// round trip.
async fn fetch_block(node: &str, millis: u64) -> String {
    tokio::time::sleep(Duration::from_millis(millis)).await;
    format!("block from {node}")
}

async fn three_rules() {
    println!("\n── the three rules ──");

    // Rule 1: calling an async fn returns a *future* — a description of work.
    let future = fetch_block("w1", 50);
    println!("  1. calling fetch_block() ran nothing at all yet");

    // Rule 2: nothing happens until you .await it.
    let answer = future.await;
    println!("  2. .await ran it: {answer}");

    // Rule 3: you may only .await inside another async fn. That is why `main`
    // is async, and why every function in this file that awaits is too.
    println!("  3. that only compiled because this fn is `async`");
}

// ─────────────────────────────────────────────────────────────────────────────
// Why anyone bothers: doing three slow things at once.
// ─────────────────────────────────────────────────────────────────────────────

async fn concurrency() {
    println!("\n── sequential vs concurrent ──");

    let t0 = Instant::now();
    let _a = fetch_block("w1", 100).await;
    let _b = fetch_block("w2", 100).await;
    let _c = fetch_block("w3", 100).await;
    println!("  three awaits, one after another:  {:>4} ms", t0.elapsed().as_millis());

    // `join!` starts all three and waits for all three. Same thread, one third
    // of the wall clock. This is how a real `read` fetches replicas.
    let t0 = Instant::now();
    let (_a, _b, _c) =
        tokio::join!(fetch_block("w1", 100), fetch_block("w2", 100), fetch_block("w3", 100),);
    println!("  the same three inside join!:      {:>4} ms", t0.elapsed().as_millis());

    // `select!` takes whichever finishes first and drops the rest — the shape
    // of a hedged read, chapter 12 §1.
    let t0 = Instant::now();
    let winner = tokio::select! {
        r = fetch_block("slow-w1", 200) => r,
        r = fetch_block("fast-w2", 40)  => r,
    };
    println!("  select! took the first to answer: {:>4} ms  ({winner})", t0.elapsed().as_millis());
}

// ─────────────────────────────────────────────────────────────────────────────
// #[async_trait] — the incantation on every impl of Backend.
// ─────────────────────────────────────────────────────────────────────────────

/// Rust's built-in `async fn` in traits does not yet support the dynamic
/// dispatch (`&dyn Backend`) that Mammoth needs. The `async_trait` macro
/// rewrites each method to return a boxed future, which does.
///
/// You need the attribute in **two** places: on the trait, and on every `impl`.
/// Forgetting it on the impl gives a confusing error about lifetimes; this is
/// the answer to it.
#[async_trait::async_trait]
trait Fetcher {
    async fn get(&self, key: &str) -> String;
}

struct Worker(&'static str);

#[async_trait::async_trait]
impl Fetcher for Worker {
    async fn get(&self, key: &str) -> String {
        tokio::time::sleep(Duration::from_millis(10)).await;
        format!("{key} @ {}", self.0)
    }
}

async fn async_traits() {
    println!("\n── #[async_trait] ──");

    // And because it is a trait, `&dyn` works exactly as in example 04.
    let workers: Vec<Box<dyn Fetcher>> = vec![Box::new(Worker("w1")), Box::new(Worker("w2"))];
    for w in &workers {
        println!("  {}", w.get("blk_1001").await);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ByteStream — how Mammoth moves a 10 TB file through 8 MB of memory.
// ─────────────────────────────────────────────────────────────────────────────

/// This is `mammoth_core::backend::ByteStream`, spelled out.
///
/// ```text
/// Stream<Item = Bytes>  — a sequence of chunks, produced over time
/// + Send                — safe to move between threads
/// Box<...>              — on the heap, because the concrete type varies
/// Pin<...>              — it may not be moved in memory once polled
/// ```
///
/// You will type this alias, not build it from scratch. But it is worth being
/// able to read.
type ByteStream = Pin<Box<dyn Stream<Item = Bytes> + Send>>;

/// Chop a buffer into `chunk` sized pieces and hand them out one at a time.
fn chunked(data: Vec<u8>, chunk: usize) -> ByteStream {
    let bytes = Bytes::from(data);
    Box::pin(futures_util::stream::unfold(bytes, move |rest| async move {
        if rest.is_empty() {
            return None;
        }
        let take = chunk.min(rest.len());
        // `Bytes::slice` is a refcount bump, not a copy. That is why the
        // `Backend` trait uses `Bytes` rather than `Vec<u8>`.
        let head = rest.slice(..take);
        Some((head, rest.slice(take..)))
    }))
}

async fn streams() {
    println!("\n── streams of bytes ──");

    let data: Vec<u8> = (0..=255u8).cycle().take(10_000).collect();
    let mut stream = chunked(data, 4096);

    let mut chunks = 0;
    let mut total = 0;
    // This loop is exactly what `mammoth cat` does. Note that the whole file is
    // never in memory at once — only the chunk in hand.
    while let Some(chunk) = stream.next().await {
        chunks += 1;
        total += chunk.len();
        println!("  chunk {chunks}: {:>5} bytes", chunk.len());
    }

    println!("  {chunks} chunks, {total} bytes, never more than 4 KB held at a time");
    println!();
    println!("  Scale that up: the same loop streams a 10 TB file through a laptop.");
    println!("  Collecting into a Vec first would not. That is the whole reason");
    println!("  `Backend::read` returns a stream instead of bytes.");
}
