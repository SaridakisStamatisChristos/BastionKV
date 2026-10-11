mod common;
use bastionkv::{Config, Database};
use common::Temp;
use std::fs;
fn setup() -> Temp {
    let t = Temp::new();
    let db = Database::create(t.db(), Config::default()).unwrap();
    db.put(b"base", b"safe").unwrap();
    let mut tx = db.begin_write().unwrap();
    tx.put(b"left", b"new").unwrap();
    tx.put(b"right", b"new").unwrap();
    tx.commit().unwrap();
    drop(db);
    t
}
fn crc(b: &[u8]) -> u32 {
    let mut x = u32::MAX;
    for v in b {
        x ^= u32::from(*v);
        for _ in 0..8 {
            x = if x & 1 == 1 {
                (x >> 1) ^ 0x82f63b78
            } else {
                x >> 1
            };
        }
    }
    !x
}
fn reseal(frame: &mut [u8]) {
    let p = crc(&frame[24..]).to_le_bytes();
    frame[8..12].copy_from_slice(&p);
    let h = crc(&frame[..20]).to_le_bytes();
    frame[20..24].copy_from_slice(&h);
}
#[test]
fn every_wal_tail_cut_recovers_atomic_prefix_and_is_idempotent() {
    let t = setup();
    let wal = t.wal();
    let bytes = fs::read(&wal).unwrap();
    let first = 24 + u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    for cut in first..bytes.len() {
        fs::write(&wal, &bytes[..cut]).unwrap();
        let before = fs::read(&wal).unwrap();
        let v = bastionkv::verify(t.db()).unwrap();
        assert_eq!(v.last_sequence, 1);
        assert_eq!(fs::read(&wal).unwrap(), before, "verify mutated");
        let db = Database::open(t.db()).unwrap();
        assert_eq!(db.get(b"base").unwrap(), Some(b"safe".to_vec()));
        assert_eq!(db.get(b"left").unwrap(), None);
        assert_eq!(db.get(b"right").unwrap(), None);
        drop(db);
        let recovered = fs::read(&wal).unwrap();
        for _ in 0..2 {
            let db = Database::open(t.db()).unwrap();
            drop(db);
            assert_eq!(fs::read(&wal).unwrap(), recovered);
        }
    }
}
#[test]
fn every_complete_wal_byte_flip_is_detected() {
    let t = setup();
    let wal = t.wal();
    let bytes = fs::read(&wal).unwrap();
    for i in 0..bytes.len() {
        let mut x = bytes.clone();
        x[i] ^= 1;
        fs::write(&wal, x).unwrap();
        assert!(bastionkv::verify(t.db()).is_err(), "offset={i}");
        assert!(Database::open(t.db()).is_err(), "offset={i}");
    }
}
#[test]
fn segment_every_byte_cut_and_bit_flip_fail_closed() {
    let t = setup();
    let db = Database::open(t.db()).unwrap();
    db.compact().unwrap();
    drop(db);
    let p = t.segment();
    let bytes = fs::read(&p).unwrap();
    for i in 0..bytes.len() {
        fs::write(&p, &bytes[..i]).unwrap();
        assert!(bastionkv::verify(t.db()).is_err());
        let mut x = bytes.clone();
        x[i] ^= 1;
        fs::write(&p, x).unwrap();
        assert!(bastionkv::verify(t.db()).is_err());
    }
}
#[test]
fn duplicate_transaction_and_sequence_rollback_rejected() {
    let t = setup();
    let wal = t.wal();
    let bytes = fs::read(&wal).unwrap();
    let first = 24 + u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let mut x = bytes.clone();
    x.extend_from_slice(&bytes[first..]);
    fs::write(&wal, x).unwrap();
    assert!(bastionkv::verify(t.db()).is_err());
    let mut x = bytes.clone();
    x[first + 12..first + 20].copy_from_slice(&1u64.to_le_bytes());
    let h = crc(&x[first..first + 20]).to_le_bytes();
    x[first + 20..first + 24].copy_from_slice(&h);
    fs::write(wal, x).unwrap();
    assert!(bastionkv::verify(t.db()).is_err());
}
#[test]
fn plausible_checksums_do_not_bypass_structural_validation() {
    let t = setup();
    let wal = t.wal();
    let bytes = fs::read(&wal).unwrap();
    for mutation in 0..6 {
        let mut x = bytes.clone();
        let first = 24 + u32::from_le_bytes(x[4..8].try_into().unwrap()) as usize;
        let f = &mut x[..first];
        match mutation {
            0 => f[28] = 7,
            1 => f[29..33].copy_from_slice(&u32::MAX.to_le_bytes()),
            2 => f[33..37].copy_from_slice(&u32::MAX.to_le_bytes()),
            3 => f[24..28].copy_from_slice(&u32::MAX.to_le_bytes()),
            4 => f[first - 1] = 0,
            5 => f[28] = 2,
            _ => unreachable!(),
        }
        reseal(f);
        fs::write(&wal, x).unwrap();
        assert!(bastionkv::verify(t.db()).is_err(), "mutation={mutation}");
    }
}
#[test]
fn malicious_length_headers_and_metadata_never_allocate_unbounded() {
    let t = setup();
    let wal = t.wal();
    let mut bytes = fs::read(&wal).unwrap();
    bytes[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
    let h = crc(&bytes[..20]).to_le_bytes();
    bytes[20..24].copy_from_slice(&h);
    fs::write(&wal, bytes).unwrap();
    assert!(bastionkv::verify(t.db()).is_err());
    let t = setup();
    let p = t.db().join("MANIFEST");
    let orig = fs::read(&p).unwrap();
    for (offset, val) in [(36, u32::MAX), (8, 99)] {
        let mut m = orig.clone();
        m[offset..offset + 4].copy_from_slice(&val.to_le_bytes());
        let c = crc(&m[..48]).to_le_bytes();
        m[48..].copy_from_slice(&c);
        fs::write(&p, m).unwrap();
        assert!(bastionkv::verify(t.db()).is_err());
    }
    let mut m = orig;
    m[12..20].copy_from_slice(&123u64.to_le_bytes());
    let c = crc(&m[..48]).to_le_bytes();
    m[48..].copy_from_slice(&c);
    fs::write(p, m).unwrap();
    assert!(bastionkv::verify(t.db()).is_err());
}
#[test]
fn missing_manifest_or_referenced_file_is_not_implicitly_recreated() {
    let t = setup();
    fs::remove_file(t.segment()).unwrap();
    assert!(Database::open(t.db()).is_err());
    assert!(bastionkv::verify(t.db()).is_err());
    let t = setup();
    fs::remove_file(t.db().join("MANIFEST")).unwrap();
    assert!(Database::open(t.db()).is_err());
}
#[cfg(unix)]
#[test]
fn symlink_and_invalid_files_are_rejected() {
    let t = setup();
    let p = t.wal();
    fs::remove_file(&p).unwrap();
    std::os::unix::fs::symlink("/dev/zero", p).unwrap();
    assert!(bastionkv::verify(t.db()).is_err());
    let t = setup();
    fs::remove_file(t.db().join("MANIFEST")).unwrap();
    fs::create_dir(t.db().join("MANIFEST")).unwrap();
    assert!(Database::open(t.db()).is_err());
}
#[test]
fn checksum_valid_unsorted_record_index_is_rejected() {
    let t = setup();
    let wal = t.wal();
    let mut b = fs::read(&wal).unwrap();
    let first = 24 + u32::from_le_bytes(b[4..8].try_into().unwrap()) as usize;
    let frame = &mut b[first..];
    // Second frame's first key is left. Make it zzzz so the following right key is out of order.
    frame[37..41].copy_from_slice(b"zzzz");
    reseal(frame);
    fs::write(wal, b).unwrap();
    assert!(bastionkv::verify(t.db()).is_err());
}
#[test]
fn sequence_and_generation_exhaustion_never_wrap() {
    let t = setup();
    let db = Database::open(t.db()).unwrap();
    db.compact().unwrap();
    drop(db);
    let manifest = t.db().join("MANIFEST");
    let mut m = fs::read(&manifest).unwrap();
    m[20..28].copy_from_slice(&u64::MAX.to_le_bytes());
    let c = crc(&m[..48]).to_le_bytes();
    m[48..].copy_from_slice(&c);
    fs::write(&manifest, m).unwrap();
    let p = t.segment();
    let mut s = fs::read(&p).unwrap();
    s[12..20].copy_from_slice(&u64::MAX.to_le_bytes());
    reseal(&mut s);
    fs::write(p, s).unwrap();
    let db = Database::open(t.db()).unwrap();
    assert!(matches!(
        db.put(b"x", b"y"),
        Err(bastionkv::Error::Invariant(_))
    ));
    assert_eq!(db.statistics().unwrap().last_sequence, u64::MAX);
    drop(db);
    let mut m = fs::read(&manifest).unwrap();
    let oldgen = u64::from_le_bytes(m[12..20].try_into().unwrap());
    m[12..20].copy_from_slice(&u64::MAX.to_le_bytes());
    let c = crc(&m[..48]).to_le_bytes();
    m[48..].copy_from_slice(&c);
    fs::write(manifest, m).unwrap();
    fs::rename(
        t.db().join(format!("wal-{oldgen:020}.bkw")),
        t.db().join(format!("wal-{:020}.bkw", u64::MAX)),
    )
    .unwrap();
    fs::rename(
        t.db().join(format!("segment-{oldgen:020}.bks")),
        t.db().join(format!("segment-{:020}.bks", u64::MAX)),
    )
    .unwrap();
    let db = Database::open(t.db()).unwrap();
    assert!(matches!(db.compact(), Err(bastionkv::Error::Invariant(_))));
    drop(db);
    assert_eq!(bastionkv::verify(t.db()).unwrap().last_sequence, u64::MAX);
}
