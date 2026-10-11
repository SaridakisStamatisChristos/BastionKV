# BastionKV R7 campaign report

## Result and architecture

Implemented Rust library + bastionkv CLI, explicit binary-safe configuration/errors, a checksummed framed WAL, immutable full-map MVCC, atomic optimistic batches, process locking, offline verification/dumps, crash recovery, real generation checkpoint/compaction and statistics. Zero external Rust dependencies and no unsafe. Initial empty repository/default main preserved; no unrelated content, force push, release or deployment.

Success acknowledgment follows WAL sync and atomic map publication. Unacknowledged/failed commits may recover entirely old or entirely new; after sync they must recover new. Any intervening commit conflicts globally. Snapshots retain their original map through newer writes/deletes/compaction and are process-local. Compaction publishes a synced generation pair through atomic manifest replacement before cleanup. Linux filesystem/hardware persistence assumptions are explicit.

## Executed assurance

33 regular tests + one compiling doctest, plus executed opt-in 256-seed × 5000-step reference stress (1,280,000 steps). Bounded reference campaign covers 32 × 200 steps. Six commit crash points, ten compaction crash points, twelve injected I/O error scenarios; crash suites require SIGABRT. Every-byte WAL/segment cuts/flips, checksum-valid structural corruption, sequence/generation exhaustion, symlink framing, concurrency/conflicts, stale-temp handling, non-mutating verifier and real kernel file-size-limit short-write recovery. Rust 1.89 and 1.99 local compiler/Clippy/package gates, documented example/CLI commands, and repository-bound GitHub assurance runs. Non-root permissions and read-only verification are checked by CI. The immutable green repaired-implementation observation is preserved in ci-observation.json; final enclosing commit runs the same workflow plus file-hash verification.

## Benchmarks

Three 1000-key runs preserve all raw CSV samples, environment/source checksums and median/p95/p99/throughput summaries. The full workload set includes four batch sizes, warm reopen/WAL recovery and compaction; SQLite synchronous single-key sequential writes calibrate only that comparable workload. Size and format-byte counters are reported; no superiority claim. See benchmarks/summary.json and metrics-*.json for actual numbers; no invented performance figures.

## Limits and handoff

Bounded database lives in RAM, full map clones per commit; default logical state budget 32MiB, 128 snapshot cap, logical accounting is not RSS. Global conflicts, synchronous-only commits and full blocking explicit compaction. No persistent snapshots, client idempotency, physical power-cut verification, hostile-directory-owner sandbox or non-Linux durability claim. Reopen resolves uncertain I/O outcomes; no automatic corrupt-data salvage.

Read manifest.json, falsification.md, files.sha256 and docs before changing protocols. Every hard brief group is mapped to implementation and evidence. Hash list excludes itself and is anchored by enclosing Git commit; CI validates it from a fresh checkout.

Verdict: VERIFIED FOR DECLARED LOCAL TARGET with documented persistence assumptions and scope. Production certification, SOTA, performance superiority and other-platform support are unverified and not advertised.

Strongest future work (not acceptance blockers): persistent ordered-map structural sharing to reduce clone costs; recovery replay without repeated full-map cloning; controlled filesystem/VM/device power-cut experiments; maintained additional platform lanes with their own sync/rename protocols. Preserve all reference/crash/corruption contracts during optimization.
