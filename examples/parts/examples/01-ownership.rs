//! 01 · Ownership, borrowing and moves — the thing that trips everyone up first.
//!
//!     cargo run -p mammoth-parts --example 01-ownership
//!
//! Read this top to bottom. Then break it: uncomment the lines marked BREAK ME,
//! run it again, and read the compiler error. The errors are the lesson.

fn main() {
    owning();
    borrowing();
    mutable_borrowing();
    moves();
    cloning();
    the_rule();
}

/// Every value has exactly one owner. When the owner goes out of scope, the
/// value is freed. No garbage collector, no `free()`, no leak.
fn owning() {
    println!("\n── owning ──");

    let node = String::from("w1"); // `node` owns these bytes
    println!("  {node} is owned by the variable `node`");

    {
        let temporary = String::from("w2");
        println!("  {temporary} exists only inside this block");
    } // <- `temporary` is dropped right here, memory freed. Nothing to write.

    println!("  {node} is still alive; `temporary` is gone");
}

/// `&thing` lends a value out, read-only. The owner keeps ownership.
/// Any number of read-only borrows may exist at the same time.
fn borrowing() {
    println!("\n── borrowing (&) ──");

    let racks = vec![String::from("rack-a"), String::from("rack-b")];

    let first: &String = &racks[0]; // a loan
    let count = racks.len(); // reading through the owner is also fine

    println!("  {count} racks, the first is {first}");
    println!("  `racks` still owns its data — nothing was copied");

    // A function that only reads takes `&`:
    println!("  longest name: {}", longest(&racks));
}

/// Takes a read-only loan of the vector. Cannot modify it, does not consume it.
fn longest(racks: &[String]) -> &str {
    let mut best = "";
    for r in racks {
        if r.len() > best.len() {
            best = r;
        }
    }
    best
}

/// `&mut thing` is an *exclusive* loan. While it exists, nobody else — not even
/// the owner — may read or write. One writer, or many readers. Never both.
fn mutable_borrowing() {
    println!("\n── borrowing (&mut) ──");

    let mut blocks: Vec<u32> = vec![1, 2, 3];

    append_block(&mut blocks, 4); // hand out the exclusive loan
    println!("  after append: {blocks:?}"); // loan is over, owner may read again

    // BREAK ME — two live loans, one of them exclusive:
    //
    //   let reader = &blocks;
    //   append_block(&mut blocks, 5);
    //   println!("{reader:?}");
    //
    // error[E0502]: cannot borrow `blocks` as mutable because it is also
    //               borrowed as immutable
    //
    // This is the rule that makes data races impossible to compile.
}

fn append_block(blocks: &mut Vec<u32>, id: u32) {
    blocks.push(id);
}

/// Passing a value *without* `&` gives ownership away. The caller can no longer
/// use it. Rust calls this a **move**.
fn moves() {
    println!("\n── moves ──");

    let path = String::from("/data/sales.csv");
    let consumed = consume(path); // `path` is moved into `consume`

    println!("  consume returned: {consumed}");

    // BREAK ME:
    //
    //   println!("{path}");
    //
    // error[E0382]: borrow of moved value: `path`
    //
    // The fix is almost always one of three things:
    //   1. pass `&path` instead — the function only needed to read it
    //   2. `path.clone()` — you genuinely need two copies
    //   3. use the value the function gave back
}

fn consume(p: String) -> String {
    format!("{p} (consumed)")
}

/// `.clone()` makes a second, independent copy. It costs an allocation, so it
/// is not free — but while you are learning, cloning to get past a borrow error
/// is completely fine. Make it fast later, once a profiler says it matters.
fn cloning() {
    println!("\n── cloning ──");

    let original = String::from("/data/big.log");
    let copy = original.clone();

    println!("  original: {original}");
    println!("  copy:     {copy}   (two separate allocations)");
}

/// Where you will actually meet all of this: method receivers.
fn the_rule() {
    println!("\n── &self vs &mut self ──");

    struct Store {
        files: Vec<String>,
    }

    impl Store {
        /// `&self` — "this method reads the struct". Callers may call it from
        /// many places at once.
        fn count(&self) -> usize {
            self.files.len()
        }

        /// `&mut self` — "this method changes the struct". Exclusive.
        fn add(&mut self, path: &str) {
            self.files.push(path.to_string());
        }
    }

    let mut store = Store { files: Vec::new() };
    store.add("/data/a.csv");
    store.add("/data/b.csv");

    println!("  store holds {} files", store.count());
    println!();
    println!("  Rule of thumb when reading Mammoth's code:");
    println!("    &self      → reads");
    println!("    &mut self  → writes");
    println!("    self       → consumes; the value is gone afterwards");
}
