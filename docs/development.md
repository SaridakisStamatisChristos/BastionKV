# Development

## Prerequisites

- Linux
- Rust 1.89.0 or newer
- Python 3

BastionKV currently has no third-party Rust crate dependencies.

## Local workflow

Use the repository verification script as the default pre-commit gate:

```sh
bash scripts/check.sh
```

For a quick executable smoke test:

```sh
cargo run --example transactions -- /tmp/bastion-example
```

For benchmark calibration:

```sh
python3 scripts/bench.py --n 1000 --runs 3
```

For the opt-in deterministic long-run reference test:

```sh
cargo test --locked --all-features --test reference stress_reference_oracle -- --ignored
```

## Safety-sensitive changes

Storage, recovery, compaction, locking, corruption handling, and transaction-publication changes must be reviewed against [`correctness-contract.md`](correctness-contract.md), the durability model, recovery model, and threat model. Do not weaken sync points, parser bounds, lock semantics, or corruption refusal merely to improve throughput or make a test pass.

Fault hooks are compiled only with the `fault-injection` feature and are intended for verification builds. They must not be enabled in production artifacts.

## Documentation and evidence

Update product documentation when externally visible behavior or supported boundaries change. If an authored repository file changes, regenerate the integrity list:

```sh
python3 scripts/evidence.py --write
python3 scripts/evidence.py
```

The generated `evidence/files.sha256` intentionally excludes itself, `.git`, `target`, Python bytecode caches, and ephemeral `evidence/ci` output.
