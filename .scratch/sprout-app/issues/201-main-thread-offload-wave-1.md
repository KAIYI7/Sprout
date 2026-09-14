# 201 — Main-thread offload wave 1 (async commands, repro-gated)

**What to build:** Wave-1 heavy work stops freezing the window: plan composition, backup inspect/import, log listing/tailing, and settings-search indexing run as async backend commands off the UI thread, with the same contracts and cancellation the UI already relies on.

**Blocked by:** None — can start immediately (same contracts, new executor). Supplies 203.

**Status:** ready-for-agent

**Parent:** [198 — Prerequisite + offload + motion (spec)](198-prerequisite-offload-motion-spec.md).

## ACs

- [ ] `compute_plan`, backup `inspect`/`import` parsing, log list/tail, and settings-search index build are `async` commands over `spawn_blocking`; Tauri call signatures stay compatible for existing callers.
- [ ] No JS parsing of large payloads; frontend keeps render plus trivial (<200-row) client filtering.
- [ ] SQLite lock scope stays short; existing cancellation/timeout behavior preserved on every converted command.
- [ ] `tools/repro-tab-freeze.mjs` green before/after on a large plan and a large backup (landing + stall-gap thresholds); a regression reads as failed.
- [ ] Existing plan/backup/log/settings suites + frontend check green; ownership gate passes.

## Implementation notes

- Sources: the 11 existing `spawn_blocking` sites (update, walker snapshot, icon candidates, AI discovery) as the executor pattern; [26](26-tab-navigation-freeze.md) repro tooling for the budget. Wave-2 surfaces only with new measurements — not in this ticket. Claims above are estimates to recheck against code at dispatch (CodeGraph first).

## Verification

- Repro report pasted in-ticket (before/after numbers); full backend + frontend suites per affected area; ownership gate before sync.
