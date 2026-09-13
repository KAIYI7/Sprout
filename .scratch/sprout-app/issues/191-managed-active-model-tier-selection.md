# 191 — Active managed model radio with tier-first labels

**What to build:** An explicit Active-model picker over installed managed models with friendly tier names first and full artifact identity one level down.

**Blocked by:** 189 — Start/Stop + status contract (same files).

**Status:** ready-for-agent

**Parent:** [188 — Managed local lifecycle UX](188-managed-local-lifecycle-ux-spec.md).

## ACs

- [x] `Active model` radio lists installed models only, Save-deferred like provider; install/remove no longer implicitly retargets except last-remove forces `off` as today; no `None` mode.
- [x] Row shows `{Tier} — {params} · {size} · {RAM}` via frontend id→tier map (no catalog schema change); Details dialog keeps full artifact/source/revision/hash/license; blocked tier hidden until qualified.
- [x] `aiReady = managed + active∈installed`; idle switch stops old + starts new; busy switch (different model, `active_requests>0`) honestly refuses; no silent switch.
- [x] Copy follows selective-guidance rules; no removed hint re-added; tokens/components only.

## Verification

Fixtures: multi-install select/save, idle-switch, busy-refuse, last-remove-to-off, blocked-hidden, tier-row vs details-identity; managed + Settings suites green + ownership gate.

## Implementation notes

Estimates to recheck at dispatch: Settings managed list, frontend tier map, lifecycle switch path. Settles the `aiReady` contract consumed by 192; reconcile with 152's explicit-switch (no behavior fork).
