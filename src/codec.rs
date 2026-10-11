use crate::{Config, Error, Result};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Seek},
};

pub(crate) type Map = BTreeMap<Vec<u8>, Vec<u8>>;
pub(crate) type Batch = BTreeMap<Vec<u8>, Option<Vec<u8>>>;
pub(crate) const HEADER: usize = 24;
const MARKER: &[u8; 8] = b"COMMIT01";

/// CRC32C (Castagnoli), independent of architecture and Rust object layout.
pub(crate) fn crc(bytes: &[u8]) -> u32 {
    let mut c = !0u32;
    for b in bytes {
        c ^= u32::from(*b);
        for _ in 0..8 {
            c = (c >> 1) ^ (0x82f63b78 & (0u32.wrapping_sub(c & 1)));
        }
    }
    !c
}
pub(crate) fn footprint(map: &Map) -> u64 {
    map.iter()
        .map(|(k, v)| (k.len() + v.len() + 16) as u64)
        .sum()
}
pub(crate) fn apply(map: &Map, batch: &Batch, cfg: &Config) -> Result<Map> {
    let mut next = map.clone();
    for (k, v) in batch {
        cfg.key(k)?;
        match v {
            Some(v) => {
                cfg.value(v)?;
                next.insert(k.clone(), v.clone());
            }
            None => {
                next.remove(k);
            }
        }
    }
    if footprint(&next) > cfg.max_state_bytes {
        return Err(Error::ResourceLimit("live state budget".into()));
    }
    Ok(next)
}
pub(crate) fn encode(seq: u64, entries: &Batch, segment: bool, cfg: &Config) -> Result<Vec<u8>> {
    let len = entries.iter().try_fold(12usize, |n, (k, v)| {
        cfg.key(k)?;
        if let Some(v) = v {
            cfg.value(v)?;
        }
        n.checked_add(9 + k.len() + v.as_ref().map_or(0, Vec::len))
            .ok_or_else(|| Error::ResourceLimit("encoded frame overflow".into()))
    })?;
    if len > cfg.max_frame_bytes as usize {
        return Err(Error::ResourceLimit("transaction frame budget".into()));
    }
    let count =
        u32::try_from(entries.len()).map_err(|_| Error::ResourceLimit("record count".into()))?;
    let mut p = Vec::with_capacity(len);
    p.extend(count.to_le_bytes());
    for (k, v) in entries {
        p.push(if v.is_some() { 1 } else { 2 });
        p.extend((k.len() as u32).to_le_bytes());
        p.extend((v.as_ref().map_or(0, Vec::len) as u32).to_le_bytes());
        p.extend(k);
        if let Some(v) = v {
            p.extend(v);
        }
    }
    p.extend(MARKER);
    let mut out = Vec::with_capacity(HEADER + p.len());
    out.extend(if segment { b"BKS1" } else { b"BKW1" });
    out.extend((p.len() as u32).to_le_bytes());
    out.extend(crc(&p).to_le_bytes());
    out.extend(seq.to_le_bytes());
    out.extend(crc(&out).to_le_bytes());
    out.extend(p);
    Ok(out)
}
pub(crate) struct Frame {
    pub seq: u64,
    pub batch: Batch,
    pub offset: u64,
    pub end: u64,
}
pub(crate) enum ReadFrame {
    End,
    Tail(u64),
    Complete(Frame),
}
fn u32_at(p: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([p[i], p[i + 1], p[i + 2], p[i + 3]])
}
fn u64_at(p: &[u8], i: usize) -> u64 {
    u64::from_le_bytes([
        p[i],
        p[i + 1],
        p[i + 2],
        p[i + 3],
        p[i + 4],
        p[i + 5],
        p[i + 6],
        p[i + 7],
    ])
}
pub(crate) fn read_frame(
    f: &mut File,
    name: &str,
    segment: bool,
    cfg: &Config,
) -> Result<ReadFrame> {
    let offset = f.stream_position()?;
    let size = f.metadata()?.len();
    let remain = size
        .checked_sub(offset)
        .ok_or_else(|| Error::corrupt(name, offset, "offset beyond file"))?;
    if remain == 0 {
        return Ok(ReadFrame::End);
    }
    if remain < HEADER as u64 {
        return Ok(ReadFrame::Tail(offset));
    }
    let mut h = [0u8; HEADER];
    f.read_exact(&mut h)
        .map_err(|e| Error::io(format!("read {name} header at {offset}"), e))?;
    let bad = |reason| Error::corrupt(name, offset, reason);
    if &h[..4] != if segment { b"BKS1" } else { b"BKW1" } {
        return Err(bad("wrong frame type/magic"));
    }
    if crc(&h[..20]) != u32_at(&h, 20) {
        return Err(bad("header checksum"));
    }
    let len = u32_at(&h, 4) as usize;
    if len < 12 || len > cfg.max_frame_bytes as usize {
        return Err(bad("payload length outside bounds"));
    }
    if len as u64 > remain - HEADER as u64 {
        return Ok(ReadFrame::Tail(offset));
    }
    let mut p = vec![0; len];
    f.read_exact(&mut p)
        .map_err(|e| Error::io(format!("read {name} payload at {offset}"), e))?;
    if crc(&p) != u32_at(&h, 8) {
        return Err(bad("payload checksum"));
    }
    if &p[len - 8..] != MARKER {
        return Err(bad("commit trailer"));
    }
    let count = u32_at(&p, 0) as usize;
    if count > (len - 12) / 9 {
        return Err(bad("impossible record count"));
    }
    let mut i = 4;
    let mut batch = Batch::new();
    let mut previous: Option<Vec<u8>> = None;
    for _ in 0..count {
        if i > len - 8 || len - 8 - i < 9 {
            return Err(bad("record header exceeds payload"));
        }
        let tag = p[i];
        let kl = u32_at(&p, i + 1) as usize;
        let vl = u32_at(&p, i + 5) as usize;
        i += 9;
        if kl == 0
            || kl > cfg.max_key_bytes as usize
            || vl > cfg.max_value_bytes as usize
            || (tag != 1 && tag != 2)
            || (tag == 2 && (vl != 0 || segment))
        {
            return Err(bad("illegal record type/key/value length"));
        }
        let end = i
            .checked_add(kl)
            .and_then(|x| x.checked_add(vl))
            .ok_or_else(|| bad("record length overflow"))?;
        if end > len - 8 {
            return Err(bad("record range exceeds payload"));
        }
        let key = p[i..i + kl].to_vec();
        if previous.as_ref().is_some_and(|prev| prev >= &key) {
            return Err(bad("duplicate or unsorted key/index"));
        }
        previous = Some(key.clone());
        let value = if tag == 1 {
            Some(p[i + kl..end].to_vec())
        } else {
            None
        };
        batch.insert(key, value);
        i = end;
    }
    if i != len - 8 {
        return Err(bad("unclaimed payload bytes"));
    }
    Ok(ReadFrame::Complete(Frame {
        seq: u64_at(&h, 12),
        batch,
        offset,
        end: offset + HEADER as u64 + len as u64,
    }))
}

