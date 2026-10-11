# R7 contract and topology lock

Source: user brief BASTIONKV_R7_ZERO_TO_HERO.md. Baseline: inspected empty repository; bootstrap commit e31e8d3b8774b48291adf91005fa30f57c068279, main. No pre-existing implementation. Every row is HARD, USER, applicable to this embedded Linux engine; initial state OPEN. Final states and concrete evidence live in evidence/manifest.json.

| ID | Requirement / invariant | Acceptance evidence | Owner |
|---|---|---|---|
| R01 | Binary keys, empty values, bounds, deterministic scans, typed errors | API and malformed input tests | lib/codec |
| R02 | Atomic synchronous commits; unknown outcome explicitly defined | subprocess crashes at append/sync/publication; reference oracle | engine |
| R03 | Stable point/scan snapshots; explicit optimistic conflict policy | snapshots, concurrent writes, conflict tests | engine |
| R04 | Explicit checksummed/versioned format; bounded parsing | roundtrip, mutation, length, sequence, offset tests | codec |
| R05 | Idempotent recovery, truncated tail handling, middle corruption refusal | every-byte truncation and middle damage tests | disk |
| R06 | Real crash-safe checkpoint/rotation/compaction; no resurrection | crash matrix plus delete/reopen model | disk/engine |
| R07 | Multiple readers, coordinated writers, process lock | thread tests, deterministic ordering model, subprocess lock tests | engine/disk |
| R08 | CLI including readonly integrity inspection, WAL/segment dumps | CLI integration and mutation-free verifier tests | CLI |
| R09 | Deterministic fault hooks, properties/reference, corruption, stress | tests + stress invocation | tests |
| R10 | Reproducible full workload benchmarks with raw data/metadata; optional fair baseline | benchmark script and captured output | benches/scripts |
| R11 | Accurate stats, validated configuration, corruption resource bounds | stats, limits, parser adversarial tests | lib |
| R12 | Architecture, format, recovery, compaction, concurrency, threat, limitations docs | documentation/API checks and executed demos | docs |
| R13 | fmt, warnings, clippy, tests, doc, Linux/MSRV, package CI | actual GitHub run tied to commit | CI |
| R14 | Repository-bound artifacts, real hashes, coverage/claims/repairs/compatibility ledgers | remote tree+CI inspection and manifest | evidence |

Additional SHOULD: dependency audit where supported (zero third-party Rust dependencies); stress manual lane; SQLite calibration. No distributed components/auth/network/containers are applicable: local library+CLI with OS permissions, no server. Loom is replaced with exhaustive order enumeration for this single-mutex publication protocol plus real threads; no custom atomics govern database state. Release/deploy N/A: no publication requested.

## Assumptions

- A01 MEDIUM/high confidence: declared target Linux local filesystem with working fsync/atomic same-directory rename/advisory locks. Invalidate on another platform/filesystem; do not widen durability claims.
- A02 MEDIUM/high: immutable full-map snapshots (copy-on-write MVCC) and global-version optimistic validation; whole database bounded in RAM. Invalidate if workloads demand large-scale storage; affects R03/R10/R11.
- A03 LOW/high: explicit compaction only, single checkpoint segment plus generation WAL. Runtime snapshots do not survive restart. No relaxed durability mode.
- A04 LOW/high: proprietary copyright notice consistent with user's recent repository licensing preference; no third-party Rust code dependencies.

## Spec

Create is explicit, open never initializes a missing database. Default key/value/frame/state bounds: 4096/1048576/67108864/33554432 bytes. A valid configuration reserves frame space beyond state bounds. Snapshot cap 128. Keys nonempty. Range start inclusive, end exclusive; start>end returns invalid input. Write get sees staged writes then its base snapshot. Commit aborts if any other commit advanced the global version (including disjoint keys); nonempty successful commits advance once. Empty commits validate and return the current version without WAL. A serialization mutex covers conflict check, encode, append, sync and publication. Read snapshots take it only to register/copy the Arc. Each immutable map is a version. Drop releases snapshot registration; compaction never needs old maps on disk. Reads already held remain usable after I/O poisoning. Any possibly state-changing I/O error poisons the handle: reopen to determine atomic outcome. Expected errors never panic.

## Plan / topology

Smallest dependency DAG: error/config → codec → disk → engine(public API); CLI/benches/tests consume public API. Disk doesn't call engine. Codec doesn't access filesystem. No unsafe, no external Rust dependencies. No custom lock-free protocol. Index: ordered in-memory BTreeMap; records contain keys+values, no persistent offset index to trust. Full-map clone cost is explicit.

B1 contract/spec/scaffold (this document, Cargo, license); B2 codec+disk, R01/04/05/06/07; B3 engine+CLI, R01/02/03/06/08/11; B4 tests/examples/benchmarks, R09/10; B5 docs/CI/evidence, R12/13/14. Gates follow integration, verification, falsification, selective repair, package, manifest. Final verdict follows weakest HARD gate.
