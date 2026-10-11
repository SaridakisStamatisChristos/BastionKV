#![forbid(unsafe_code)]
use bastionkv::{Database, Error, Result};
use std::{
    env,
    io::{self, Write},
};
fn run() -> Result<()> {
    let a: Vec<_> = env::args().collect();
    if a.len() != 3 {
        return Err(Error::InvalidInput(
            "worker <path> <batch|compact|lock|io-batch|io-compact>".into(),
        ));
    }
    let db = Database::open(&a[1])?;
    match a[2].as_str() {
        "batch" | "io-batch" => {
            let mut t = db.begin_write()?;
            t.put(b"left", b"new")?;
            t.put(b"right", b"new")?;
            t.delete(b"dead")?;
            let r = t.commit();
            if a[2] == "io-batch" {
                assert!(r.is_err());
                assert!(matches!(db.put(b"later", b"x"), Err(Error::Poisoned)));
            } else {
                r?;
            }
        }
        "compact" | "io-compact" => {
            let r = db.compact();
            if a[2] == "io-compact" {
                assert!(r.is_err());
                assert!(matches!(db.flush(), Err(Error::Poisoned)));
            } else {
                r?;
            }
        }
        "lock" => {
            println!("locked");
            io::stdout().flush()?;
            let mut line = String::new();
            io::stdin().read_line(&mut line)?;
        }
        _ => return Err(Error::InvalidInput("unknown worker mode".into())),
    }
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
