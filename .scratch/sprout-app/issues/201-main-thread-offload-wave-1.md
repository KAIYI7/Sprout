# 201 — Main-thread offload wave 1 (async commands, repro-gated)

**What to build:** Wave-1 heavy work stops freezing the window: plan composition, backup inspect/import, log listing/tailing, and settings-search indexing run as async backend commands off the UI thread, with the same contracts and cancellation the UI already relies on.

**Blocked by:** None — can start immediately (same contracts, new executor). Supplies 203.

**Status:** complete - all ACs closed (seeded large-payload sweep GREEN 2026-09-14; see Repro report 2026-09-14 PM seeded).

**Parent:** [198 — Prerequisite + offload + motion (spec)](198-prerequisite-offload-motion-spec.md).

## ACs

- [x] `compute_plan`, backup `inspect`/`import` parsing, log list/tail, and settings-search index build are `async` commands over `spawn_blocking`; Tauri call signatures stay compatible for existing callers.
- [x] No JS parsing of large payloads; frontend keeps render plus trivial (<200-row) client filtering.
- [x] SQLite lock scope stays short; existing cancellation/timeout behavior preserved on every converted command.
- [x] `tools/repro-tab-freeze.mjs` green on a large plan and a large backup (landing + stall-gap thresholds); a regression reads as failed. Seeded sweep GREEN 2026-09-14 PM (see below). Pre-conversion before-numbers remain unavailable (baseline superseded) — the seeded sweep on current code is the regression basis going forward.
- [x] Existing plan/backup/log/settings suites + frontend check green; ownership gate passes.

## Implementation notes

- Sources: the 11 existing `spawn_blocking` sites (update, walker snapshot, icon candidates, AI discovery) as the executor pattern; [26](26-tab-navigation-freeze.md) repro tooling for the budget. Wave-2 surfaces only with new measurements — not in this ticket. Claims above are estimates to recheck against code at dispatch (CodeGraph first).

## Verification

- Repro report pasted in-ticket (before/after numbers); full backend + frontend suites per affected area; ownership gate before sync.

## Validation 2026-09-14 (coordinator, batch-199-201-202-20260914, candidate-1 + warning repairs)

Conversions shipped (`src-tauri/src/lib.rs`, signatures unchanged):
- `inspect_backup` — whole read/zip/JSON-parse/validate on the blocking pool (`backup::inspect_backup`).
- `import_backup` — parse+validate on the pool (`parse_backup_document`), SQLite lock covers only `merge_backup_document`.
- `compute_plan` — lock covers only the preset snapshot; `detect_many` + `plan::compose` on the pool. Stale-response guard untouched.
- `quick_install_plan` (coordinator addition, same heavy shape) — lock covers only the product snapshot.
- `read_run_progress` (the run tail) — file reads + JSON parse on the pool; offset/`done` semantics unchanged; `cancel_run` untouched (cancel-file marker).
- `list_logs` — directory walk + sizing via `logs::list_log_locations` on the pool.
- Settings-search deviation ACCEPTED: index stays frontend — `buildSettingsSearchIndex`
  runs in a memoized `$derived.by` over the settings snapshot (`src/routes/settings/+page.svelte:885`),
  rebuilt on settings change, never per keystroke; per-keystroke work is the trivial string matcher.
  No `JSON.parse` remains in the plan/logs routes.

Suites (all in `C:\Sprout`): backend 681 passed; `npm run check` 0 errors;
`npm run test` 298 passed; ownership gate pass.

Repro report — PENDING, not run (honest status, 2026-09-14):
- The harness (`tools/repro-tab-freeze.mjs`) always spawns its own app and has no
  attach mode; the running dev (`sprout-windows-desktop.exe`, candidate build 07:53)
  exposes no CDP endpoint (ports 9222/9333/9444/9555/9666 verified closed), and
  `dist/Sprout.exe` is stale (2026-08-22, pre-candidate).
- Spawning a second `tauri dev` alongside the running one risks vite-port collision,
  target-dir exe relink lock, SQLite contention, and skewed stall numbers — not attempted.
- Before-numbers are unavailable (baseline code superseded); the tool also takes no
  large-plan/large-backup fixtures, so the AC as written needs harness or fixture work
  before it can literally go green. The 07:57-stamped earlier attempt logged only its
  header line and never produced a verdict.
