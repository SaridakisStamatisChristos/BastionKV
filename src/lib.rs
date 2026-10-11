//! BastionKV: bounded, synchronous, crash-consistent embedded persistence.
//! See the repository durability model for filesystem assumptions and uncertain outcomes.
//!
//! ```no_run
//! use bastionkv::{Config, Database, Result};
//! fn example() -> Result<()> {
//!     let db = Database::create("new.db", Config::default())?;
//!     let mut tx = db.begin_write()?;
//!     tx.put(b"key", b"value")?;
//!     tx.commit()?;
//!     let snapshot = db.begin_read()?;
//!     db.delete(b"key")?;
//!     db.compact()?;
//!     assert_eq!(snapshot.get(b"key")?, Some(b"value".to_vec()));
//!     Ok(())
//! }
//! ```
#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
mod codec;
mod config;
mod disk;
mod error;
mod fault;
use codec::{Batch, Map};
pub use config::Config;
pub use disk::{dump_file, dump_wal, verify, Verification};
pub use error::{Error, Result};
use std::{
    fmt, fs,
    path::Path,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex, MutexGuard,
    },
};

pub type KeyValue = (Vec<u8>, Vec<u8>);
const MAX_SNAPSHOTS: usize = 128;
struct State {
    disk: disk::Disk,
    map: Arc<Map>,
    seq: u64,
    poisoned: bool,
    state_bytes: u64,
    compactions: u64,
    tombstones: u64,
    recovery_frames: u64,
    recovered_tail: u64,
}
struct Inner {
    state: Mutex<State>,
    cfg: Config,
    snapshots: AtomicUsize,
}
/// Clones share one process lock and serialization mutex. Open does not create missing databases.
#[derive(Clone)]
pub struct Database {
    inner: Arc<Inner>,
}
impl fmt::Debug for Database {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Database")
            .field("config", &self.inner.cfg)
            .finish_non_exhaustive()
    }
}
/// Process-local immutable MVCC version. Dropping it releases the snapshot slot and map.
pub struct ReadTransaction {
    inner: Arc<Inner>,
    map: Arc<Map>,
    sequence: u64,
}
impl fmt::Debug for ReadTransaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReadTransaction")
            .field("sequence", &self.sequence)
            .finish_non_exhaustive()
    }
}
impl Drop for ReadTransaction {
    fn drop(&mut self) {
        self.inner.snapshots.fetch_sub(1, Ordering::Relaxed);
    }
}
/// A write transaction reads its base snapshot and its staged batch.
#[derive(Debug)]
pub struct WriteTransaction {
    snapshot: ReadTransaction,
    batch: Batch,
}
#[derive(Debug)]
pub struct Statistics {
    pub live_keys: usize,
    pub active_snapshots: usize,
    pub last_sequence: u64,
    pub logical_state_bytes: u64,
    pub wal_bytes: u64,
    pub segment_count: usize,
    pub total_data_bytes: u64,
    pub reclaimable_wal_bytes: u64,
    pub tombstone_records_since_open_or_compact: u64,
    pub compactions_since_open: u64,
    pub recovered_transactions: u64,
    pub discarded_tail_bytes: u64,
}
fn guard(inner: &Inner) -> Result<MutexGuard<'_, State>> {
    let g = inner
        .state
        .lock()
        .map_err(|_| Error::Invariant("serialization mutex poisoned".into()))?;
    if g.poisoned {
        return Err(Error::Poisoned);
    }
    Ok(g)
}
impl Database {
    pub fn create(path: impl AsRef<Path>, cfg: Config) -> Result<Self> {
        let disk = disk::Disk::create(path.as_ref(), &cfg)?;
        Ok(Self::assemble(disk, Map::new(), 0, cfg, 0, 0))
    }
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let (disk, l) = disk::Disk::open(path.as_ref())?;
        Ok(Self::assemble(disk, l.map, l.seq, l.cfg, l.frames, l.tail))
    }
    fn assemble(
        disk: disk::Disk,
        map: Map,
        seq: u64,
        cfg: Config,
        recovery_frames: u64,
        recovered_tail: u64,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                cfg,
                snapshots: AtomicUsize::new(0),
                state: Mutex::new(State {
                    disk,
                    state_bytes: codec::footprint(&map),
                    map: Arc::new(map),
                    seq,
                    poisoned: false,
                    compactions: 0,
                    tombstones: 0,
                    recovery_frames,
                    recovered_tail,
                }),
            }),
        }
    }
    pub fn config(&self) -> &Config {
        &self.inner.cfg
    }
    pub fn begin_read(&self) -> Result<ReadTransaction> {
        let s = guard(&self.inner)?;
        // All increments run under the state mutex. Drops only decrement.
        // A prior increment happens-before this load through mutex release/acquire;
        // any concurrent decrement can only make this cap check conservative.
        if self.inner.snapshots.load(Ordering::Relaxed) >= MAX_SNAPSHOTS {
            return Err(Error::ResourceLimit(
                "128 concurrent read/write snapshots".into(),
            ));
        }
        self.inner.snapshots.fetch_add(1, Ordering::Relaxed);
        Ok(ReadTransaction {
            inner: self.inner.clone(),
            map: s.map.clone(),
            sequence: s.seq,
        })
    }
    pub fn begin_write(&self) -> Result<WriteTransaction> {
        Ok(WriteTransaction {
            snapshot: self.begin_read()?,
            batch: Batch::new(),
        })
    }
    pub fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        self.begin_read()?.get(key)
    }
    pub fn contains_key(&self, key: &[u8]) -> Result<bool> {
        Ok(self.get(key)?.is_some())
    }
    pub fn scan(&self, start: Option<&[u8]>, end: Option<&[u8]>) -> Result<Vec<KeyValue>> {
        self.begin_read()?.scan(start, end)
    }
    pub fn put(&self, key: &[u8], value: &[u8]) -> Result<u64> {
        let mut t = self.begin_write()?;
        t.put(key, value)?;
        t.commit()
    }
    pub fn delete(&self, key: &[u8]) -> Result<u64> {
        let mut t = self.begin_write()?;
        t.delete(key)?;
        t.commit()
    }
    /// Sync the already synchronous WAL. Does not checkpoint or weaken commit durability.
    pub fn flush(&self) -> Result<()> {
        let mut s = guard(&self.inner)?;
        if let Err(e) = s.disk.wal.sync_all() {
            s.poisoned = true;
            return Err(e.into());
        }
        Ok(())
    }
    /// Full checkpoint/rotation; blocks writers while snapshots continue reading immutable maps.
    pub fn compact(&self) -> Result<()> {
        let mut s = guard(&self.inner)?;
        let map = s.map.clone();
        let seq = s.seq;
        if let Err(e) = s.disk.checkpoint(seq, &map, &self.inner.cfg) {
            s.poisoned = true;
            return Err(e);
        }
        s.compactions += 1;
        s.tombstones = 0;
        Ok(())
    }
    pub fn statistics(&self) -> Result<Statistics> {
        let s = guard(&self.inner)?;
        let mut total = 0u64;
        let mut segments = 0;
        for e in fs::read_dir(&s.disk.path)? {
            let e = e?;
            let n = e.file_name();
            let n = n.to_string_lossy();
            if n.starts_with("segment-") && n.ends_with(".bks") {
                segments += 1;
            }
            if n != "LOCK" && e.file_type()?.is_file() {
                total = total
                    .checked_add(e.metadata()?.len())
                    .ok_or_else(|| Error::Invariant("data byte counter overflow".into()))?;
            }
        }
        let wal = s.disk.wal.metadata()?.len();
        Ok(Statistics {
            live_keys: s.map.len(),
            active_snapshots: self.inner.snapshots.load(Ordering::Relaxed),
            last_sequence: s.seq,
            logical_state_bytes: s.state_bytes,
            wal_bytes: wal,
            segment_count: segments,
            total_data_bytes: total,
            reclaimable_wal_bytes: wal,
            tombstone_records_since_open_or_compact: s.tombstones,
            compactions_since_open: s.compactions,
            recovered_transactions: s.recovery_frames,
            discarded_tail_bytes: s.recovered_tail,
        })
    }
}
impl ReadTransaction {
    pub fn sequence(&self) -> u64 {
        self.sequence
    }
    pub fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        self.inner.cfg.key(key)?;
        Ok(self.map.get(key).cloned())
    }
    pub fn contains_key(&self, key: &[u8]) -> Result<bool> {
        Ok(self.get(key)?.is_some())
    }
    /// Lexicographic byte order; inclusive lower bound, exclusive upper bound.
    pub fn scan(&self, start: Option<&[u8]>, end: Option<&[u8]>) -> Result<Vec<KeyValue>> {
        if start.zip(end).is_some_and(|(a, b)| a > b) {
            return Err(Error::InvalidInput("range start exceeds end".into()));
        }
        Ok(self
            .map
            .iter()
            .filter(|(k, _)| {
                start.is_none_or(|a| k.as_slice() >= a) && end.is_none_or(|b| k.as_slice() < b)
            })
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect())
    }
}
impl WriteTransaction {
    pub fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        self.snapshot.inner.cfg.key(key)?;
        match self.batch.get(key) {
            Some(v) => Ok(v.clone()),
            None => self.snapshot.get(key),
        }
    }
    pub fn put(&mut self, key: &[u8], value: &[u8]) -> Result<()> {
        self.snapshot.inner.cfg.key(key)?;
        self.snapshot.inner.cfg.value(value)?;
        self.stage(key, Some(value.to_vec()))
    }
    pub fn delete(&mut self, key: &[u8]) -> Result<()> {
        self.snapshot.inner.cfg.key(key)?;
        self.stage(key, None)
    }
    fn stage(&mut self, key: &[u8], value: Option<Vec<u8>>) -> Result<()> {
        let old = self.batch.insert(key.to_vec(), value);
        // Bound staged data before commit, not just encoded files. Replacement rollback preserves the batch.
        let cost: u64 = self
            .batch
            .iter()
            .map(|(k, v)| (k.len() + v.as_ref().map_or(0, Vec::len) + 16) as u64)
            .sum();
        if cost > self.snapshot.inner.cfg.max_state_bytes {
            match old {
                Some(v) => {
                    self.batch.insert(key.to_vec(), v);
                }
                None => {
                    self.batch.remove(key);
                }
            }
            return Err(Error::ResourceLimit("staged transaction budget".into()));
        }
        Ok(())
    }
    pub fn rollback(self) {}
    /// Success is returned only after WAL sync and atomic in-memory publication.
    /// Any I/O failure has an unknown atomic outcome and poisons the shared handle.
    pub fn commit(self) -> Result<u64> {
        let mut s = guard(&self.snapshot.inner)?;
        if s.seq != self.snapshot.sequence {
            return Err(Error::Conflict {
                expected: self.snapshot.sequence,
                actual: s.seq,
            });
        }
        if self.batch.is_empty() {
            return Ok(s.seq);
        }
        let seq = s
            .seq
            .checked_add(1)
            .ok_or_else(|| Error::Invariant("sequence exhausted".into()))?;
        let next = codec::apply(&s.map, &self.batch, &self.snapshot.inner.cfg)?;
        let frame = codec::encode(seq, &self.batch, false, &self.snapshot.inner.cfg)?;
        if let Err(e) = s
            .disk
            .append(&frame)
            .and_then(|()| fault::point("before_publish"))
        {
            s.poisoned = true;
            return Err(e);
        }
        s.state_bytes = codec::footprint(&next);
        s.map = Arc::new(next);
        s.seq = seq;
        s.tombstones = s
            .tombstones
            .saturating_add(self.batch.values().filter(|v| v.is_none()).count() as u64);
        if let Err(e) = fault::point("after_publish") {
            s.poisoned = true;
            return Err(e);
        }
        Ok(seq)
    }
}
