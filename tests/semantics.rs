mod common;
use bastionkv::{Config, Database, Error};
use common::Temp;
#[test]
fn basic_binary_persistence_atomicity_and_rollback() {
    let t = Temp::new();
    let db = Database::create(t.db(), Config::default()).unwrap();
    let mut w = db.begin_write().unwrap();
    w.put(&[0, 255], &[]).unwrap();
    w.put(b"b", b"two").unwrap();
    assert_eq!(w.get(&[0, 255]).unwrap(), Some(vec![]));
    assert!(!db.contains_key(b"b").unwrap());
    assert_eq!(w.commit().unwrap(), 1);
    let mut w = db.begin_write().unwrap();
    w.delete(b"b").unwrap();
    w.rollback();
    db.flush().unwrap();
    drop(db);
    let db = Database::open(t.db()).unwrap();
    assert_eq!(db.get(&[0, 255]).unwrap(), Some(vec![]));
    assert_eq!(db.get(b"b").unwrap(), Some(b"two".to_vec()));
}
#[test]
fn snapshot_visibility_delete_and_compaction() {
    let t = Temp::new();
    let db = Database::create(t.db(), Config::default()).unwrap();
    db.put(b"a", b"old").unwrap();
    let s = db.begin_read().unwrap();
    db.put(b"a", b"new").unwrap();
    db.delete(b"a").unwrap();
    db.compact().unwrap();
    assert_eq!(s.get(b"a").unwrap(), Some(b"old".to_vec()));
    assert_eq!(
        s.scan(None, None).unwrap(),
        vec![(b"a".to_vec(), b"old".to_vec())]
    );
    assert_eq!(db.get(b"a").unwrap(), None);
    drop(s);
    drop(db);
    for _ in 0..3 {
        let db = Database::open(t.db()).unwrap();
        assert_eq!(db.get(b"a").unwrap(), None);
        db.compact().unwrap();
    }
}
#[test]
fn conflict_including_disjoint_keys_and_empty_transaction() {
    let t = Temp::new();
    let db = Database::create(t.db(), Config::default()).unwrap();
    let mut a = db.begin_write().unwrap();
    let mut b = db.begin_write().unwrap();
    let empty = db.begin_write().unwrap();
    a.put(b"a", b"a").unwrap();
    b.put(b"b", b"b").unwrap();
    a.commit().unwrap();
    assert!(matches!(b.commit(), Err(Error::Conflict { .. })));
    assert!(matches!(empty.commit(), Err(Error::Conflict { .. })));
    assert_eq!(db.begin_write().unwrap().commit().unwrap(), 1);
}
#[test]
fn ranges_limits_and_stats() {
    let t = Temp::new();
    let cfg = Config {
        max_key_bytes: 4,
        max_value_bytes: 4,
        max_frame_bytes: 128,
        max_state_bytes: 54,
    };
    let db = Database::create(t.db(), cfg).unwrap();
    assert!(db.put(b"", b"v").is_err());
    assert!(db.put(b"12345", b"v").is_err());
    assert!(db.put(b"a", b"12345").is_err());
    for k in [b"a", b"b", b"c"] {
        db.put(k, b"v").unwrap();
    }
    assert!(db.put(b"d", b"v").is_err());
    assert_eq!(db.scan(Some(b"b"), Some(b"c")).unwrap().len(), 1);
    assert!(db.scan(Some(b"c"), Some(b"a")).is_err());
    assert!(db.scan(Some(b"b"), Some(b"b")).unwrap().is_empty());
    let s = db.statistics().unwrap();
    assert_eq!(s.live_keys, 3);
    assert_eq!(s.active_snapshots, 0);
    assert_eq!(s.last_sequence, 3);
    db.delete(b"a").unwrap();
    let s = db.statistics().unwrap();
    assert_eq!(s.tombstone_records_since_open_or_compact, 1);
    db.compact().unwrap();
    assert_eq!(db.statistics().unwrap().wal_bytes, 0);
}
#[test]
fn invalid_configuration_and_missing_path_fail_early() {
    let t = Temp::new();
    let bad = Config {
        max_frame_bytes: u32::MAX,
        ..Config::default()
    };
    assert!(matches!(
        Database::create(t.db(), bad),
        Err(Error::InvalidConfig(_))
    ));
    assert!(!t.db().exists());
    assert!(Database::open(t.db()).is_err());
    let db = Database::create(t.db(), Config::default()).unwrap();
    drop(db);
    assert!(Database::create(t.db(), Config::default()).is_err());
}
#[test]
fn snapshot_resource_cap_and_drop_release() {
    let t = Temp::new();
    let db = Database::create(t.db(), Config::default()).unwrap();
    let mut snapshots = Vec::new();
    for _ in 0..128 {
        snapshots.push(db.begin_read().unwrap());
    }
    assert!(matches!(db.begin_read(), Err(Error::ResourceLimit(_))));
    snapshots.pop();
    db.begin_write().unwrap().rollback();
}
#[test]
fn live_state_overflow_is_atomic_and_does_not_poison() {
    let t = Temp::new();
    let cfg = Config {
        max_key_bytes: 1,
        max_value_bytes: 1,
        max_frame_bytes: 100,
        max_state_bytes: 18,
    };
    let db = Database::create(t.db(), cfg).unwrap();
    db.put(b"a", b"v").unwrap();
    assert!(matches!(db.put(b"b", b"v"), Err(Error::ResourceLimit(_))));
    assert_eq!(db.get(b"a").unwrap(), Some(b"v".to_vec()));
    db.delete(b"a").unwrap();
    db.put(b"b", b"v").unwrap();
}
#[test]
fn stale_and_unrelated_files_lifecycle() {
    let t = Temp::new();
    let db = Database::create(t.db(), Config::default()).unwrap();
    drop(db);
    std::fs::write(t.db().join("segment-00000000000000000002.tmp"), b"partial").unwrap();
    std::fs::write(t.db().join("personal-notes"), b"keep").unwrap();
    let v = bastionkv::verify(t.db()).unwrap();
    assert_eq!(v.orphan_files.len(), 2);
    let db = Database::open(t.db()).unwrap();
    assert!(!t.db().join("segment-00000000000000000002.tmp").exists());
    assert!(t.db().join("personal-notes").exists());
    drop(db);
}
#[test]
fn oversized_staging_restores_previous_batch_entry() {
    let t = Temp::new();
    let cfg = Config {
        max_key_bytes: 1,
        max_value_bytes: 1,
        max_frame_bytes: 100,
        max_state_bytes: 18,
    };
    let db = Database::create(t.db(), cfg).unwrap();
    let mut tx = db.begin_write().unwrap();
    tx.put(b"a", b"1").unwrap();
    assert!(tx.put(b"b", b"2").is_err());
    assert_eq!(tx.get(b"a").unwrap(), Some(b"1".to_vec()));
    assert_eq!(tx.get(b"b").unwrap(), None);
    tx.commit().unwrap();
}
#[cfg(unix)]
#[test]
fn init_rejects_broken_lock_symlink_without_creating_target() {
    let t = Temp::new();
    std::fs::create_dir(t.db()).unwrap();
    let target = t.0.join("outside");
    std::os::unix::fs::symlink(&target, t.db().join("LOCK")).unwrap();
    assert!(Database::create(t.db(), Config::default()).is_err());
    assert!(!target.exists());
}
