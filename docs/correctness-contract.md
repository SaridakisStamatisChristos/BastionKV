# Correctness contract

This document defines the externally meaningful invariants that BastionKV's implementation and verification suite are expected to preserve. Concrete evidence is indexed by [`evidence/manifest.json`](../evidence/manifest.json).

## Requirements

| ID | Requirement / invariant | Primary acceptance evidence | Component |
|---|---|---|---|
| R01 | Binary keys, empty values, bounds, deterministic scans, typed errors | API and malformed-input tests | API / codec |
| R02 | Atomic synchronous commits with explicitly documented unknown I/O outcome | subprocess crash points and reference oracle | engine / disk |
| R03 | Stable point/scan snapshots and explicit optimistic conflict policy | snapshot, concurrency, and conflict tests | engine |
| R04 | Explicit checksummed/versioned format with bounded parsing | codec roundtrips and corruption tests | codec |
| R05 | Idempotent recovery, incomplete-tail handling, middle-corruption refusal | truncation and corruption tests | disk |
| R06 | Crash-safe checkpoint/rotation/compaction with no delete resurrection | compaction crash matrix and reopen model | disk / engine |
| R07 | Multiple readers, coordinated writers, process locking | concurrency and subprocess lock tests | engine / disk |
| R08 | CLI with read-only integrity verification plus WAL/segment inspection | CLI integration and mutation-free verification tests | CLI |
| R09 | Deterministic fault hooks, reference-model checks, corruption tests, opt-in stress | test suite and stress target | tests |
| R10 | Reproducible workload benchmarks with raw data and environment metadata | benchmark script and captured evidence | benches / scripts |
| R11 | Accurate statistics, validated configuration, bounded corruption handling | semantics and corruption tests | API / codec |
| R12 | Architecture, storage, recovery, compaction, transaction, threat, and limitation documentation | documentation build and examples | docs |
| R13 | Formatting, warnings, Clippy, tests, rustdoc, Linux/MSRV, packaging | CI and `scripts/check.sh` | CI |
| R14 | Repository-bound evidence with integrity hashes and explicit claim limits | evidence manifest and hash validation | evidence |

The engine has no server, network protocol, authentication layer, or distributed component. Those concerns are outside the current product boundary.

## Platform assumptions

- The supported durability target is a Linux local filesystem providing working `fsync`, same-directory atomic rename, and advisory locking.
- The live database is held in a bounded in-memory ordered map; published snapshots use immutable `Arc`-owned maps.
- Compaction is explicit and synchronous. A checkpoint segment plus generation WAL is the only durable generation pair.
- The repository uses a proprietary license and currently has no third-party Rust crate dependencies.

Changing any of these assumptions requires corresponding documentation, implementation, and verification updates before widening product claims.

## Data and transaction semantics

- `Database::create` initializes a new database; `Database::open` never creates a missing one.
- Default key/value/frame/state bounds are 4 KiB / 1 MiB / 64 MiB / 32 MiB.
- Keys are non-empty; values may be empty.
- Range lower bounds are inclusive and upper bounds are exclusive.
- Write-transaction reads include staged changes over their immutable base snapshot.
- Any intervening successful commit conflicts an optimistic writer, including disjoint-key commits.
- Successful non-empty commits advance the global sequence exactly once.
- Empty commits validate their base sequence without appending a WAL frame.
- A single serialization mutex covers conflict validation, encoding, WAL append, file synchronization, and map publication.
- Read snapshots register under the same mutex and then read from immutable maps without engine locking.
- At most 128 read/write snapshots are registered per handle.
- Any possibly state-changing I/O failure poisons the handle; reopen is required to determine the atomic outcome.
- Expected input, format, lock, and I/O failures are returned as typed errors rather than panics.

## Module boundaries

`error` and `config` define shared types and validation. `codec` owns the binary format and parsing rules. `disk` owns file locking, WAL/segment/manifest persistence, recovery, and compaction. `lib` owns the public database API, immutable-map publication, snapshots, and transaction semantics. The CLI, examples, tests, and benchmarks consume public interfaces.

The durable format does not persist an offset index. Records are validated and replayed into the ordered in-memory map. No custom lock-free database-state protocol or unsafe Rust is used.

## Change discipline

Any change to commit ordering, sync points, format framing, recovery decisions, generation publication, process locking, snapshot visibility, or corruption handling must preserve the relevant requirement IDs above and keep the evidence manifest synchronized.
