use crate::{
    codec::{self, Batch, Map, ReadFrame},
    fault, Config, Error, Result,
};
use std::{
    fs::{self, File, OpenOptions, TryLockError},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

pub(crate) fn wal_name(gen: u64) -> String {
    format!("wal-{gen:020}.bkw")
}
pub(crate) fn segment_name(gen: u64) -> String {
    format!("segment-{gen:020}.bks")
}
pub(crate) fn regular(path: &Path) -> Result<File> {
    let meta = fs::symlink_metadata(path)
        .map_err(|e| Error::io(format!("metadata {}", path.display()), e))?;
    if !meta.file_type().is_file() {
        return Err(Error::corrupt(
            path.display().to_string(),
            0,
            "expected regular non-symlink file",
        ));
    }
    File::open(path).map_err(|e| Error::io(format!("open {}", path.display()), e))
}
fn lock(path: &Path, create: bool, shared: bool) -> Result<File> {
    if !create || fs::symlink_metadata(path.join("LOCK")).is_ok() {
        regular(&path.join("LOCK"))?;
    }
    let f = OpenOptions::new()
        .read(true)
        .write(true)
        .create(create)
        .truncate(false)
        .open(path.join("LOCK"))?;
    let outcome = if shared {
        f.try_lock_shared()
    } else {
        f.try_lock()
    };
    match outcome {
        Ok(()) => Ok(f),
        Err(TryLockError::WouldBlock) => Err(Error::Locked),
        Err(TryLockError::Error(e)) => Err(e.into()),
    }
}
pub(crate) fn sync_dir(path: &Path) -> Result<()> {
    File::open(path)?.sync_all()?;
    Ok(())
}
fn write_new(path: &Path, bytes: &[u8], middle: &str) -> Result<()> {
    let mut f = OpenOptions::new().write(true).create_new(true).open(path)?;
    let half = bytes.len() / 2;
    f.write_all(&bytes[..half])?;
    fault::point(middle)?;
    f.write_all(&bytes[half..])?;
    f.sync_all()?;
    Ok(())
}
fn manifest(path: &Path, gen: u64, seq: u64, cfg: &Config) -> Result<()> {
    let temp = path.join("MANIFEST.tmp");
    if temp.try_exists()? {
        fs::remove_file(&temp)?;
    }
    write_new(&temp, &codec::metadata(gen, seq, cfg), "manifest_partial")?;
    fault::point("before_manifest_replace")?;
    fs::rename(temp, path.join("MANIFEST"))?;
    fault::point("after_manifest_replace")?;
    sync_dir(path)?;
    fault::point("after_manifest_sync")?;
    Ok(())
}
pub(crate) struct Loaded {
    pub gen: u64,
    pub seq: u64,
    pub map: Map,
    pub cfg: Config,
    pub frames: u64,
    pub valid_wal: u64,
    pub tail: u64,
}
pub(crate) fn load(path: &Path) -> Result<Loaded> {
    let mut meta = regular(&path.join("MANIFEST"))?;
    if meta.metadata()?.len() != codec::META_SIZE as u64 {
        return Err(Error::corrupt("MANIFEST", 0, "metadata size"));
    }
    let mut b = [0; codec::META_SIZE];
    meta.read_exact(&mut b)?;
    let (gen, checkpoint, cfg) = codec::parse_meta(&b)?;
    let sname = segment_name(gen);
    let mut sf = regular(&path.join(&sname))?;
    let mut map = match codec::read_frame(&mut sf, &sname, true, &cfg)? {
        ReadFrame::Complete(f) if f.seq == checkpoint => codec::apply(&Map::new(), &f.batch, &cfg)
            .map_err(|e| Error::corrupt(&sname, f.offset, e.to_string()))?,
        _ => {
            return Err(Error::corrupt(
                &sname,
                0,
                "checkpoint sequence or incomplete segment",
            ))
        }
    };
    if !matches!(
        codec::read_frame(&mut sf, &sname, true, &cfg)?,
        ReadFrame::End
    ) {
        return Err(Error::corrupt(
            &sname,
            sf.stream_position()?,
            "segment trailing data",
        ));
    }
    let wname = wal_name(gen);
    let mut wf = regular(&path.join(&wname))?;
    let mut seq = checkpoint;
    let mut frames = 0;
    let mut valid = 0;
    let mut tail = 0;
    loop {
        match codec::read_frame(&mut wf, &wname, false, &cfg)? {
            ReadFrame::End => break,
            ReadFrame::Tail(offset) => {
                tail = wf.metadata()?.len() - offset;
                break;
            }
            ReadFrame::Complete(f) => {
                if seq.checked_add(1) != Some(f.seq) || f.batch.is_empty() {
                    return Err(Error::corrupt(
                        &wname,
                        f.offset,
                        "duplicate/reused/nonconsecutive sequence or empty transaction",
                    ));
                }
                map = codec::apply(&map, &f.batch, &cfg)
                    .map_err(|e| Error::corrupt(&wname, f.offset, e.to_string()))?;
                seq = f.seq;
                frames += 1;
                valid = f.end;
            }
        }
    }
    Ok(Loaded {
        gen,
        seq,
        map,
        cfg,
        frames,
        valid_wal: valid,
        tail,
    })
}
pub(crate) struct Disk {
    pub path: PathBuf,
    pub gen: u64,
    pub wal: File,
    pub _lock: File,
}
impl Disk {
    pub fn create(path: &Path, cfg: &Config) -> Result<Self> {
        cfg.validate()?;
        if !path.try_exists()? {
            fs::create_dir(path)?;
            sync_dir(
                path.parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .unwrap_or(Path::new(".")),
            )?;
        }
        if fs::symlink_metadata(path)?.file_type().is_symlink() {
            return Err(Error::InvalidInput("database path is a symlink".into()));
        }
        let guard = lock(path, true, false)?;
        if fs::read_dir(path)?.any(|e| e.map_or(true, |e| e.file_name() != "LOCK")) {
            return Err(Error::InvalidInput(
                "init requires empty directory (except LOCK)".into(),
            ));
        }
        let gen = 1;
        write_new(
            &path.join(segment_name(gen)),
            &codec::encode(0, &Batch::new(), true, cfg)?,
            "init_segment_partial",
        )?;
        let wal = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path.join(wal_name(gen)))?;
        wal.sync_all()?;
        sync_dir(path)?;
        manifest(path, gen, 0, cfg)?;
        Ok(Self {
            path: path.to_path_buf(),
            gen,
            wal,
            _lock: guard,
        })
    }
    pub fn open(path: &Path) -> Result<(Self, Loaded)> {
        if fs::symlink_metadata(path)?.file_type().is_symlink() {
            return Err(Error::InvalidInput("database path is a symlink".into()));
        }
        let guard = lock(path, false, false)?;
        let loaded = load(path)?;
        let mut wal = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path.join(wal_name(loaded.gen)))?;
        if loaded.tail > 0 {
            wal.set_len(loaded.valid_wal)?;
            wal.sync_all()?;
        }
        wal.seek(SeekFrom::End(0))?;
        let disk = Self {
            path: path.to_path_buf(),
            gen: loaded.gen,
            wal,
            _lock: guard,
        };
        // Only remove names owned by the format, after authoritative generation is recovered.
        disk.cleanup()?;
        Ok((disk, loaded))
    }
    pub fn append(&mut self, frame: &[u8]) -> Result<()> {
        fault::point("before_wal_append")?;
        let half = frame.len() / 2;
        self.wal.write_all(&frame[..half])?;
        fault::point("wal_partial")?;
        self.wal.write_all(&frame[half..])?;
        fault::point("after_wal_write")?;
        self.wal.sync_all()?;
        fault::point("after_wal_sync")?;
        Ok(())
    }
    pub fn checkpoint(&mut self, seq: u64, map: &Map, cfg: &Config) -> Result<()> {
        let gen = self
            .gen
            .checked_add(1)
            .ok_or_else(|| Error::Invariant("generation exhausted".into()))?;
        let batch: Batch = map
            .iter()
            .map(|(k, v)| (k.clone(), Some(v.clone())))
            .collect();
        let bytes = codec::encode(seq, &batch, true, cfg)?;
        let target = self.path.join(segment_name(gen));
        let temp = target.with_extension("tmp");
        // A failed checkpoint poisons its handle; reopen cleans uncommitted generation files.
        write_new(&temp, &bytes, "segment_partial")?;
        fault::point("after_segment_sync")?;
        fs::rename(&temp, &target)?;
        sync_dir(&self.path)?;
        fault::point("after_segment_rename")?;
        let new_wal = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(self.path.join(wal_name(gen)))?;
        new_wal.sync_all()?;
        sync_dir(&self.path)?;
        fault::point("after_new_wal_sync")?;
        manifest(&self.path, gen, seq, cfg)?;
        self.wal = new_wal;
        self.gen = gen;
        fault::point("before_cleanup")?;
        self.cleanup()?;
        Ok(())
    }
    fn cleanup(&self) -> Result<()> {
        for entry in fs::read_dir(&self.path)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if let Some(gen) = owned_generation(&name) {
                if gen != self.gen || name.ends_with(".tmp") {
                    fs::remove_file(entry.path())?;
                    fault::point("during_cleanup")?;
                }
            } else if name == "MANIFEST.tmp" {
                fs::remove_file(entry.path())?;
            }
        }
        sync_dir(&self.path)
    }
}
fn owned_generation(name: &str) -> Option<u64> {
    let rest = name
        .strip_prefix("wal-")
        .or_else(|| name.strip_prefix("segment-"))?;
    let (digits, ext) = rest.split_once('.')?;
    if digits.len() != 20
        || !digits.bytes().all(|b| b.is_ascii_digit())
        || !matches!(ext, "bkw" | "bks" | "tmp")
    {
        return None;
    }
    digits.parse().ok()
}