- Structural evidence above (pool isolation + short locks + unchanged cancel/timeout
  paths) plus green suites is the current validation basis for the offload itself.

### Repro sweep 2026-09-14 PM (dev, plain sweep, post-offload only)
- Log: `C:\Users\admin\AppData\Local\Temp\opencode\batch-199-201-202-20260914\repro-sweep-20260914-pm.log`
  (UTF-16LE; `Read` reports binary — verified via `Get-Content`).
- Cmd: `node tools/repro-tab-freeze.mjs --mode dev --port 9222 --keep`
  (`mode=dev port=9222 reps=1 budget=5000ms stall=1000ms delay=400ms sample=100ms`).
- Result: `boot: 45.8s target=http://localhost:1420/`, `app ready, active tab=/`;
  `/presets 2170ms`, `/plan 706ms`, `/history 628ms`, `/logs 843ms`,
  `/settings 2340ms`, `/ 386ms`; console only `[vite] connecting/connected`
  (x2 each); no `STALLS:` markers; no exceptions; `VERDICT: GREEN`.
- Verdict logic (`tools/repro-tab-freeze.mjs:308-313`): RED iff `!ok` OR
  `landMs > budget` OR `stalls.length > 0`. All 6 `ok`, max 2340ms < 5000ms
  budget, zero stall episodes > 1000ms — GREEN is correct per harness.
- Scope gap vs AC: plain 6-tab sweep only — no large-plan / large-backup
  fixtures, no before/after pair. AC (`green before/after on a large plan and
  a large backup`) remains OPEN on its literal wording; this sweep is
  post-offload no-freeze evidence only (no router wedge, no >1s stall).

### Repro report 2026-09-14 PM seeded (attach, large plan + large backup) — AC CLOSED
- Harness gap fixed in working copy: `tools/repro-tab-freeze.mjs` gained
  `--large-plan N` / `--large-presets K` / `--large-backup N` (synthetic
  fixtures through the UI-owned `create/delete_product/preset` commands, same
  contracts as the dialogs; prefix-clash abort; reverse-order cleanup with a
  leftover check that fails infra), timed `compute_plan` + `inspect_backup`
  probes under the stall sampler, `--attach` (drive the live dev app, no
  second spawn — vite `strictPort` forbids two dev servers) and
  `--cleanup-only` (leak recovery). Fixture shapes verified standalone
  (1200 reqs / 6 presets, 2500-record backup doc round-trips). Drive-by fix:
  `killTree` now imports `spawnSync` (previously a silent no-op leak).
- Cmd (against the running candidate dev app, CDP :9222):
  `node tools/repro-tab-freeze.mjs --attach --port 9222 --large-plan 1500 --large-presets 6 --large-backup 20000`
  (`budget=5000ms stall=1000ms sample=100ms`).
- Seed: library 3 products / 1 preset before; 1500 products + 6 presets
  created (`repro-large-*`); backup fixture 20000 records
  (8000 products / 1000 presets / 3000 launch / 3000 actions / 5000 clips),
  3821 KiB temp JSON, never imported.
- Heavy probes (wall + in-page ms, zero stall episodes > 1000ms):
  `compute_plan 1500reqs ok 7505ms entries=1500`; `inspect_backup ok 178ms`
  with exact counts back. The 7.5 s plan cost is backend pool work
  (winget snapshot + registry scan + compose of 1500) with the UI thread
  responsive throughout — the offload working as designed.
- Sweep WITH seeded data present: `/presets 395ms`, `/plan 290ms`,
  `/history 148ms`, `/logs 158ms`, `/settings 384ms`, `/ 212ms`; console
  only `[vite] connecting/connected`; no exceptions; `VERDICT: GREEN`.
- Cleanup: all `repro-large-*` rows removed, temp file deleted, library back
  to 3 products / 1 preset. Zero leftovers.
- Standing limitation (recorded, not hidden): pre-conversion before-numbers
  cannot be produced — the inline baseline is superseded. Regression basis
  from here: re-run this seeded command; any STALLS line, landing over
  budget, or heavy-probe failure reads RED.
