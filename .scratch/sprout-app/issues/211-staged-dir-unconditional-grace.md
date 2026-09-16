# 211 — Staged-dir unconditional grace

**Parent:** [205 — Companion picker cap, Python detection + shell, file Download, staging grace (spec)](205-companion-cap-python-download-staging-grace-spec.md)

**What to build:** Handed-off opens stop racing the cleanup: the per-run staged directory always lingers past shell exit, so a cold-starting viewer or player finds its file whether or not a child process was observed.

**Blocked by:** None — can start immediately (needs only the 179 lifetime).

**Status:** done — implemented 2026-09-16, ACs verified.

- [x] Release always lingers past shell exit even with no observed children (repairs the observe-then-grace TOCTOU hole); drain-then-handoff behavior retained when children exist
- [x] Non-file runs provably unchanged (no placeholder means no staging, no lingering); orphan sweep and 24h ceiling untouched
- [x] Cold-start opens verified for a document (pdf) and audio (mp3) handler: staged path resolves after shell exit
- [x] Verification: unit tests through the existing grace parameter (immediate-release case now lingers, drain case unchanged); backend suite + ownership gate green

**Explicitly not built:**

- Quoting/expansion changes; new viewer-launch policy; navigation handling (171)

## Done notes (2026-09-16)

- Fix in `src-tauri/src/windows_execution/files.rs`: `release_staged_dir_with_grace`
  no longer gates the grace on `children_alive(shell_pid)` — it drains live
  children when any exist, then always sleeps the grace before cleanup. Repairs
  the TOCTOU hole where a handed-off viewer that had not yet appeared in the
  snapshot got an immediate delete. Doc comment updated to match (snapshot
  failure still lingers one grace; no new snapshot site, ADR-0029 owner
  unchanged).
- Non-file runs untouched: `start_tracked_run` only stages + releases when the
  command carries `<FilesDir>` (`lib.rs`); sweep prefix + 24h ceiling untouched.
- Tests: `release_without_children_deletes_on_shell_exit` replaced by
  `release_without_children_lingers_past_shell_exit` (1s grace, asserts linger
  then delete); new `handed_off_open_finds_its_file_after_shell_exit` stages
  `doc.pdf` + `clip.mp3`, exits the shell, and reads both staged paths back
  mid-grace from a release thread before the join reaps the dir; existing
  `release_waits_out_a_live_child_before_deleting` unchanged and green.
- Verification: `cargo check` clean (2 pre-existing warnings elsewhere);
  `cargo test windows_execution::files` 23 pass; `cargo test file` 50 pass;
  ownership gate pass.