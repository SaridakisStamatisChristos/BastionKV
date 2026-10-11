# BastionKV

A small Rust embedded key-value engine with synchronous commits, immutable MVCC snapshots, atomic batches, explicit compaction, and checksummed recovery. Linux local filesystems are the declared durability target. This implementation prioritizes inspectable protocols over throughput or large databases. See [limitations](docs/limitations.md) before use.

## Build and verify

Rust 1.89+, Python 3, Linux. Zero third-party Rust dependencies. No service, credentials, or network required after installing Rust.

```sh
bash scripts/check.sh
cargo run --example transactions -- /tmp/bastion-example
python3 scripts/bench.py --n 1000 --runs 3
cargo test --all-features --test reference stress_reference_oracle -- --ignored
```

The example requires a new path. `check.sh` exercises formatting, Clippy, default/fault builds, tests, docs, release build, real Linux file-size-limit I/O failure, and a verified Cargo package. Fault hooks are compiled only with the explicit `fault-injection` feature; don't deploy that testing build.

## Library

```rust,no_run
use bastionkv::{Config, Database, Result};
fn main() -> Result<()> {
    let db = Database::create("example.db", Config::default())?;
    let mut tx = db.begin_write()?;
    tx.put(b"a", b"one")?;
    tx.put(b"b", b"two")?;
    tx.commit()?;
    let old = db.begin_read()?;
    db.put(b"a", b"new")?;
    assert_eq!(old.get(b"a")?, Some(b"one".to_vec()));
    db.compact()?;
    assert_eq!(old.scan(None, None)?.len(), 2);
    Ok(())
}
```

`Database::open(path)` requires existing valid metadata. `get`, `put`, `delete`, `contains_key`, `scan`, `begin_read`, `begin_write`, `flush`, `compact`, `statistics`, and typed `config` are public. Keys/values are bytes; keys are nonempty, values may be empty. Scans use inclusive lower / exclusive upper bounds in byte order. Write `get` includes staged changes. Dropping or rolling back a write transaction discards it. Empty commits validate their base but don't allocate a new sequence. Readers and writers hold process-local snapshots; at most 128 are registered per handle.

Any intervening commit aborts an optimistic write transaction, including disjoint-key commits. Retry by starting a new transaction. Successful nonempty commits advance one global sequence. Acknowledgment follows file sync and atomic publication under one mutex. An interrupted or I/O-failed commit has a documented unknown **atomic** outcome; reopen before retrying. No transaction-level idempotency tokens are offered.

## CLI

```sh
cargo build --release
./target/release/bastionkv init /tmp/bastion-cli
./target/release/bastionkv put /tmp/bastion-cli hello world
./target/release/bastionkv get /tmp/bastion-cli hello
./target/release/bastionkv scan /tmp/bastion-cli he
./target/release/bastionkv put /tmp/bastion-cli 00ff 0102 --hex
./target/release/bastionkv stats /tmp/bastion-cli
./target/release/bastionkv dump-wal /tmp/bastion-cli
./target/release/bastionkv verify /tmp/bastion-cli
./target/release/bastionkv compact /tmp/bastion-cli
./target/release/bastionkv delete /tmp/bastion-cli hello
```

Input is UTF-8 command-line bytes unless `--hex` is used; output keys/values are always hexadecimal. `scan` accepts an optional prefix. `dump-segment <file>` inspects a segment directly. Inspection reports frame offsets, sequence, count, record type, and lengths. Corruption and missing keys return a nonzero status. `verify` and `dump-wal` acquire an existing shared process lock and never repair or modify files. Close the writer first. Recoverable incomplete WAL tails and orphan files are reported explicitly; only open performs cleanup.

## Architecture and evidence

Application → optimistic batch → checksummed WAL frame → fsync → immutable-map publication. Explicit compaction writes a single live-state segment, rotates the WAL, and publishes its generation through a checksummed atomic manifest replacement. Active snapshots retain their old maps, so old disk files can be reclaimed safely.

Start with [architecture](docs/architecture.md), [format](docs/storage-format.md), [durability](docs/durability-model.md), [transactions](docs/transaction-model.md), [recovery](docs/recovery.md), [compaction](docs/compaction.md), [threat model](docs/threat-model.md), and [benchmark methodology](docs/benchmark-methodology.md).

[evidence/manifest.json](evidence/manifest.json) is the authoritative claim/requirement ledger. Evidence distinguishes observed local gates from remote CI and platform/power-loss gaps. Source existence isn't a production-readiness proof. Proprietary license; publication grants no use rights.
