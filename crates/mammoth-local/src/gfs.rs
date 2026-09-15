//! Deterministic GFS teaching model. Run `cargo run -p mammoth-local --example gfs-demo`.
//!
//! All storage is in memory. An operation is one atomic simulation event: there
//! are no sockets, disk durability, network partitions, or mid-event crashes.
//! The external controller can fence a stopped master, which real deployments
//! must establish through consensus/fencing rather than a DNS change alone.

use std::collections::{BTreeMap, BTreeSet};

/// Simulation settings, deliberately separate from production `Config`.
#[derive(Debug, Clone)]
pub struct Settings {
    pub chunk_size: usize,
    pub replication: usize,
    pub heartbeat_secs: u64,
    pub missed_heartbeats: u32,
    pub lease_secs: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            chunk_size: 64 * 1024 * 1024,
            replication: 3,
            heartbeat_secs: 30,
            missed_heartbeats: 3,
            lease_secs: 60,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("invalid simulation settings or path")]
    InvalidInput,
    #[error("file already exists")]
    AlreadyExists,
    #[error("file, chunk, worker, or master does not exist")]
    NotFound,
    #[error("the active master is unavailable; advance heartbeats for takeover")]
    MasterUnavailable,
    #[error("not enough healthy workers for the requested replication")]
    NotEnoughWorkers,
    #[error("no readable replica remains")]
    DataUnavailable,
    #[error("the primary is unavailable; its lease must expire before replacement")]
    PrimaryUnavailable,
    #[error("the lease is expired or fenced; obtain a new lease and stage again")]
    StaleLease,
    #[error("data is not staged on every current replica; retry the mutation")]
    NotStaged,
    #[error("mutation exceeds the existing chunk length")]
    OutOfBounds,
}

pub type Result<T> = std::result::Result<T, Error>;

/// Authority to order writes. Its private fields cannot be manufactured by clients.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lease {
    chunk: u64,
    primary: usize,
    version: u64,
    master_epoch: u64,
    expires_at: u64,
}

impl Lease {
    pub fn primary(self) -> usize {
        self.primary
    }

    pub fn expires_at(self) -> u64 {
        self.expires_at
    }
}

/// A cached master response. Reading this contacts only the simulated workers.
#[derive(Debug, Clone)]
pub struct Location {
    pub chunk: u64,
    pub len: usize,
    pub version: u64,
    pub workers: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    WorkerDead(usize),
    Repaired { chunk: u64, from: usize, to: usize },
    UnderReplicated { chunk: u64, available: usize, wanted: usize },
    ChunkUnavailable(u64),
    MasterPromoted { master: usize, epoch: u64 },
}

#[derive(Debug, Clone, Default)]
struct Metadata {
    files: BTreeMap<String, Vec<u64>>,
    chunks: BTreeMap<u64, Chunk>,
    next_chunk: u64,
}

#[derive(Debug, Clone)]
struct Chunk {
    len: usize,
    workers: Vec<usize>,
    version: u64,
    sequence: u64,
    lease: Option<Lease>,
}

#[derive(Debug, Clone)]
struct Copy {
    bytes: Vec<u8>,
    version: u64,
    sequence: u64,
}

#[derive(Debug, Clone)]
struct Staged {
    lease: Lease,
    bytes: Vec<u8>,
}

#[derive(Debug)]
struct Worker {
    rack: String,
    online: bool,
    dead: bool,
    last_heartbeat: u64,
    chunks: BTreeMap<u64, Copy>,
    staged: BTreeMap<u64, Staged>,
}

#[derive(Debug, Default)]
struct Master {
    online: bool,
    last_heartbeat: u64,
    metadata: Metadata,
}

/// Separate byte stores and master snapshots, driven by one logical clock.
#[derive(Debug)]
pub struct Simulation {
    settings: Settings,
    now: u64,
    epoch: u64,
    active: usize,
    masters: [Master; 2],
    workers: Vec<Worker>,
    next_mutation: u64,
}

