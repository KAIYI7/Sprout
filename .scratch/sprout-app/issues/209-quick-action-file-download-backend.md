# 209 — Quick Action file Download backend

**Parent:** [205 — Companion picker cap, Python detection + shell, file Download, staging grace (spec)](205-companion-cap-python-download-staging-grace-spec.md)

**What to build:** Attached action files can come back out: a read-only bytes command returns a persisted file exactly as stored, while the list command stays lightweight metadata.

**Blocked by:** None — can start immediately (needs only the 179 table/caps). Supplies 210.

**Status:** ready-for-agent

- [ ] New Tauri bytes command in the `quick_actions` owner fetches one persisted file by id; list/attach/remove behavior unchanged; list responses stay meta-only (no byte bloat)
- [ ] Bytes round-trip exactly (mp3 to mp3, pdf to pdf); unknown id and missing row fail honestly; attach-time caps (5MB/file, 20MB/action) unchanged
- [ ] No backup/export format change; no staging-lifetime change (ticket 211 owns that)
- [ ] Verification: byte-identity tests incl. binary content, error paths, existing files tests green; ownership gate pass

**Explicitly not built:**

- Any UI (ticket 210); bulk zip (frontend composes it from this command)