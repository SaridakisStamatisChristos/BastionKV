# Adversarial campaign and repair ledger

## Coverage

| Attack | Independent evidence path | Outcome / scope |
|---|---|---|
| Partial WAL, torn tail, acknowledgment boundaries | crash subprocess matrices + every-byte final-frame cuts | Atomic old/new state; durable-point commits retained |
| Corrupt middle/last complete WAL | flip every byte, structural mutations with recomputed checksums | Open+offline verifier refuse |
| Crash after WAL sync before publication | after_wal_sync / before_publish abort | Reopen restores complete batch |
| Compaction replacement/rotation/cleanup | ten subprocess crash points, two reopen cycles each | Same live state, no delete resurrection |
| Stale temp and unrelated files | inspect before/after exclusive open | Orphans reported; only format-owned temp deleted |
| Snapshot/compaction race | immutable snapshot across delete/rewrite/compact, real threads | Stable point/scan observations |
| Overflow/giant length/metadata | forged checksum-valid u32::MAX header/record/count/config | Refusal before oversized allocation |
| Duplicate/reused sequences | append complete duplicated frame; re-seal rolled-back sequence | Fail closed |
| Invalid paths/permission failures | missing/nonregular/symlink fixtures; Linux chmod fixture | Invalid paths refused; non-root permission lane in CI |
| Short writes / I/O uncertainty | real RLIMIT_FSIZE failed append + 12 injected error points | Atomic recovery; poisoned handle prevents reuse |
| Two processes / stale process | lock worker + independent open/verify + SIGKILL | Second writer refused, lock released after exit |
| Reference histories | independent BTreeMap with puts/deletes/batches/rollback/snapshots/flush/compact/reopen | Equality after every step; deterministic seeds |
| Packaging / natural invocation | check.sh package verification, CLI integration, example execution | Compiler/package and real commands tested |

Full details and logs are mapped in manifest.json. OS power-loss behavior, hostile directory-owner races and std Mutex internals are not modeled by these tests. Tests assert actual crash hooks are reached (non-success exit), not merely that a worker runs.

## Repairs

F01 — Initial parallel subprocess crash fixtures observed intermittent `Locked` at verifier/open even after their own worker had exited. Earliest invalidated phase GENERATE (test orchestration). Diagnosis: parallel POSIX process spawning can transiently inherit another test thread's open flock descriptor between fork and exec. A serial run passed all four crash suites; the two unrelated-fork lock windows don't model the intended crash invariant. Repair: one fixture mutex around subprocess campaigns, with explicit fork lifetime documentation. Real threaded writer/reader/compactor tests remain parallel; process lock test still proves distinct writable opens are refused and killed holders release their own locks. Dependent default/fault suites and crash matrices rerun. No engine atomicity or lock gate relaxed.

F02 — Review identified ordinary stats recalculating logical state by scanning all keys. Earliest invalidated phase GENERATE. Repair: cache logical byte count at immutable-map publication/load, use that value in statistics. Range/memory/reference/concurrency gates rerun. This preserves semantics and removes unnecessary stats cost.

F03 — Initial integration attempted all-target check before the declared benchmark source batch existed. Earliest phase INTEGRATE; manifest target resolution correctly refused the incomplete batch. Repair: generate complete workload target; compiler/Clippy and benchmark build rerun. No placeholder target retained.

Rejected shortcuts: relaxing sync, ignoring complete corrupt frames, relying on Drop for durability, merging generations without a durable manifest boundary, and inferring power-loss success from process aborts. No known accepted critical defect is being masked as a TODO.

F04 — Actual initial stable CI failed on Rust 1.99.0's deprecation of AtomicUsize::fetch_update (log preserved at ci-initial-stable-failure.log, run 38175545724). Earliest invalidated phase GENERATE (API portability). Replacing it with a newer try_update would violate Rust 1.89 MSRV. Repair instead uses the existing serialization mutex for the cap check and a non-deprecated fetch_add: all increments occur under the mutex, drop only decrements, so a concurrent drop can only make the cap decision conservative. No lint suppression or MSRV weakening. Reverify both toolchains, snapshot limits, threaded readers, full reference stress, and benchmark affected registration paths.

F05 — Adversarial review found `try_exists` followed a broken LOCK symlink during init, potentially creating its target before the regular-file guard. Earliest invalidated phase GENERATE (path validation). Repair: lstat-style symlink_metadata presence check before regular-file validation. Added test proves broken LOCK symlink is refused and external target isn't created. Directory-owner TOCTOU attacks remain outside the declared trust model; all path/corruption/CLI gates rerun.

F04 re-verification also exposed new stable Clippy recommendations for integer is_multiple_of in CLI/test/benchmark expressions. The standard method is supported by the declared MSRV; adopting it passed both Rust 1.89 and 1.99 Clippy without suppressed lints. No behavior or benchmark semantics changed.

F06 — Review found offline shared locking opened LOCK with write permissions even though verification never wrote it. Earliest invalidated phase GENERATE (inspection access contract). Repair uses a read-only regular-file handle for shared locking, while writable opens retain read/write descriptors. The OS permission harness now verifies a directory with mode 0500 and files 0400 and proves a writer cannot open it on non-root CI. The root local environment cannot independently demonstrate permission denial, so remote non-root execution is required. Process/shared lock contention and all CLI/integrity tests rerun.
