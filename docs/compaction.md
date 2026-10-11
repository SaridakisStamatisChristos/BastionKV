# Compaction and WAL rotation

Full explicit compaction serializes with writes under the one mutex:

1. Encode current live state in next-generation immutable segment.
2. Create segment temp exclusively, write/sync, rename to final name, sync directory.
3. Create empty next-generation WAL exclusively, sync it and directory.
4. Create/sync MANIFEST.tmp selecting new pair+current sequence; atomically rename to MANIFEST; sync directory.
5. Switch live writable descriptor/generation, remove old owned generation/temp files, sync directory.

Before manifest publication, old generation is authoritative and no old files are removed. After durable manifest publication, the complete new pair is authoritative. A crash around unsynced manifest rename may expose old or new manifest; both reference synced data representing the same logical state. Interrupted cleanup leaves unreferenced files that recovery removes. Temp lifecycle and names are defined in the format document.

Segment contains only newest live values. Overwritten versions/tombstones disappear from disk, and WAL is reclaimed. No old segment can feed a later replay after switching generations, so deletes cannot resurrect. Active snapshots already own immutable maps, so old disk versions are never needed. Generation advances on every checkpoint even with no state changes; sequence doesn't. Generation exhaustion fails rather than wraps.

Errors poison the handle, preventing reuse after an uncertain manifest outcome. Reopen first cleans pending next-generation files using the authoritative manifest, so a subsequent compaction can reuse the next number safely. File permissions or cleanup errors are reported; no acknowledged data is dropped to hide them. One-segment explicit checkpointing is real compaction, not incremental/background compaction. It blocks new registrations/writes/ordinary stats for its full I/O duration; already-held snapshot reads proceed.
