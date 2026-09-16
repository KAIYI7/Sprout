# 210 — Quick Action file Download frontend + start lint

**Parent:** [205 — Companion picker cap, Python detection + shell, file Download, staging grace (spec)](205-companion-cap-python-download-staging-grace-spec.md)

**What to build:** Download lives where the files do: per-file Download in the action edit dialog and row menu, Download-all-zip when there is more than one file, plus a warning when a cmd `start` line will eat its own quoted path.

**Blocked by:** 209 (needs the bytes contract).

**Status:** ready-for-agent

- [ ] Per-file Download in `QuickActionFormDialog.svelte` files section + quick-actions row menu; `Download all (.zip)` appears iff the action has 2 or more files; main app only, never the dock
- [ ] Save happens through the OS Save-As dialog (no silent overwrite); single-file kept as-is, never zipped
- [ ] Cmd title-trap lint: a `start` line without an empty title under the cmd shell warns with the `start "" …` / `Start-Process` / `explorer` forms, in the dialog copy voice per 167
- [ ] Verification: dialog + menu flows incl. zip threshold and Save-As cancel; lint fires only on the trap shape; frontend check clean; UI cites 0004:3/0006:4

**Explicitly not built:**

- Backend changes beyond the 209 contract; dock or Settings surfaces