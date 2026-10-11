mod common;
use bastionkv::{Config, Database, Error};
use common::Temp;
use std::sync::{Arc, Barrier};
#[test]
fn readers_writer_and_compactor() {
    let t = Temp::new();
    let db = Database::create(t.db(), Config::default()).unwrap();
    let mut tx = db.begin_write().unwrap();
    tx.put(b"a", &0u64.to_le_bytes()).unwrap();
    tx.put(b"b", &0u64.to_le_bytes()).unwrap();
    tx.commit().unwrap();
    let mut threads = Vec::new();
    for _ in 0..4 {
        let d = db.clone();
        threads.push(std::thread::spawn(move || {
            for _ in 0..300 {
                let s = d.begin_read().unwrap();
                let a = s.get(b"a").unwrap().unwrap();
                assert_eq!(s.get(b"b").unwrap(), Some(a.clone()));
                assert_eq!(s.scan(None, None).unwrap()[0].1, a);
            }
        }));
    }
    let d = db.clone();
    threads.push(std::thread::spawn(move || {
        for n in 1u64..100 {
            let mut tx = d.begin_write().unwrap();
            tx.put(b"a", &n.to_le_bytes()).unwrap();
            tx.put(b"b", &n.to_le_bytes()).unwrap();
            tx.commit().unwrap();
        }
    }));
    let d = db.clone();
    threads.push(std::thread::spawn(move || {
        for _ in 0..20 {
            d.compact().unwrap();
        }
    }));
    for j in threads {
        j.join().unwrap();
    }
    assert_eq!(db.get(b"a").unwrap(), Some(99u64.to_le_bytes().to_vec()));
}
#[test]
fn concurrent_writers_one_serialization_winner() {
    let t = Temp::new();
    let db = Database::create(t.db(), Config::default()).unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let mut handles = Vec::new();
    for key in [b"a", b"b"] {
        let d = db.clone();
        let b = barrier.clone();
        handles.push(std::thread::spawn(move || {
            let mut tx = d.begin_write().unwrap();
            tx.put(key, b"value").unwrap();
            b.wait();
            tx.commit()
        }));
    }
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|r| matches!(r, Err(Error::Conflict { .. })))
            .count(),
        1
    );
}
#[test]
fn exhaustive_serialized_snapshot_checkpoint_orders() {
    // Enumerate 12 valid schedules from 24 permutations of publication/read registration/compaction/read use.
    // Model oracle knows only whether registration preceded publication. Filesystem is exercised.
    for a in 0..4 {
        for b in 0..4 {
            for c in 0..4 {
                for d in 0..4 {
                    let order = [a, b, c, d];
                    if (0..4).any(|n| order.iter().filter(|x| **x == n).count() != 1) {
                        continue;
                    }
                    if order.iter().position(|x| *x == 1) > order.iter().position(|x| *x == 3) {
                        continue;
                    }
                    let t = Temp::new();
                    let db = Database::create(t.db(), Config::default()).unwrap();
                    db.put(b"k", b"old").unwrap();
                    let mut snapshot = None;
                    let mut current = b"old".to_vec();
                    let mut expected = Vec::new();
                    for op in order {
                        match op {
                            0 => {
                                db.put(b"k", b"new").unwrap();
                                current = b"new".to_vec();
                            }
                            1 => {
                                snapshot = Some(db.begin_read().unwrap());
                                expected = current.clone();
                            }
                            2 => db.compact().unwrap(),
                            3 => {
                                assert_eq!(
                                    snapshot.as_ref().unwrap().get(b"k").unwrap(),
                                    Some(expected.clone())
                                );
                            }
                            _ => unreachable!(),
                        }
                    }
                }
            }
        }
    }
}
