# Format version 1

All integers are explicit little-endian unsigned values. No Rust layouts or unsafe dumps. CRC32C uses the reflected Castagnoli polynomial 0x82f63b78, initial/final XOR 0xffffffff; standard check vector `123456789` → 0xe3069283. CRC detects accidental corruption; it is not authentication.

## Directory and manifest

`LOCK`, `MANIFEST`, `segment-<20-digit-generation>.bks`, `wal-<20-digit-generation>.bkw`. Initial generation 1. A single manifest selects exactly one segment/WAL pair. Generation derives names, so no persisted path strings permit traversal. Generation and sequence increment with checked arithmetic. Files referenced by metadata must be regular, non-symlink files.

MANIFEST is exactly 52 bytes:

| Offset | Bytes | Meaning |
|---|---:|---|
| 0 | 8 | ASCII BKMETA01 |
| 8 | 4 | Format version (1) |
| 12 | 8 | Nonzero generation |
| 20 | 8 | Checkpoint sequence |
| 28 | 4 | Maximum key bytes |
| 32 | 4 | Maximum value bytes |
| 36 | 4 | Maximum frame payload bytes |
| 40 | 8 | Maximum logical live/staged-state bytes |
| 48 | 4 | CRC32C of bytes 0..48 |

Unsupported versions fail closed. Limits must pass Config validation before any frame allocation; no silent upgrades. Future incompatible formats require an explicit offline export/migration tool (none offered in v1).

## Frames

Every WAL transaction is one frame. A segment is exactly one checkpoint frame containing only live PUT records, at the manifest checkpoint sequence. An empty initial checkpoint is valid. WAL frames must be nonempty.

| Header offset | Bytes | Meaning |
|---|---:|---|
| 0 | 4 | BKW1 (WAL) or BKS1 (segment) |
| 4 | 4 | Payload length |
| 8 | 4 | Payload CRC32C |
| 12 | 8 | Commit/checkpoint sequence |
| 20 | 4 | Header CRC32C over bytes 0..20 |

Payload: count (u32), `count` records, literal 8-byte COMMIT01 trailer. A record is tag u8 (1 PUT, 2 DELETE), key length u32, value length u32, key bytes, value bytes. DELETE has zero value length; PUT may have zero value length. Keys are nonempty and strictly ascending with no duplicates per frame. Trailer is included in the payload checksum.

The header checksum is verified before trusting length; payload length is checked against persisted bounds and remaining file bytes before allocating. Record count is checked against payload minimum size; each record range and end offset is checked; no unclaimed bytes, unknown tags, tombstones in segments, or invalid lengths are accepted. The index is reconstructed from sorted key records; no on-disk offset index exists. Reported frame offsets/end offsets are derived from validated bounds. This avoids persistent overlapping offset references entirely.

Complete-frame checksum/header/trailer damage fails closed even at EOF. A short final header or a valid-header incomplete final payload is an unacknowledged torn tail, reported by verify and truncated only by exclusive open. An incomplete segment is always corruption. Interior complete-frame damage cannot be skipped to reach later data. WAL sequence must equal previous+1; duplicate/reused/rolled-back/gapped IDs fail closed. A whole-file malicious rollback to another checksummed valid database cannot be detected without an external trusted witness.
