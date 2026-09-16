//! A bounded cache of bytes verified from immutable blocks. Path lookups still
//! use committed metadata; unique block IDs prevent reuse after overwrites.
use bytes::Bytes;
use hashlink::LinkedHashMap;
use mammoth_core::backend::CacheStats;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Mutex,
};

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub(super) struct Key {
    pub block: u64,
    pub start: u64,
    pub end: u64,
}
struct State {
    entries: LinkedHashMap<Key, Bytes>,
    resident: u64,
}
pub(super) struct ReadCache {
    capacity: u64,
    max_entries: usize,
    state: Mutex<State>,
    hits: AtomicU64,
    misses: AtomicU64,
    evictions: AtomicU64,
}
impl ReadCache {
    pub fn new(capacity: u64) -> Self {
        // The entry cap bounds bookkeeping for tiny ranges independently of
        // payload capacity. LRU entries use at least 128 budget bytes each.
        let entries = (capacity / 128).clamp(1, 65536) as usize;
        Self {
            capacity,
            max_entries: entries,
            state: Mutex::new(State { entries: LinkedHashMap::new(), resident: 0 }),
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            evictions: AtomicU64::new(0),
        }
    }
    pub fn get(&self, key: Key) -> Option<Bytes> {
        if self.capacity == 0 {
            return None;
        }
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let result = state.entries.to_back(&key).cloned();
        if result.is_some() {
            self.hits.fetch_add(1, Ordering::Relaxed);
        } else {
            self.misses.fetch_add(1, Ordering::Relaxed);
        }
        result
    }
    pub fn insert(&self, key: Key, value: Bytes) {
        let cost = value.len() as u64 + 128;
        if cost > self.capacity || value.is_empty() {
            return;
        }
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.entries.contains_key(&key) {
            return;
        }
        while state.resident + cost > self.capacity || state.entries.len() == self.max_entries {
            let Some((_, old)) = state.entries.pop_front() else {
                break;
            };
            state.resident -= old.len() as u64 + 128;
            self.evictions.fetch_add(1, Ordering::Relaxed);
        }
        state.resident += cost;
        state.entries.insert(key, value);
    }
    pub fn clear(&self) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.entries.clear();
        state.resident = 0;
    }
    pub fn stats(&self) -> CacheStats {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        CacheStats {
            capacity_bytes: self.capacity,
            resident_bytes: state.resident,
            entries: state.entries.len() as u64,
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
            evictions: self.evictions.load(Ordering::Relaxed),
        }
    }
}