impl Simulation {
    pub fn new(settings: Settings, racks: Vec<String>) -> Result<Self> {
        if settings.chunk_size == 0
            || settings.replication == 0
            || settings.heartbeat_secs == 0
            || settings.missed_heartbeats == 0
            || settings.lease_secs == 0
            || settings.heartbeat_secs.checked_mul(settings.missed_heartbeats.into()).is_none()
            || racks.iter().any(String::is_empty)
        {
            return Err(Error::InvalidInput);
        }
        if racks.len() < settings.replication {
            return Err(Error::NotEnoughWorkers);
        }
        Ok(Self {
            settings,
            now: 0,
            epoch: 1,
            active: 0,
            masters: std::array::from_fn(|_| Master { online: true, ..Master::default() }),
            workers: racks
                .into_iter()
                .map(|rack| Worker {
                    rack,
                    online: true,
                    dead: false,
                    last_heartbeat: 0,
                    chunks: BTreeMap::new(),
                    staged: BTreeMap::new(),
                })
                .collect(),
            next_mutation: 0,
        })
    }

    pub fn now(&self) -> u64 {
        self.now
    }

    /// The simulated service endpoint. No real DNS is changed.
    pub fn active_master(&self) -> usize {
        self.active
    }

    fn metadata(&self) -> Result<&Metadata> {
        let master = &self.masters[self.active];
        if !master.online {
            return Err(Error::MasterUnavailable);
        }
        Ok(&master.metadata)
    }

    // Atomic in-memory snapshot replication, not a durable operation log.
    fn publish(&mut self, metadata: Metadata) {
        for master in &mut self.masters {
            if master.online {
                master.metadata = metadata.clone();
            }
        }
    }

    fn healthy(&self, worker: usize) -> bool {
        self.workers[worker].online && !self.workers[worker].dead
    }

    fn current_copy(&self, worker: usize, id: u64, chunk: &Chunk) -> bool {
        self.healthy(worker)
            && self.workers[worker].chunks.get(&id).is_some_and(|copy| {
                copy.version == chunk.version && copy.sequence == chunk.sequence
            })
    }

    // Prefer unused racks, then fewer stored bytes, then stable worker IDs.
    fn destination(&self, selected: &[usize]) -> Option<usize> {
        let racks: BTreeSet<_> = selected.iter().map(|&i| &self.workers[i].rack).collect();
        (0..self.workers.len()).filter(|&i| self.healthy(i) && !selected.contains(&i)).min_by_key(
            |&i| {
                (
                    racks.contains(&self.workers[i].rack),
                    self.workers[i].chunks.values().map(|c| c.bytes.len()).sum::<usize>(),
                    i,
                )
            },
        )
    }

    /// Split a new file into chunks and put each chunk on distinct workers.
    /// File bytes never live in either master snapshot (including small files).
    pub fn create_file(&mut self, path: &str, bytes: &[u8]) -> Result<()> {
        if !path.starts_with('/')
            || path[1..].split('/').any(|part| part.is_empty() || part == "." || part == "..")
        {
            return Err(Error::InvalidInput);
        }
        let mut metadata = self.metadata()?.clone();
        if metadata.files.contains_key(path) {
            return Err(Error::AlreadyExists);
        }
        if !bytes.is_empty()
            && (0..self.workers.len()).filter(|&i| self.healthy(i)).count()
                < self.settings.replication
        {
            return Err(Error::NotEnoughWorkers);
        }
        let count = bytes.len().div_ceil(self.settings.chunk_size) as u64;
        metadata.next_chunk.checked_add(count).ok_or(Error::InvalidInput)?;
        let mut ids = Vec::new();
        for data in bytes.chunks(self.settings.chunk_size) {
            let id = metadata.next_chunk;
            metadata.next_chunk += 1;
            let mut selected = Vec::new();
            for _ in 0..self.settings.replication {
                let worker = self.destination(&selected).ok_or(Error::NotEnoughWorkers)?;
                self.workers[worker]
                    .chunks
                    .insert(id, Copy { bytes: data.to_vec(), version: 0, sequence: 0 });
                selected.push(worker);
            }
            metadata.chunks.insert(
                id,
                Chunk { len: data.len(), workers: selected, version: 0, sequence: 0, lease: None },
            );
            ids.push(id);
        }
        metadata.files.insert(path.to_owned(), ids);
        self.publish(metadata);
        Ok(())
    }

