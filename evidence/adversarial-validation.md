# Adversarial validation and repair record

This document preserves failures that materially shaped BastionKV's correctness boundaries. It is retained as engineering evidence, not as a development timeline.

## Coverage

| Attack | Independent evidence path | Outcome / scope |
|---|---|---|
| Partial WAL, torn tail, acknowledgment boundaries | crash subprocess matrices + every-byte final-frame cuts | Atomic old/new state; durable-point commits retained |
| Corrupt middle/last complete WAL | flip every byte, structural mutations with recomputed checksums | Open and offline verifier refuse |
| Crash after WAL sync before publication | `after_wal_sync` / `before_publish` abort | Reopen restores complete batch |
| Compaction replacement/rotation/cleanup | ten subprocess crash points, two reopen cycles each | Same live state, no delete resurrection |
| Stale temp and unrelated files | inspect before/after exclusive open | Orphans reported; only format-owned temp deleted |
| Snapshot/compaction race | immutable snapshot across delete/rewrite/compact, real threads | Stable point/scan observations |
| Overflow/giant length/metadata | forged checksum-valid `u32::MAX` header/record/count/config | Refusal before oversized allocation |
| Duplicate/reused sequences | append complete duplicated frame; re-seal rolled-back sequence | Fail closed |
| Invalid paths/permission failures | missing/nonregular/symlink fixtures; Linux chmod fixture | Invalid paths refused; non-root permission lane in CI |
| Short writes / I/O uncertainty | real `RLIMIT_FSIZE` failed append + 12 injected error points | Atomic recovery; poisoned handle prevents reuse |
| Two processes / stale process | lock worker + independent open/verify + SIGKILL | Second writer refused, lock released after exit |
| Reference histories | independent `BTreeMap` with puts/deletes/batches/rollback/snapshots/flush/compact/reopen | Equality after every step; deterministic seeds |
| Packaging / natural invocation | `check.sh` package verification, CLI integration, example execution | Compiler/package and real commands tested |

The tests do not model physical power loss, hostile directory-owner races, or the internals of `std::sync::Mutex`. Crash tests assert that the configured crash hook is actually reached rather than accepting any worker failure.

## Findings and repairs

**F01 — transient inherited file locks in parallel subprocess fixtures.** Parallel POSIX process spawning could temporarily inherit another test thread's open `flock` descriptor between fork and exec, causing intermittent `Locked` results unrelated to the intended database crash invariant. The repair serializes subprocess crash fixtures with one fixture mutex and documents fork lifetime. Real threaded reader/writer/compactor tests remain parallel, and the independent process-lock test still proves refusal and lock release after holder termination.

**F02 — statistics recalculated logical state by scanning all keys.** Logical byte count is now cached at immutable-map publication/load and used by statistics. Semantics and reference/concurrency checks were rerun; behavior was preserved while removing unnecessary work.

**F03 — an all-target verification run referenced the benchmark target before that target existed.** The complete workload target was added before compiler/Clippy and benchmark verification. No placeholder target remains.

**F04 — stable Rust deprecated the snapshot-cap `AtomicUsize::fetch_update` path.** Replacing it with a newer method would have broken the Rust 1.89 MSRV. The repair performs the cap check under the existing serialization mutex and uses `fetch_add`; drops only decrement, so a concurrent drop can only make a cap decision conservative. Both toolchains and snapshot/concurrency/reference paths were reverified. Related stable Clippy recommendations were adopted without suppressing lints or changing behavior.

**F05 — broken `LOCK` symlink handling during initialization.** A `try_exists` check could follow a broken lock symlink and create its target before the regular-file guard. The repair uses `symlink_metadata` presence checks before regular-file validation. A regression test proves the external target is not created. Directory-owner TOCTOU attacks remain outside the declared trust model.

**F06 — offline shared locking requested write access.** Read-only verification never writes the lock file, so shared locking now opens it read-only while writable database opens retain read/write descriptors. The OS permission harness covers read-only directories/files and remote non-root CI verifies writer denial.

**F07 — benchmark CSV line endings changed during evidence transport.** A byte-for-byte remote comparison found three SQLite CSVs whose CRLF data had been normalized by text-mode transport. The raw files were restored byte-for-byte and the integrity manifest was regenerated. No benchmark measurements or expected checksums were changed to conceal the mismatch.

## Rejected shortcuts

The implementation deliberately does not relax synchronization, ignore complete corrupt frames, rely on `Drop` for durability, merge generations without a durable manifest boundary, or infer power-loss durability from process-crash tests.
