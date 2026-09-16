# 209 — Quick Action file Download backend

**Parent:** [205 — Companion picker cap, Python detection + shell, file Download, staging grace (spec)](205-companion-cap-python-download-staging-grace-spec.md)

**What to build:** Attached action files can come back out: a read-only bytes command returns a persisted file exactly as stored, while the list command stays lightweight metadata.

**Blocked by:** None — can start immediately (needs only the 179 table/caps). Supplies 210.

**Status:** done — implemented 2026-09-16, ACs verified (full unfiltered `cargo test` blocked on disk space, see below).

- [x] New Tauri bytes command in the `quick_actions` owner fetches one persisted file by id; list/attach/remove behavior unchanged; list responses stay meta-only (no byte bloat)
- [x] Bytes round-trip exactly (mp3 to mp3, pdf to pdf); unknown id and missing row fail honestly; attach-time caps (5MB/file, 20MB/action) unchanged
- [x] No backup/export format change; no staging-lifetime change (ticket 211 owns that)
- [x] Verification: byte-identity tests incl. binary content, error paths, existing files tests green; ownership gate pass

**Explicitly not built:**

- Any UI (ticket 210); bulk zip (frontend composes it from this command)

## Done notes (2026-09-16)

- Contract: `quick_actions::get_quick_action_file_bytes(conn, file_id)` returns
  the stored `(filename, bytes)`; unknown id fails `"That file is gone — refresh
  and try again."` (same voice as remove). Tauri command `get_quick_action_file`
  base64-encodes at the boundary (consistent with attach/backup/clip-image);
  registered in `generate_handler`; seam `getQuickActionFile` in `api.ts` plus
  `QuickActionFileMeta`-sibling `QuickActionFileBytes` in `types.ts`.
- List/attach/remove/backup/staging untouched: list still selects
  `LENGTH(bytes)` only; new tests assert list stays meta-only while the getter
  returns the bytes.
- Verification: `cargo check` clean (2 pre-existing warnings elsewhere);
  `cargo test file_bytes` 3 pass (incl. 0–255 binary, PDF/MP3 headers),
  plus existing files tests green (filenames/caps/listing/cascade/migrate) and
  backup files tests green (whole-app + single-export-zip); ownership gate
  pass. Full unfiltered `cargo test` could not link on this machine (disk-full
  archiving `sprout_lib.lib`, os error 112 — environmental); all filtered runs
  reuse the already-built test binary with no rebuild.