    pub fn locate(&self, path: &str) -> Result<Vec<Location>> {
        let metadata = self.metadata()?;
        let ids = metadata.files.get(path).ok_or(Error::NotFound)?;
        Ok(ids
            .iter()
            .map(|&id| {
                let chunk = &metadata.chunks[&id];
                Location {
                    chunk: id,
                    len: chunk.len,
                    version: chunk.version,
                    workers: chunk.workers.clone(),
                }
            })
            .collect())
    }

    /// Read directly from cached locations, retrying another worker on failure.
    pub fn read_location(&self, location: &Location) -> Result<Vec<u8>> {
        location
            .workers
            .iter()
            .filter_map(|&id| self.workers.get(id))
            .filter(|worker| worker.online && !worker.dead)
            .filter_map(|worker| worker.chunks.get(&location.chunk))
            .find(|copy| copy.version == location.version && copy.bytes.len() == location.len)
            .map(|copy| copy.bytes.clone())
            .ok_or(Error::DataUnavailable)
    }

    pub fn read_file(&self, path: &str) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        for location in self.locate(path)? {
            bytes.extend(self.read_location(&location)?);
        }
        Ok(bytes)
    }

    /// Inspect a single current replica for teaching and assertions.
    pub fn replica_bytes(&self, chunk: u64, worker: usize) -> Result<&[u8]> {
        let record = self.metadata()?.chunks.get(&chunk).ok_or(Error::NotFound)?;
        if worker >= self.workers.len() || !self.current_copy(worker, chunk, record) {
            return Err(Error::DataUnavailable);
        }
        Ok(&self.workers[worker].chunks[&chunk].bytes)
    }

    pub fn lease(&mut self, id: u64) -> Result<Lease> {
        let mut metadata = self.metadata()?.clone();
        let chunk = metadata.chunks.get_mut(&id).ok_or(Error::NotFound)?;
        if let Some(lease) = chunk.lease {
            if lease.master_epoch == self.epoch && self.now < lease.expires_at {
                return if chunk.workers.contains(&lease.primary)
                    && self.current_copy(lease.primary, id, chunk)
                {
                    Ok(lease)
                } else {
                    Err(Error::PrimaryUnavailable)
                };
            }
        }
        let current: Vec<_> = chunk
            .workers
            .iter()
            .copied()
            .filter(|&worker| self.current_copy(worker, id, chunk))
            .collect();
        if current.len() < self.settings.replication {
            return Err(Error::NotEnoughWorkers);
        }
        let version = chunk.version.checked_add(1).ok_or(Error::InvalidInput)?;
        let lease = Lease {
            chunk: id,
            primary: current[0],
            version,
            master_epoch: self.epoch,
            expires_at: self
                .now
                .checked_add(self.settings.lease_secs)
                .ok_or(Error::InvalidInput)?,
        };
        chunk.version = version;
        chunk.lease = Some(lease);
        for worker in current {
            self.workers[worker].chunks.get_mut(&id).ok_or(Error::DataUnavailable)?.version =
                version;
        }
        // A new lease cannot inherit old buffered mutations.
        for worker in &mut self.workers {
            worker.staged.retain(|_, staged| staged.lease.chunk != id);
        }
        self.publish(metadata);
        Ok(lease)
    }

    fn validate_lease(&self, lease: Lease) -> Result<&Chunk> {
        let chunk = self.metadata()?.chunks.get(&lease.chunk).ok_or(Error::NotFound)?;
        if chunk.lease != Some(lease)
            || lease.master_epoch != self.epoch
            || self.now >= lease.expires_at
        {
            return Err(Error::StaleLease);
        }
        if !chunk.workers.contains(&lease.primary) || !self.healthy(lease.primary) {
            return Err(Error::PrimaryUnavailable);
        }
        if chunk.workers.len() != self.settings.replication
            || chunk.workers.iter().any(|&worker| !self.current_copy(worker, lease.chunk, chunk))
        {
            return Err(Error::NotEnoughWorkers);
        }
        Ok(chunk)
    }

    /// Data flow: buffer a client's bytes on every replica without changing data.
    /// Calls from several clients may be staged before any is ordered.
    pub fn stage(&mut self, lease: Lease, bytes: &[u8]) -> Result<u64> {
        let chunk = self.validate_lease(lease)?;
        if bytes.len() > chunk.len {
            return Err(Error::OutOfBounds);
        }
        let participants = chunk.workers.clone();
        let mutation = self.next_mutation.checked_add(1).ok_or(Error::InvalidInput)?;
        self.next_mutation = mutation;
        for worker in participants {
            self.workers[worker].staged.insert(mutation, Staged { lease, bytes: bytes.to_vec() });
        }
        Ok(mutation)
    }

    /// Control flow: the primary orders a staged mutation and every replica
    /// applies that order. Returns the sequence number, not the staging order.
    /// This fixed-length overwrite is not GFS record append or a file transaction.
    pub fn commit(&mut self, lease: Lease, mutation: u64, offset: usize) -> Result<u64> {
        let chunk = self.validate_lease(lease)?.clone();
        let bytes = self.workers[lease.primary]
            .staged
            .get(&mutation)
            .filter(|staged| staged.lease == lease)
            .ok_or(Error::NotStaged)?
            .bytes
            .clone();
        let end = offset.checked_add(bytes.len()).ok_or(Error::OutOfBounds)?;
        if end > chunk.len {
            return Err(Error::OutOfBounds);
        }
        for &worker in &chunk.workers {
            if !self.workers[worker]
                .staged
                .get(&mutation)
                .is_some_and(|staged| staged.lease == lease && staged.bytes == bytes)
            {
                return Err(Error::NotStaged);
            }
        }
        let sequence = chunk.sequence.checked_add(1).ok_or(Error::InvalidInput)?;
        let mut metadata = self.metadata()?.clone();
        metadata.chunks.get_mut(&lease.chunk).ok_or(Error::NotFound)?.sequence = sequence;
        // All participants were checked before applying this atomic model event.
        for worker in chunk.workers {
            let copy =
                self.workers[worker].chunks.get_mut(&lease.chunk).ok_or(Error::DataUnavailable)?;
            copy.bytes[offset..end].copy_from_slice(&bytes);
            copy.sequence = sequence;
            self.workers[worker].staged.remove(&mutation);
        }
        self.publish(metadata);
        Ok(sequence)
    }

    pub fn stop_worker(&mut self, worker: usize) -> Result<()> {
        let worker = self.workers.get_mut(worker).ok_or(Error::NotFound)?;
        worker.online = false;
        worker.staged.clear();
        Ok(())
    }

    /// Inject permanent disk loss, including any staged data.
    pub fn lose_worker_data(&mut self, worker: usize) -> Result<()> {
        self.stop_worker(worker)?;
        self.workers[worker].chunks.clear();
        Ok(())
    }

    /// Returning workers stay quarantined until a heartbeat and reconciliation.
    pub fn start_worker(&mut self, worker: usize) -> Result<()> {
        let worker = self.workers.get_mut(worker).ok_or(Error::NotFound)?;
        worker.online = true;
        worker.dead = true;
        Ok(())
    }

    pub fn stop_master(&mut self, master: usize) -> Result<()> {
        self.masters.get_mut(master).ok_or(Error::NotFound)?.online = false;
        Ok(())
    }

    /// Rejoin a stopped master as a follower after copying the active snapshot.
    pub fn start_master(&mut self, master: usize) -> Result<()> {
        if master >= self.masters.len() {
            return Err(Error::NotFound);
        }
        let metadata = self.metadata()?.clone();
        self.masters[master] = Master { online: true, last_heartbeat: self.now, metadata };
        Ok(())
    }

    /// Advance one heartbeat interval; healthy roles send beats, the external
    /// monitor promotes a standby if needed, then the master detects and repairs.
    pub fn tick(&mut self) -> Result<Vec<Event>> {
        self.now = self.now.checked_add(self.settings.heartbeat_secs).ok_or(Error::InvalidInput)?;
        let deadline = self.settings.heartbeat_secs * u64::from(self.settings.missed_heartbeats);
        let mut events = Vec::new();
        for master in &mut self.masters {
            if master.online {
                master.last_heartbeat = self.now;
            }
        }
        if !self.masters[self.active].online
            && self.now - self.masters[self.active].last_heartbeat >= deadline
        {
            if let Some(next) = self.masters.iter().position(|master| master.online) {
                self.epoch = self.epoch.checked_add(1).ok_or(Error::InvalidInput)?;
                self.active = next;
                events.push(Event::MasterPromoted { master: next, epoch: self.epoch });
                // The controller fences the old master; staging must be retried.
                for worker in &mut self.workers {
                    worker.staged.clear();
                }
            }
        }
        if !self.masters[self.active].online {
            return Ok(events);
        }
        for (id, worker) in self.workers.iter_mut().enumerate() {
            if worker.online {
                worker.last_heartbeat = self.now;
                worker.dead = false;
            } else if !worker.dead && self.now - worker.last_heartbeat >= deadline {
                worker.dead = true;
                events.push(Event::WorkerDead(id));
            }
        }
        self.repair(&mut events)?;
        Ok(events)
    }

    fn repair(&mut self, events: &mut Vec<Event>) -> Result<()> {
        let mut metadata = self.metadata()?.clone();
        for (&id, chunk) in &mut metadata.chunks {
            // Keep silent workers in the map until the configured deadline.
            // A reboot before it expires can therefore avoid unnecessary copies.
            let needs_repair = chunk.workers.len() < self.settings.replication
                || chunk.workers.iter().any(|&i| {
                    self.workers[i].dead
                        || (self.workers[i].online && !self.current_copy(i, id, chunk))
                });
            if !needs_repair {
                continue;
            }
            let mut current: Vec<_> = chunk
                .workers
                .iter()
                .copied()
                .filter(|&i| self.current_copy(i, id, chunk))
                .collect();
            let Some(&source) = current.first() else {
                events.push(Event::ChunkUnavailable(id));
                continue;
            };
            while current.len() < self.settings.replication {
                let Some(target) = self.destination(&current) else { break };
                let copy = self.workers[source].chunks[&id].clone();
                self.workers[target].chunks.insert(id, copy);
                self.workers[target].staged.retain(|_, staged| staged.lease.chunk != id);
                current.push(target);
                events.push(Event::Repaired { chunk: id, from: source, to: target });
            }
            if current.len() < self.settings.replication {
                events.push(Event::UnderReplicated {
                    chunk: id,
                    available: current.len(),
                    wanted: self.settings.replication,
                });
            }
            // Discard buffered writes after membership changes, but preserve the
            // lease deadline: replacing a lost copy cannot revoke a still-live
            // primary lease. A new primary must wait for expiry or master fencing.
            chunk.workers = current;
            for worker in &mut self.workers {
                worker.staged.retain(|_, staged| staged.lease.chunk != id);
            }
        }
        // Reconcile returned workers: obsolete, unlisted copies cannot serve a
        // cached client location after that worker resumes.
        for (id, worker) in self.workers.iter_mut().enumerate() {
            if worker.online {
                worker.chunks.retain(|chunk_id, copy| {
                    metadata.chunks.get(chunk_id).is_some_and(|chunk| {
                        chunk.workers.contains(&id)
                            && copy.version == chunk.version
                            && copy.sequence == chunk.sequence
                    })
                });
            }
        }
        self.publish(metadata);
        Ok(())
    }
}
