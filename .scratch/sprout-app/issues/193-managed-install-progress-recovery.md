# 193 — Managed install progress as app state with recovery

**What to build:** Install progress that survives tab switches (bar + % + bytes, cancel everywhere) plus explicit recovery for the occupying-revision guard.

**Blocked by:** None — can start immediately (assumes 151 install flow + 186 ownership shapes; coordinates files with 189/191 via 197).

**Status:** ready-for-agent

**Parent:** [188 — Managed local lifecycle UX](188-managed-local-lifecycle-ux-spec.md).

## ACs

- [x] App-level store fed by backend progress events (phase runtime-vs-model, bytes, %); row-level compact bar + dialog full bar; Cancel in both places; survives tab switch; reload shows honest `Interrupted — safe to retry` (single-flight stays in-memory).
- [x] Guard-hit (`already occupies this model revision`) offers `Remove that revision and retry` for that id + `Keep files`; never auto-deletes; copy splits incomplete (retry offered) vs incompatible (update Sprout); staging cleanup unchanged.
- [x] Single-flight (`already in progress`) preserved across tabs instead of per-tab busy string; failure/cancel never exposes partial as installed.

## Verification

Controlled download fixtures: %/bytes/phases, tab-switch persistence, cancel-from-row/dialog, guard-hit retry/keep paths, single-flight across tabs, interrupted-never-ready; managed + Settings suites green + ownership gate.

## Implementation notes

Estimates to recheck at dispatch: downloader progress channel, install-event plumbing, Settings store/row/dialog. No new Windows-invocation site.

Follow-ups delivered: progress events carry a `stage` (`downloading`, `verifying-runtime`, `verifying-model`, `extracting`, `activating`) so silent post-download work narrates instead of sitting on 100% (dots breathe under the stage copy, frozen when Animation is off or OS reduced-motion applies); any failed install offers one-click Retry in row and dialog (guard-hit keeps its own remove-and-retry); activation retries once after 500 ms for transient Windows file locks.
