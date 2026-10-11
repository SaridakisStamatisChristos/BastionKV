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
