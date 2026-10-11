# Limits and evidence gaps

- Linux local-filesystem durability target only; other OSes/filesystems unverified. No Windows atomic-replace/directory-sync claim.
- Process crash, deterministic I/O failure and real file-size-limit short writes are tested. Physical power-cut/device/controller behavior is unverified.
- Full map held in memory, cloned per nonempty commit; O(number of live records) preparation. Small bounded databases, not an LSM performance substitute. Scans filter the ordered map and copy results. Default state budget 32MiB; up to 128 snapshots can retain multiple full maps. Logical budget isn't an exact RSS limit.
- Global optimistic conflicts abort disjoint writers; one commit mutex; synchronous individual commits only. No background compaction, automatic WAL-size threshold, cache or relaxed durability.
- WAL size grows until explicit compact; live/staged maps and parser allocations are bounded, WAL history itself is not size capped.
- No persistent snapshot handles, incremental compaction, client-idempotency tokens, SQL, distributed operations, read-only live open or repair utility.
- CRC32C is accidental-integrity protection, not authentication. A truncated incomplete last frame is recoverable warning; a malicious whole-file deletion/rollback may be indistinguishable from a valid prior database. No root-directory race protection against hostile owners.
- Exhaustive serialized schedule tests exercise application protocol outcomes but do not model-check std Mutex internals or filesystems. This implementation has no custom lock-free state protocol; Loom isn't integrated.
- Standard advisory lock inheritance applies to POSIX fork. A fork child retaining descriptors can extend the lock beyond the parent lifetime. Don't fork long-lived children with open handles.
- Interrupted first init requires explicit directory quarantine/removal; open doesn't fabricate missing metadata.
- Statistics counters reset on reopen; reclaimed WAL-byte estimate isn't net disk-space gain. Filesystem errors in stats are returned.
- Benchmarks are host-local descriptive calibration, not superiority claims or performance requirements. Package/source review is still needed for an application's durability/latency/memory requirements.