/// Read-only integrity result. Orphan files are recoverable warnings, never authoritative data.
#[derive(Debug)]
pub struct Verification {
    pub last_sequence: u64,
    pub live_keys: usize,
    pub wal_frames: u64,
    pub incomplete_tail_bytes: u64,
    pub orphan_files: Vec<String>,
}
pub fn verify(path: impl AsRef<Path>) -> Result<Verification> {
    let path = path.as_ref();
    let _lock = lock(path, false, true)?;
    let loaded = load(path)?;
    let required = [
        "LOCK".to_string(),
        "MANIFEST".to_string(),
        wal_name(loaded.gen),
        segment_name(loaded.gen),
    ];
    let mut orphans = Vec::new();
    for e in fs::read_dir(path)? {
        let e = e?;
        let name = e.file_name().to_string_lossy().to_string();
        if !required.contains(&name) {
            orphans.push(name);
        }
    }
    orphans.sort();
    Ok(Verification {
        last_sequence: loaded.seq,
        live_keys: loaded.map.len(),
        wal_frames: loaded.frames,
        incomplete_tail_bytes: loaded.tail,
        orphan_files: orphans,
    })
}
/// Inspect frame boundaries, record types, byte lengths, and sequence numbers without writes.
pub fn dump_file(path: impl AsRef<Path>, segment: bool, out: &mut impl Write) -> Result<()> {
    let path = path.as_ref();
    let mut f = regular(path)?;
    // Inspection uses absolute format limits; opening/verification use persisted tighter limits.
    let cfg = Config {
        max_key_bytes: 1_048_576,
        max_value_bytes: 16_777_216,
        max_frame_bytes: 268_435_456,
        max_state_bytes: 268_435_444,
    };
    loop {
        match codec::read_frame(&mut f, &path.display().to_string(), segment, &cfg)? {
            ReadFrame::End => break,
            ReadFrame::Tail(o) => {
                writeln!(out, "incomplete tail at {o}")?;
                if segment {
                    return Err(Error::corrupt(
                        path.display().to_string(),
                        o,
                        "incomplete segment",
                    ));
                }
                break;
            }
            ReadFrame::Complete(frame) => {
                writeln!(
                    out,
                    "offset={} end={} seq={} records={}",
                    frame.offset,
                    frame.end,
                    frame.seq,
                    frame.batch.len()
                )?;
                for (k, v) in frame.batch {
                    writeln!(
                        out,
                        "  {} key_bytes={} value_bytes={}",
                        if v.is_some() { "PUT" } else { "DELETE" },
                        k.len(),
                        v.as_ref().map_or(0, Vec::len)
                    )?;
                }
            }
        }
    }
    Ok(())
}
pub fn dump_wal(path: impl AsRef<Path>, out: &mut impl Write) -> Result<()> {
    let path = path.as_ref();
    let _lock = lock(path, false, true)?;
    let loaded = load(path)?;
    dump_file(path.join(wal_name(loaded.gen)), false, out)
}
