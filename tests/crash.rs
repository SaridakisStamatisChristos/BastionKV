#![cfg(feature = "fault-injection")]
mod common;
use bastionkv::{Config, Database, Error};
use common::Temp;
use std::{
    io::{BufRead, BufReader, Write},
    process::{Command, Stdio},
};
// Serialize subprocess fixtures: fork temporarily inherits other threads' open locks
// until exec. Writer/reader concurrency is tested independently without process spawning.
static PROCESS_FIXTURE: std::sync::Mutex<()> = std::sync::Mutex::new(());
fn setup() -> Temp {
    let t = Temp::new();
    let db = Database::create(t.db(), Config::default()).unwrap();
    let mut tx = db.begin_write().unwrap();
    tx.put(b"left", b"old").unwrap();
    tx.put(b"right", b"old").unwrap();
    tx.put(b"dead", b"value").unwrap();
    tx.commit().unwrap();
    drop(db);
    t
}
fn worker(t: &Temp, mode: &str, env: &str, point: &str) -> std::process::ExitStatus {
    Command::new(env!("CARGO_BIN_EXE_bastionkv-fault-worker"))
        .arg(t.db())
        .arg(mode)
        .env(env, point)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap()
}
fn assert_abort(status: std::process::ExitStatus, point: &str) {
    assert!(!status.success(), "point not reached {point}");
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(
            status.signal(),
            Some(6),
            "expected SIGABRT at {point}, got {status}"
        );
    }
}
fn atomic(t: &Temp, must_new: bool, must_old: bool) {
    bastionkv::verify(t.db()).unwrap();
    let db = Database::open(t.db()).unwrap();
    let l = db.get(b"left").unwrap().unwrap();
    let r = db.get(b"right").unwrap().unwrap();
    assert_eq!(l, r);
    if must_new {
        assert_eq!(l, b"new");
    }
    if must_old {
        assert_eq!(l, b"old");
    }
    assert_eq!(db.get(b"dead").unwrap().is_some(), l == b"old");
    drop(db);
    bastionkv::verify(t.db()).unwrap();
}
#[test]
fn commit_crash_boundary_matrix() {
    let _fixture = PROCESS_FIXTURE.lock().unwrap();
    for point in [
        "before_wal_append",
        "wal_partial",
        "after_wal_write",
        "after_wal_sync",
        "before_publish",
        "after_publish",
    ] {
        let t = setup();
        assert_abort(worker(&t, "batch", "BASTIONKV_CRASH", point), point);
        atomic(
            &t,
            matches!(point, "after_wal_sync" | "before_publish" | "after_publish"),
            matches!(point, "before_wal_append" | "wal_partial"),
        );
    }
}
#[test]
fn compaction_crash_boundary_matrix_no_resurrection() {
    let _fixture = PROCESS_FIXTURE.lock().unwrap();
    for point in [
        "segment_partial",
        "after_segment_sync",
        "after_segment_rename",
        "after_new_wal_sync",
        "manifest_partial",
        "before_manifest_replace",
        "after_manifest_replace",
        "after_manifest_sync",
        "before_cleanup",
        "during_cleanup",
    ] {
        let t = setup();
        let db = Database::open(t.db()).unwrap();
        db.delete(b"dead").unwrap();
        db.put(b"left", b"latest").unwrap();
        db.put(b"right", b"latest").unwrap();
        drop(db);
        assert_abort(worker(&t, "compact", "BASTIONKV_CRASH", point), point);
        for _ in 0..2 {
            bastionkv::verify(t.db()).unwrap();
            let db = Database::open(t.db()).unwrap();
            assert_eq!(db.get(b"dead").unwrap(), None);
            assert_eq!(db.get(b"left").unwrap(), Some(b"latest".to_vec()));
            assert_eq!(db.get(b"right").unwrap(), Some(b"latest".to_vec()));
            drop(db);
        }
    }
}
#[test]
fn injected_io_failures_poison_handle_and_recover_atomically() {
    let _fixture = PROCESS_FIXTURE.lock().unwrap();
    for point in [
        "before_wal_append",
        "wal_partial",
        "after_wal_write",
        "after_wal_sync",
        "before_publish",
        "after_publish",
    ] {
        let t = setup();
        assert!(worker(&t, "io-batch", "BASTIONKV_IO_FAIL", point).success());
        atomic(
            &t,
            matches!(point, "after_wal_sync" | "before_publish" | "after_publish"),
            matches!(point, "before_wal_append" | "wal_partial"),
        );
    }
    for point in [
        "segment_partial",
        "after_segment_sync",
        "after_new_wal_sync",
        "before_manifest_replace",
        "after_manifest_replace",
        "during_cleanup",
    ] {
        let t = setup();
        assert!(worker(&t, "io-compact", "BASTIONKV_IO_FAIL", point).success());
        atomic(&t, false, true);
    }
}
#[test]
fn cross_process_lock_release_after_forced_termination() {
    let _fixture = PROCESS_FIXTURE.lock().unwrap();
    let t = setup();
    let mut child = Command::new(env!("CARGO_BIN_EXE_bastionkv-fault-worker"))
        .arg(t.db())
        .arg("lock")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    assert_eq!(line.trim(), "locked");
    assert!(matches!(Database::open(t.db()), Err(Error::Locked)));
    assert!(matches!(bastionkv::verify(t.db()), Err(Error::Locked)));
    child.kill().unwrap();
    child.wait().unwrap();
    let db = Database::open(t.db()).unwrap();
    drop(db);
    let mut child = Command::new(env!("CARGO_BIN_EXE_bastionkv-fault-worker"))
        .arg(t.db())
        .arg("lock")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    child.stdin.take().unwrap().write_all(b"exit\n").unwrap();
    assert!(child.wait().unwrap().success());
    Database::open(t.db()).unwrap();
}
