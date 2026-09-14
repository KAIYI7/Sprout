# 199 — Prerequisite detect backend (read-only, single owner)

**What to build:** A read-only prerequisite check the AI skill and dialog can trust: ask whether X is installed and get back `present` / `not-found for X` / `not-verifiable` — without running any draft text, without network, without changing the machine.

**Blocked by:** None — can start immediately. Supplies 200/203. Makes no skill-template or dialog edits (200 owns those).

**Status:** ready-for-agent

**Parent:** [198 — Prerequisite + offload + motion (spec)](198-prerequisite-offload-motion-spec.md). Extends [ADR-0032](../../../../docs/adr/0032-model-recommendations-and-skills-ship-with-app.md) detection surface; ADR-0029 ownership, ADR-0030/0031 boundaries unchanged.

## ACs

- [ ] One detect command owned by the process/shell invocation owner — no second Windows-invocation site (ownership gate passes).
- [ ] Closed v1 source list only: PATH lookup, winget install state, runtime `--version` probes (node, python), Playwright probe, editor-extension probe.
- [ ] Per-prerequisite verdict: `present` (with version where meaningful), `not-found for X` (check actually ran), `not-verifiable` (offline/timeout/uncertain — never a guess).
- [ ] Timeboxed and cancellable; timeout reads as `not-verifiable`, never as absent.
- [ ] Zero script-execution calls across detect/validate/cancel (startup of managed runtime + fixed read-only discovery remain distinguished, as before).
- [ ] Deterministic tests: present/not-found/not-verifiable matrix per source + timeout path; relevant backend checks/tests green.

## Implementation notes

- Sources: existing timed-capture helper + `winget/authoring` search/show patterns as prior art; 184 fixture style for verdict matrix. Claims above are estimates to recheck against code at dispatch (CodeGraph first).

## Verification

- Backend suite + ownership gate green; verdict-shape contract documented for 200 (field names, version semantics, timebox value).
