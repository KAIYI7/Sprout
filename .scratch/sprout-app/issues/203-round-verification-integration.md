# 203 — Round verification + docs reconciliation (spec-198 coordinator)

**What to build:** The round lands as one verified unit: all four slices green together, manual matrices recorded, glossary/ADR/research statuses reconciled, and each completed unit published through the ownership gate plus verified sync.

**Blocked by:** [199](199-prerequisite-detect-backend.md) + [200](200-prerequisite-skill-dialog-surfacing.md) + [201](201-main-thread-offload-wave-1.md) + [202](202-motion-token-rollout.md) contracts. Coordinator-owned; workers update their own ACs/results, never rewrite global docs.

**Status:** ready-for-agent

**Parent:** [198 — Prerequisite + offload + motion (spec)](198-prerequisite-offload-motion-spec.md).

## ACs

- [ ] Combined checks green: backend suite, frontend check, frontend unit tests, production build, ownership gate.
- [ ] Manual matrices recorded in-ticket: prereq present / not-found / offline-not-verifiable × both shells; repro-tab-freeze before/after; motion On/Off × reduce on/off, light/dark, real DPI, keyboard-only.
- [ ] Docs reconciled: `Prerequisite`/`Motion token` delivery marked verified; ADR-0032 amendment (if 200 changed skill text) and 196-supersede note (202) consistent; research 0020 standing (no rewrite); spec-198 Acceptance section checked per completed ticket.
- [ ] Each completed unit published via `node tools/ownership-gate.mjs` (must pass) + `tools\sync.ps1 -Up` twice (expect 0 copied on the second run); divergences reported as `SHARE-NEWER`, never force-overwritten.

## Implementation notes

- Sources: 185/197 round-verification pattern. If run as a concurrent batch, follow `docs/agents/parallel-tickets.md` with this ticket as the single coordinator.

## Verification

- Evidence table (command → result) pasted in-ticket; frozen manual-matrix notes linked; parent spec status flipped to implemented only when every box above is checked.
