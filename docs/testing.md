# Testing and verification

BastionKV uses layered verification rather than a single unit-test gate.

## Standard verification

`bash scripts/check.sh` executes:

1. `cargo fmt --check`
2. Clippy on all targets/features with warnings denied
3. default test suite
4. all-feature test suite
5. rustdoc with warnings denied
6. release and debug builds
7. Linux OS-level fault harness
8. Cargo package verification

The GitHub Actions workflow executes the same repository gate on Rust 1.89.0 and the current stable toolchain.

## Behavioral suites

- `tests/semantics.rs` — API, persistence, snapshots, compaction, statistics, configuration.
- `tests/concurrency.rs` — threaded reader/writer/compaction behavior and conflict ordering.
- `tests/crash.rs` — subprocess crash points for commits, compaction, and process locks.
- `tests/corruption.rs` — truncation, checksum-valid structural damage, parser bounds, symlink/path handling, and mutation-free verification.
- `tests/reference.rs` — deterministic histories checked against an independent `BTreeMap` oracle.
- `tests/cli.rs` — command behavior, exit status, and read-only integrity inspection.

The ignored `stress_reference_oracle` target expands the deterministic reference workload to 256 seeds × 5000 steps.

## Integrity evidence

`python3 scripts/evidence.py` recomputes SHA-256 for authored repository files and compares them with `evidence/files.sha256`. CI runs this before compilation so stale or silently modified evidence fails the workflow.

Raw benchmark samples and prior CI logs under `evidence/` are retained as historical observations. They are not substituted for a green workflow on the current pull-request revision.

## Claim boundaries

Passing these tests demonstrates the exercised process-crash, parser, locking, and local-filesystem behaviors. It does not establish physical power-loss certification, non-Linux durability, hostile directory-owner safety, or performance superiority.
