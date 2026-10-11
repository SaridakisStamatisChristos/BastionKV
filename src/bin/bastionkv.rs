#![forbid(unsafe_code)]
use bastionkv::{Config, Database, Error, Result};
use std::{
    env,
    io::{self, Write},
    path::Path,
};
fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}
fn data(s: &str, binary: bool) -> Result<Vec<u8>> {
    if !binary {
        return Ok(s.as_bytes().to_vec());
    }
    if !s.len().is_multiple_of(2) || !s.is_ascii() {
        return Err(Error::InvalidInput("hex requires even ASCII digits".into()));
    }
    (0..s.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&s[i..i + 2], 16)
                .map_err(|_| Error::InvalidInput("invalid hex".into()))
        })
        .collect()
}
fn run() -> Result<()> {
    let mut a: Vec<String> = env::args().skip(1).collect();
    let binary = a.iter().any(|s| s == "--hex");
    a.retain(|s| s != "--hex");
    if a.len() < 2 {
        return Err(Error::InvalidInput("usage: bastionkv <init|put|get|delete|scan|stats|verify|compact|dump-wal|dump-segment> <path> [key/value/prefix] [--hex]".into()));
    }
    let cmd = &a[0];
    let path = Path::new(&a[1]);
    let mut out = io::stdout().lock();
    match (cmd.as_str(), a.len()) {
        ("init", 2) => {
            Database::create(path, Config::default())?;
            writeln!(out, "initialized")?;
        }
        ("verify", 2) => {
            writeln!(out, "{:?}", bastionkv::verify(path)?)?;
        }
        ("dump-wal", 2) => bastionkv::dump_wal(path, &mut out)?,
        ("dump-segment", 2) => bastionkv::dump_file(path, true, &mut out)?,
        ("put", 4) => {
            let db = Database::open(path)?;
            writeln!(
                out,
                "{}",
                db.put(&data(&a[2], binary)?, &data(&a[3], binary)?)?
            )?;
        }
        ("get", 3) => {
            let db = Database::open(path)?;
            let key = data(&a[2], binary)?;
            match db.get(&key)? {
                Some(v) => writeln!(out, "{}", hex(&v))?,
                None => return Err(Error::InvalidInput("key not found".into())),
            }
        }
        ("delete", 3) => {
            let db = Database::open(path)?;
            writeln!(out, "{}", db.delete(&data(&a[2], binary)?)?)?;
        }
        ("scan", 2 | 3) => {
            let db = Database::open(path)?;
            let prefix = if a.len() == 3 {
                data(&a[2], binary)?
            } else {
                Vec::new()
            };
            for (k, v) in db.scan(Some(&prefix), None)? {
                if k.starts_with(&prefix) {
                    writeln!(out, "{}\t{}", hex(&k), hex(&v))?;
                }
            }
        }
        ("stats", 2) => {
            writeln!(out, "{:?}", Database::open(path)?.statistics()?)?;
        }
        ("compact", 2) => {
            Database::open(path)?.compact()?;
            writeln!(out, "compacted")?;
        }
        _ => {
            return Err(Error::InvalidInput(
                "unknown command or wrong argument count".into(),
            ))
        }
    }
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
