# 210 — Quick Action file Download frontend + start lint

**Parent:** [205 — Companion picker cap, Python detection + shell, file Download, staging grace (spec)](205-companion-cap-python-download-staging-grace-spec.md)

**What to build:** Download lives where the files do: per-file Download in the action edit dialog and row menu, Download-all-zip when there is more than one file, plus a warning when a cmd `start` line will eat its own quoted path.

**Blocked by:** 209 (needs the bytes contract).

**Status:** done — implemented 2026-09-16, ACs verified (manual keyboard/SR/theme/size matrix still owed a human runtime run, per 180 precedent).

- [x] Per-file Download in `QuickActionFormDialog.svelte` files section + quick-actions row menu; `Download all (.zip)` appears iff the action has 2 or more files; main app only, never the dock
- [x] Save happens through the OS Save-As dialog (no silent overwrite); single-file kept as-is, never zipped
- [x] Cmd title-trap lint: a `start` line without an empty title under the cmd shell warns with the `start "" …` / `Start-Process` / `explorer` forms, in the dialog copy voice per 167
- [x] Verification: dialog + menu flows incl. zip threshold and Save-As cancel; lint fires only on the trap shape; frontend check clean; UI cites 0004:3/0006:4

**Explicitly not built:**

- Backend changes beyond the 209 contract; dock or Settings surfaces

## Done notes (2026-09-16)

- New pure module `src/lib/quickActionFiles.ts` (base64 codecs, minimal
  stored-zip writer with fixed zero timestamp for determinism, Save-As filter
  helper, `detectCmdStartTitleTrap`) plus `src/lib/quickActionDownload.ts`
  (one Save-As + write owner shared by the dialog and the row menu). Deletion
  test holds: removing Download removes both modules plus their dialog/page
  wiring. No editor-library and no zip dependency.
- Dialog: per-file Download (ghost, beside its row, persisted rows only) plus
  `Download all (.zip)` (secondary, iff 2+ persist) in the files section;
  row menu: Download flyout (per-file plus zip iff 2+, content-gated so a
  fileless action shows none) after Export. Save-As cancel aborts silently
  like export; success announces through the existing status line.
- Save path: `saveDialog` + `writeFile` from `@tauri-apps/plugin-fs` (new
  standard plugin — `tauri-plugin-fs`, `.plugin(init)`, `fs:allow-write-file`
  with user-consented `**` scope, same unrestricted destination the backend
  export commands already write to). Zip name is `<action>-files.zip` with a
  plain "Zip archive" filter, distinct from the `<action>.zip` backup bundle.
  The `quickActionEditor.test.ts` dependency pin now includes the fs plugin.
- Lint: quote-aware line/statement scan (comments, `REM`/`::`, quoted
  mentions, and non-cmd shells never fire); skips `/d`-family switch args;
  an empty title or an intentional `start "Title" <command>` stays quiet, and
  the warning never blocks Save. Copy keeps 167's plain constraint voice.
- UI rules applied: 0006 pattern 4 (Download beside its file row / on its
  action's menu; lint under the Command field) and 0004 rule 3 (full Download
  + authoring in the main app only — the dock stays read-only). Tokens and
  shared Button/Notice only; no new components, tokens, or dimensions.
- Verification: vitest 16 new pass (bytes/zip/filter/lint shapes, Save-As
  cancel, zip threshold/order); full frontend suite 30 files / 364 pass;
  `npm.cmd run check` 0 errors (3 pre-existing warnings); ownership gate pass.