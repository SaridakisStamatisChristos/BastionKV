mod common;
use bastionkv::{Config, Database};
use common::{Rng, Temp};
use std::collections::BTreeMap;

fn run_reference_history(seeds: u64, steps: usize) {
    for seed in 1..=seeds {
        let t = Temp::new();
        let mut db = Database::create(t.db(), Config::default()).unwrap();
        let mut oracle = BTreeMap::new();
        let mut rng = Rng(seed);
        let mut held = None;
        for i in 0..steps {
            let key = (rng.next() % 17).to_be_bytes().to_vec();
            let value = rng.next().to_be_bytes().to_vec();
            match rng.next() % 10 {
                0 | 1 => {
                    db.put(&key, &value).unwrap();
                    oracle.insert(key, value);
                }
                2 => {
                    db.delete(&key).unwrap();
                    oracle.remove(&key);
                }
                3 => {
                    let mut tx = db.begin_write().unwrap();
                    let mut next = oracle.clone();
                    for _ in 0..4 {
                        let k = (rng.next() % 17).to_be_bytes().to_vec();
                        if rng.next().is_multiple_of(3) {
                            tx.delete(&k).unwrap();
                            next.remove(&k);
                        } else {
                            let v = rng.next().to_be_bytes().to_vec();
                            tx.put(&k, &v).unwrap();
                            next.insert(k, v);
                        }
                    }
                    tx.commit().unwrap();
                    oracle = next;
                }
                4 => {
                    db.compact().unwrap();
                }
                5 => {
                    held = None;
                    drop(db);
                    db = Database::open(t.db()).unwrap();
                }
                6 => {
                    held = Some((db.begin_read().unwrap(), oracle.clone()));
                }
                7 => {
                    db.flush().unwrap();
                }
                8 => {
                    let mut tx = db.begin_write().unwrap();
                    tx.put(&key, &value).unwrap();
                    tx.rollback();
                }
                _ => {
                    assert_eq!(
                        db.get(&key).unwrap(),
                        oracle.get(&key).cloned(),
                        "seed={seed} step={i}"
                    );
                }
            }
            let expected: Vec<_> = oracle.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            assert_eq!(
                db.scan(None, None).unwrap(),
                expected,
                "seed={seed} step={i}"
            );
            if let Some((s, o)) = &held {
                assert_eq!(
                    s.scan(None, None).unwrap(),
                    o.iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect::<Vec<_>>()
                );
            }
        }
        drop(held);
        drop(db);
        assert_eq!(bastionkv::verify(t.db()).unwrap().live_keys, oracle.len());
    }
}

#[test]
fn random_histories_reference_oracle() {
    run_reference_history(32, 200);
}

#[test]
#[ignore = "opt-in deterministic long-run stress"]
fn stress_reference_oracle() {
    run_reference_history(256, 5000);
}
