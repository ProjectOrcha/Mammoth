//! Run: cargo run -p mammoth-parts --example 17-atomic-publication
//! An in-memory teaching model: atomic publication and stable read snapshots.
//! It does not implement disk durability, checksums, quotas or a network service.
use std::{
    collections::{btree_map::Entry, BTreeMap},
    sync::{Arc, Barrier, Mutex},
};

#[derive(Default)]
struct Namespace(Mutex<BTreeMap<String, Arc<[u8]>>>);
impl Namespace {
    fn create(&self, path: &str, data: Arc<[u8]>) -> Result<(), &'static str> {
        let mut entries = self.0.lock().map_err(|_| "namespace lock poisoned")?;
        match entries.entry(path.into()) {
            Entry::Vacant(entry) => {
                entry.insert(data);
                Ok(())
            }
            Entry::Occupied(_) => Err("destination already exists"),
        }
    }
    fn replace(&self, path: &str, data: Arc<[u8]>) -> Result<(), &'static str> {
        self.0.lock().map_err(|_| "namespace lock poisoned")?.insert(path.into(), data);
        Ok(())
    }
    fn read(&self, path: &str) -> Result<Arc<[u8]>, &'static str> {
        self.0
            .lock()
            .map_err(|_| "namespace lock poisoned")?
            .get(path)
            .cloned()
            .ok_or("missing path")
    }
}

fn main() -> Result<(), &'static str> {
    let namespace = Namespace::default();
    namespace.create("/result", Arc::from(&b"original"[..]))?;
    let snapshot = namespace.read("/result")?;
    assert_eq!(
        namespace.create("/result", Arc::from(&b"accidental overwrite"[..])),
        Err("destination already exists")
    );
    namespace.replace("/result", Arc::from(&b"new generation"[..]))?;
    println!("Captured read: {}", String::from_utf8_lossy(&snapshot));
    println!("New read: {}", String::from_utf8_lossy(&namespace.read("/result")?));

    let shared = Arc::new(Namespace::default());
    let start = Arc::new(Barrier::new(2));
    let handles: Vec<_> = [b"client A".as_slice(), b"client B".as_slice()]
        .into_iter()
        .map(|bytes| {
            let shared = shared.clone();
            let start = start.clone();
            std::thread::spawn(move || {
                start.wait();
                shared.create("/race", Arc::from(bytes))
            })
        })
        .collect();
    let successes = handles
        .into_iter()
        .map(|task| task.join().expect("example thread panicked"))
        .filter(Result::is_ok)
        .count();
    assert_eq!(successes, 1);
    println!("Two simultaneous creates: {successes} succeeded, 1 preserved the existing file.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn creation_during_processing_is_not_overwritten() {
        let ns = Namespace::default();
        assert!(ns.read("/output").is_err()); // A job checks before computing.
        ns.create("/output", Arc::from(&b"another client"[..])).unwrap();
        assert!(ns.create("/output", Arc::from(&b"job result"[..])).is_err());
        assert_eq!(&*ns.read("/output").unwrap(), b"another client");
    }
    #[test]
    fn a_snapshot_retains_the_generation_it_opened() {
        let ns = Namespace::default();
        ns.create("/input", Arc::from(&b"first"[..])).unwrap();
        let snapshot = ns.read("/input").unwrap();
        ns.replace("/input", Arc::from(&b"second"[..])).unwrap();
        assert_eq!(&*snapshot, b"first");
        assert_eq!(&*ns.read("/input").unwrap(), b"second");
    }
}
