use bastionkv::{Config, Database, Result};
use std::{env, fs, time::Instant};
fn samples(name: &str, n: usize, ops: usize, mut f: impl FnMut(usize) -> Result<()>) -> Result<()> {
    for i in 0..n {
        let t = Instant::now();
        f(i)?;
        println!("{name},{i},{ops},{}", t.elapsed().as_nanos());
    }
    Ok(())
}
fn run() -> Result<()> {
    let n: usize = env::var("BASTIONKV_BENCH_N")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1000);
    if n == 0 || n > 100_000 {
        return Err(bastionkv::Error::InvalidInput(
            "benchmark n must be 1..100000".into(),
        ));
    }
    let p = env::temp_dir().join(format!("bastionkv-bench-{}", std::process::id()));
    fs::create_dir(&p)?;
    let mut db = Database::create(p.join("db"), Config::default())?;
    let value = vec![42; 128];
    let key = |i: usize| (i as u64).to_be_bytes();
    println!("workload,sample,logical_operations,latency_ns");
    samples("sequential_write", n, 1, |i| {
        db.put(&key(i), &value).map(|_| ())
    })?;
    samples("random_write", n, 1, |i| {
        db.put(&key((i.wrapping_mul(7919) + 17) % n), &value)
            .map(|_| ())
    })?;
    samples("random_read", n, 1, |i| {
        assert_eq!(
            db.get(&key((i.wrapping_mul(7919) + 17) % n))?,
            Some(value.clone())
        );
        Ok(())
    })?;
    samples("read_heavy_90", n, 1, |i| {
        if i % 10 == 0 {
            db.put(&key(i % n), &value)?;
        } else {
            db.get(&key(i % n))?;
        }
        Ok(())
    })?;
    samples("write_heavy_90", n, 1, |i| {
        if i % 10 != 0 {
            db.put(&key(i % n), &value)?;
        } else {
            db.get(&key(i % n))?;
        }
        Ok(())
    })?;
    samples("range_scan", 100.min(n), 1, |i| {
        db.scan(Some(&key(i % n)), Some(&key((i % n + 32).min(n))))?;
        Ok(())
    })?;
    for batch in [1, 8, 32, 128] {
        samples(&format!("batch_{batch}"), (n / batch).max(5), batch, |i| {
            let mut tx = db.begin_write()?;
            for j in 0..batch {
                tx.put(&key((i * batch + j) % n), &value)?;
            }
            tx.commit()?;
            Ok(())
        })?;
    }
    let before = db.statistics()?;
    println!("#metrics,before_compact_bytes,{}", before.total_data_bytes);
    println!("#metrics,wal_bytes_written,{}", before.wal_bytes);
    samples("compaction", 5, 1, |_| db.compact())?;
    let after = db.statistics()?;
    println!("#metrics,after_compact_bytes,{}", after.total_data_bytes);
    println!(
        "#metrics,checkpoint_bytes_written,{}",
        5 * (after.total_data_bytes - 52)
    );
    println!("#metrics,live_payload_bytes,{}", n * 136);
    drop(db);
    samples("reopen", 10, 1, |_| {
        let x = Database::open(p.join("db"))?;
        assert_eq!(x.statistics()?.live_keys, n);
        Ok(())
    })?;
    db = Database::open(p.join("db"))?;
    for i in 0..n {
        db.put(&key(i), &value)?;
    }
    drop(db);
    samples("wal_recovery", 10, 1, |_| {
        let x = Database::open(p.join("db"))?;
        assert_eq!(x.statistics()?.recovered_transactions, n as u64);
        Ok(())
    })?;
    fs::remove_dir_all(p)?;
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
