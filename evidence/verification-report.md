# Verification report

## Current implementation

BastionKV provides a Rust library and CLI with binary-safe configuration/errors, a checksummed framed WAL, immutable full-map MVCC snapshots, atomic optimistic write transactions, process locking, offline verification and dumps, crash recovery, generation checkpoint/compaction, and storage statistics. The Rust implementation has no external crate dependencies and uses no unsafe code.

Successful commits are acknowledged only after WAL synchronization and atomic immutable-map publication. Failed or interrupted commits can have an unknown atomic outcome and must be resolved by reopening. Compaction publishes a synchronized generation pair through atomic manifest replacement before old-generation cleanup.

## Executed assurance evidence

The retained verification evidence records 33 regular tests plus one compiling doctest, the opt-in 256-seed × 5000-step deterministic reference stress (1,280,000 steps), and a bounded 32-seed × 200-step reference workload. It also covers six commit crash points, ten compaction crash points, twelve injected I/O error scenarios, every-byte WAL/segment cuts and flips, checksum-valid structural corruption, sequence/generation exhaustion, symlink/path validation, concurrency/conflicts, stale temporary-file handling, non-mutating verification, and a real Linux file-size-limit short-write path.

The retained CI evidence covers Rust 1.89.0 and stable toolchains, compiler/Clippy/package gates, example and CLI execution, integrity-hash validation, and non-root permission behavior. Current pull requests must still pass the live CI workflow; historical logs are evidence, not a substitute for current verification.

## Benchmark evidence

Three 1000-key benchmark runs preserve raw CSV samples, environment/source checksums, and median/p95/p99/throughput summaries. The workloads include synchronous sequential and permuted writes, reads, mixed workloads, range scans, several atomic batch sizes, compaction, reopen, and WAL recovery. SQLite is used only as a calibration baseline for comparable synchronous single-key sequential writes.

No performance-superiority claim is made.

## Known limits

The database is bounded in RAM and clones the full map per published version. The default logical-state budget is 32 MiB with a 128-snapshot cap. Writers conflict globally, commits are synchronous, compaction is explicit and blocking, and snapshots are process-local. There is no persistent snapshot API, transaction idempotency token, physical power-cut certification, hostile-directory-owner sandbox, automatic corruption salvage, or non-Linux durability claim.

For current requirement-to-evidence mapping, read `manifest.json`; for discovered failure modes and their repairs, read `adversarial-validation.md`; for file integrity, read `files.sha256`.

**Status:** verified for the declared local Linux target within the documented filesystem assumptions. Production certification, SOTA, performance superiority, and cross-platform durability remain unverified.