pub(crate) const META_SIZE: usize = 52;
pub(crate) fn metadata(gen: u64, seq: u64, cfg: &Config) -> Vec<u8> {
    let mut b = Vec::with_capacity(META_SIZE);
    b.extend(b"BKMETA01");
    b.extend(1u32.to_le_bytes());
    b.extend(gen.to_le_bytes());
    b.extend(seq.to_le_bytes());
    b.extend(cfg.max_key_bytes.to_le_bytes());
    b.extend(cfg.max_value_bytes.to_le_bytes());
    b.extend(cfg.max_frame_bytes.to_le_bytes());
    b.extend(cfg.max_state_bytes.to_le_bytes());
    b.extend(crc(&b).to_le_bytes());
    b
}
pub(crate) fn parse_meta(b: &[u8]) -> Result<(u64, u64, Config)> {
    let bad = |s| Error::corrupt("MANIFEST", 0, s);
    if b.len() != META_SIZE || &b[..8] != b"BKMETA01" {
        return Err(bad("metadata length/magic"));
    }
    if crc(&b[..48]) != u32_at(b, 48) {
        return Err(bad("metadata checksum"));
    }
    let v = u32_at(b, 8);
    if v != 1 {
        return Err(Error::UnsupportedFormat(v));
    }
    let cfg = Config {
        max_key_bytes: u32_at(b, 28),
        max_value_bytes: u32_at(b, 32),
        max_frame_bytes: u32_at(b, 36),
        max_state_bytes: u64_at(b, 40),
    };
    cfg.validate()
        .map_err(|_| bad("invalid persisted limits"))?;
    let gen = u64_at(b, 12);
    if gen == 0 {
        return Err(bad("zero generation"));
    }
    Ok((gen, u64_at(b, 20), cfg))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn castagnoli_known_vector() {
        assert_eq!(crc(b"123456789"), 0xe3069283);
    }
    #[test]
    fn frame_roundtrip_with_binary_empty_values_and_tombstones() {
        let cfg = Config::default();
        for seed in 0..32u64 {
            let mut expected = Batch::new();
            for n in 0..16u64 {
                let key = (seed * 16 + n).to_be_bytes().to_vec();
                let value = if n.is_multiple_of(3) {
                    None
                } else {
                    Some((0..(n * 7)).map(|i| (i ^ seed) as u8).collect())
                };
                expected.insert(key, value);
            }
            expected.insert(vec![0, 255], Some(Vec::new()));
            let bytes = encode(seed + 1, &expected, false, &cfg).unwrap();
            let path =
                std::env::temp_dir().join(format!("bastion-frame-{}-{seed}", std::process::id()));
            let mut writer = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .unwrap();
            std::io::Write::write_all(&mut writer, &bytes).unwrap();
            drop(writer);
            let mut reader = File::open(&path).unwrap();
            let ReadFrame::Complete(frame) =
                read_frame(&mut reader, "roundtrip", false, &cfg).unwrap()
            else {
                panic!("complete encoded frame expected");
            };
            assert_eq!(frame.seq, seed + 1);
            assert_eq!(frame.batch, expected);
            assert_eq!(frame.end, bytes.len() as u64);
            assert!(matches!(
                read_frame(&mut reader, "roundtrip", false, &cfg).unwrap(),
                ReadFrame::End
            ));
            std::fs::remove_file(path).unwrap();
        }
    }
    #[test]
    fn metadata_roundtrip_and_damage() {
        let c = Config::default();
        let b = metadata(2, 19, &c);
        assert_eq!(parse_meta(&b).unwrap(), (2, 19, c));
        for i in 0..b.len() {
            let mut x = b.clone();
            x[i] ^= 1;
            assert!(parse_meta(&x).is_err());
        }
    }
}
