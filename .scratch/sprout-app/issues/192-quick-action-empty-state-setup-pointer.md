# 192 — Quick Action empty-state setup pointer

**What to build:** One plain-text pointer from the unready Quick Action AI view to the Settings page, preserving zero-chrome.

**Blocked by:** 191 — Active-model/`aiReady` contract.

**Status:** ready-for-agent

**Parent:** [188 — Managed local lifecycle UX](188-managed-local-lifecycle-ux-spec.md).

## ACs

- [x] Unready dialog keeps zero AI tabs/hero; at most one plain-text line (`No managed model is on right now — Enable it` / `Set up AI assistance in Settings`) that routes to the Settings page, expands the AI group, and focuses the provider control; focus-ring is the only highlight.
- [x] No smooth-scroll library, no pulse/flash, no config duplicated in the dialog, no per-group `/settings#` route, no disabled AI teaser.
- [x] Ready state unchanged (AI tab + textarea show; stopped-with-config shows tab + `Stopped — Generate will restart` hint, never the enable line).

## Verification

Gating tests: unready-zero-chrome + pointer-routes-expands-focuses; ready-shows-tab; stopped-shows-tab-not-pointer; keyboard/screen-reader operability; dialog suites green + ownership gate.

## Implementation notes

Estimates to recheck at dispatch: Quick Action dialog empty state, Settings expand+focus idiom. Consumes 191 contract; no execution-path change.
