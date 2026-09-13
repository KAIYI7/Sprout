# 190 — Managed resource usage under Details

**What to build:** A collapsed-by-default Details disclosure showing live RAM/CPU/uptime for the owned runtime, costing nothing until opened.

**Blocked by:** 189 — Start/Stop + status contract (same files; needs running/stopped shape).

**Status:** ready-for-agent

**Parent:** [188 — Managed local lifecycle UX](188-managed-local-lifecycle-ux-spec.md).

## ACs

- [x] Shared Disclosure primitive only; collapsed by default; tooltip-grade numbers (working set, CPU %, uptime, active tier); hidden unless `managed` + active installed + running (stopped shows static `Stopped — will start on next Generate`).
- [x] Closed = no command, no interval; open + running = ~2s poll; closed cancels; surviving tab state is disclosure-open only (metrics never persist).
- [x] Tokens/components only; respects Animation switch + OS reduced-motion (numbers update, no animated gauges).

## Verification

Fake usage source: closed-zero-cost assertion, open-poll cadence, stopped-static copy, cancel-on-close; existing disclosure + Settings suites green + ownership gate.

## Implementation notes

Estimates to recheck at dispatch: managed owner for the usage query, Settings disclosure. Consumes 189 contract; no status duplication.
