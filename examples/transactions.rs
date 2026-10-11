use bastionkv::{Config, Database, Result};
fn main() -> Result<()> {
    let path = std::env::args().nth(1).ok_or_else(|| {
        bastionkv::Error::InvalidInput("cargo run --example transactions -- <new-path>".into())
    })?;
    let db = Database::create(path, Config::default())?;
    let mut tx = db.begin_write()?;
    tx.put(b"left", b"old")?;
    tx.put(b"right", b"old")?;
    tx.commit()?;
    let snapshot = db.begin_read()?;
    let mut tx = db.begin_write()?;
    tx.put(b"left", b"new")?;
    tx.put(b"right", b"new")?;
    tx.commit()?;
    assert_eq!(snapshot.get(b"left")?, Some(b"old".to_vec()));
    db.delete(b"right")?;
    db.compact()?;
    println!(
        "snapshot={:?} current={:?}",
        snapshot.scan(None, None)?,
        db.scan(None, None)?
    );
    Ok(())
}
