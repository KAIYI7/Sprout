# 197 — Round verification and integration

**What to build:** Combined green for the 188 round with docs reconciled and each unit publishable.

**Blocked by:** 189 + 190 + 191 + 192 + 193 + 194 + 195 + 196 — starts after their contracts land.

**Status:** ready-for-agent

**Parent:** [188 — Managed local lifecycle UX](188-managed-local-lifecycle-ux-spec.md).

## ACs

- [ ] Combined `npm.cmd run check` (0 errors), vite build, `cargo check` + `cargo test`, ownership gate pass; per-ticket manual matrices (lifecycle/status, disclosure polling, radio switch, pointer route, progress/recovery, rename/update flow, presence-visible, motion matrix) recorded in owning tickets.
- [ ] Reconcile CONTEXT (no new canonical terms; `Active model`/`Managed runtime status` remain display labels), ADR-0033 amendment landed via 195 (no other ADR text changed), research `0006` dated decision update for tier-first + pointer (no standing-rule rewrite; new note only if genuinely new topic).
- [ ] Publish each completed unit via ownership gate + verified `tools\sync.ps1 -Up` (twice, expect 0 copied); never batch unrelated violations past the gate.
- [ ] Confirm 152/153 parallel-runner set below stayed green at the shared seams.

## Verification

Coordinator-owned: full check matrix + manual acceptance log + doc-status table; no application-code change beyond reconciliation fixes.

## Implementation notes

Integration owner for the 188 batch per `parallel-tickets.md`: owns 189↔190↔191↔193 managed-file handoff, 191→192 contract, backup/size overlap. Workers own their tests/repairs. Claims in 188 are estimates — recheck paths/owners via CodeGraph at dispatch.
