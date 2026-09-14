# 198 — Prerequisite validation + main-thread offload + motion tokens (spec)

**Status:** planning package only — no application behavior changed here. Tickets 199–203 `ready-for-agent`.

**Parents / related (read before implementing):**
- [145 AI-assisted Quick Action authoring](145-ai-assisted-quick-action-authoring-spec.md) + [184 clarify template](184-ai-clarify-template-scoped.md) — own the `unknown-prerequisite` clarify aspect and fixture corpus. This round puts real detection behind that wording; 184's structure and ADR-0030/0031 boundaries are unchanged.
- [50 storage/runner](50-quick-actions-storage-and-runner.md) / [51 editor page](51-quick-actions-editor-page.md) / [62 run tracking](62-quick-action-run-tracking-and-stop.md) — Quick Action execution model. Unchanged here.
- [175 pre-action backend](175-pre-action-backend-check-fix.md) / [176 pre-action dialog](176-pre-action-frontend-dialog.md) — check-first warn+guidance UX is the prior art for the prereq warn contract.
- [26 tab-navigation freeze](26-tab-navigation-freeze.md) + `tools/repro-tab-freeze.mjs` — the perf budget source (per-click landing, stall-gap threshold).
- [196 animation switch](196-global-animation-toggle-menu-motion.md) — owns the token contract and the instant-off path this round reuses. This round supersedes 196's `accordions stay instant-open` scope (accordion animates under tokens when Animation is on; see Implementation Decisions).
- [166 field cleanup](166-field-cleanup-dock-filter-companion-navigation-spec.md) + [167 selective guidance](167-selective-field-guidance-cleanup.md) — helper-text policy; prereq guidance copy must obey it.
- ADRs: 0028 (tokens/components only), 0029 (one Windows-invocation owner), 0030 (draft-only, never executes), 0031 (scoped discovery/disclosure), 0032 (bundled skills, no custom/remote), 0034 (motion tokens), 0019 (dock driver owns dock motion). Research: [0020 Discord motion philosophy](../../../docs/research/0020-discord-motion-philosophy.md). Glossary: `Prerequisite`, `Motion token` (accepted, in `docs/CONTEXT.md`).

## Problem Statement

A user asks Sprout to draft a script that needs something the machine may not have — Playwright, a language runtime, a module, an editor extension — and today the draft can only guess: disclose-or-clarify with no verified signal, so drafts ship with imagined install states. Separately, heavy work (plan composition, backup inspect/import, log handling, search indexing) can run on the frontend JS thread or inside synchronous backend commands, freezing input and paint while it runs. And motion is scattered per-component values with no large-surface budget: dialogs, chevrons, accordions, and inputs each invent timing, so the app never feels uniformly fast.

## Solution

Ship read-only prerequisite detection behind the existing clarify UX: a single-owner backend check reports present / not-found / not-verifiable per prerequisite, the pinned skill gains a Prereqs section, and unknown-prerequisite clarification surfaces verified results with install guidance — warn, never block, never auto-install, never execute. Move wave-1 heavy work off the UI thread into async backend commands over the managed thread pool, keeping the JS side to rendering. Extend the token system per ADR-0034 (`--dur-slow:280ms`, `--ease-spring`) and roll it through dialogs, chevrons, accordions, inputs, menus, and packet cards — easing for announcements, restrained spring for interruptible arrivals — with the existing reduced-motion/off path collapsing everything to instant. Visual design is unchanged.

## User Stories

1. As a Playwright requester without it installed, I want the draft to tell me Playwright was not found and how to install it, so I don't receive a script that pretends it works.
2. As a Node/Python requester, I want runtime presence and version checked read-only, so version-gated syntax is caught before I save.
3. As an offline user, I want an honest `Not verifiable` instead of a guess, so I know what was actually checked.
4. As a vague requester, I want unknown-prerequisite clarification choices grounded in detection results, so picking one narrows the draft truthfully.
5. As a privacy-conscious user, I want detection to read install state only (no script execution, no network), so checking never changes my machine or discloses anything.
6. As a preset composer, I want plan computation to never freeze the window, so large compositions stay interactive.
7. As a backup user, I want backup inspect/import parsing off the UI thread, so big backups don't stall the app.
8. As a log reader, I want log listing/tailing to stay smooth, so history remains browsable during runs.
9. As a settings searcher, I want the search index to build without jank, so filtering feels instant.
10. As a dialog user, I want dialogs to arrive in ~200ms and leave in ~100ms, so open/close feels snappy, not sluggish.
11. As a Disclosure/accordion user, I want chevrons to rotate in 120ms and panels to settle in 200ms with a subtle arrival, so expansion feels physical but never bouncy.
12. As a form user, I want textbox/textarea feedback limited to border-color in 120ms, so typing never shifts layout.
13. As a motion-sensitive user, I want the Animation switch and OS reduced-motion to render every end state instantly, so the new motion never harms me.
14. As a keyboard/screen-reader user, I want all motion to preserve focus order and announcements, so animation is decoration, never information.
15. As a maintainer, I want one Windows-invocation owner, token-only motion values, and async heavy commands, so the next change has one place to land.

## Implementation Decisions

### Current vs accepted vs proposed

- Current (verified via CodeGraph/source, not prose): `create-quick-action.md` has shell-quoting + disclose-or-clarify prereq prose with no detection call; `unknown-prerequisite` clarifies with generic choices; 11 backend sites already use `spawn_blocking` (update, walker snapshot, icon candidates, AI discovery) while list/plan/backup/log commands still run inline; tokens carry `--ease-out` + `--dur-fast:120ms` + `--dur:200ms` with per-component transitions, `Dialog` fade 140ms, `PacketCard` rise 360ms, 196's global off-switch in place.
- Accepted-but-unimplemented assumed: 146/150–152/155 broader qualification; 147 shell-aware backup v2; 167 copy rules; ADR-0034 token values (accepted, pending rollout).
- Proposed here: (a) one read-only `detect` command under the process/shell owner + skill Prereqs section + clarify surfacing; (b) wave-1 async conversions with the repro budget as gate; (c) token rollout to six surfaces with asymmetric enter/exit. No elevation/tracking/backup-format change, no second invocation site, no custom skills, no new animation library, no dock-geometry change.

### Prerequisite detection (extends ADR-0032 skill; ADR-0029 ownership)

- New read-only detect command owned by the process/shell invocation owner (`windows_execution/`): closed v1 source list — PATH lookup, winget install state, runtime `--version` probes (`node`, `python`), Playwright probe, editor-extension probe. Timeboxed, cancellable, no network, never executes draft text (ADR-0030 holds).
- Result per prerequisite: `present` (with version where meaningful), `not-found for X`, or `not-verifiable` (offline/timeout/uncertain — e.g. "Not verifiable — offline model, I cannot search online"). Uncertain wording: "Not found for X" only when the check actually ran; otherwise `Not verifiable`. Never auto-installs, never blocks saving; warn + guidance copy obeying 167 (concise constraint + install command, no repeated tutorials).
- Skill change is append-only to `create-quick-action.md`: a Prereqs section (target 5.1/CMD explicitly, disclose-or-detect unknown prerequisites, record them in `assumptions`). `unknown-prerequisite` clarify choices surface detect results where available; answering never widens disclosure (ADR-0031 unchanged).

### Main-thread offload (repro budget gates)

- Wave-1 conversions to `async` commands over `spawn_blocking`: plan composition, backup inspect/import parsing, log list/tail, settings-search index build. Short SQLite lock scope, existing cancellation preserved. JS keeps render plus trivial (<200-row) client filtering; no JS parsing of large payloads.
- Budget: `repro-tab-freeze.mjs` thresholds (per-click landing, stall-gap). A conversion that regresses the repro reads as failed.

### Motion rollout (ADR-0034; supersedes 196 accordion scope)

- Tokens only: add `--dur-slow:280ms` + `--ease-spring: cubic-bezier(0.34, 1.3, 0.64, 1)`; keep `--ease-out`, `--dur-fast`, `--dur`. No ad-hoc durations/easings (ADR-0028); deviation goes in the ticket for review.
- Asymmetric enter/exit: Dialog in 200ms decelerate / out 100ms accelerate + fade; chevron rotate 120ms; accordion 200ms opacity/transform (this replaces 196's instant-accordion scope when Animation is on; off/reduced-motion stays instant); inputs border-color 120ms only; `ContextMenu` `menu-in` unchanged values; `PacketCard` entrance capped at 300ms (down from 360ms).
- Transform/opacity (+ border-color/color for inputs) only; never layout properties; never `transition: all`. Focus order, announcements, and 0004-rule-5 feedback preserved. Dock driver untouched (ADR-0019).

### Modules and test seams

- One deep detect seam (probe → per-prereq verdict) with deterministic fakes; reuse the existing draft/clarify seam, no new dispatcher. Heavy commands keep their existing Tauri seams, converted to async — same contract, different executor. Motion has no logic seam: tokens + reduced-motion/off matrix is the verification surface. Windows process/shell mechanics stay with their current owner; no second invocation site.

## Testing Decisions

- Test observable behavior at the seams with deterministic fakes; no prompt-string snapshots or source-text assertions.
- Detect: present/not-found/not-verifiable matrix per source, timeout → not-verifiable, zero script-execution calls, timebox honored, both shells' drafts unaffected.
- Offload: repro-tab-freeze green before/after on a large plan + large backup; async cancellation preserved; existing plan/backup/log/settings suites green.
- Motion: On/Off × OS-reduce matrix (instant when either off, token-accurate when both on), light/dark, real DPI, keyboard-only; no layout-shift assertions beyond transform/opacity + border-color property audit.
- Prior art: 184 fixture battery + `ai-eval-fixtures.json`, 196 toggle matrix, 26 repro tooling, `quickActionDialog`/`aiDraftDialog` frontend tests, `ai_assist` request/output tests, backup merge tests, ownership gate before sync.

## Out of Scope

- Auto-installing missing prerequisites; executing drafts to probe; network-based detection; new provider/runtime support (146/150–152); diagnosis-skill changes (153); persistent capability cache beyond the session.
- Wave-2 offload (icons already done; discovery already done; further surfaces only with new measurements).
- New animation library, Svelte-spring physics, per-surface custom curves, dock-geometry or reveal-gate changes, dynamic presence, art assets.
- Elevation/tracking/backup-format/preset-engine changes; unrelated runner fixes.

## Further Notes

- Pending decisions assumed: 146–152 qualification state; 147 backup v2; 167 copy coverage of new prereq strings (reconcile at implementation, do not re-add removed hints).
- Research impact: none new (0020 stands). Glossary `Prerequisite`/`Motion token` already accepted; owning tickets mark verified delivery.
- If implementation meets a genuinely new trade-off, it ships a dated ADR amendment in the same unit — no silent overwrite.

## Ticket map and integration ownership

| Ticket | Behavioral prerequisites | Likely paths / owner symbols | Shared contract and integration edits | Candidate wave |
| --- | --- | --- | --- | --- |
| [199 Prereq detect backend](199-prerequisite-detect-backend.md) | None — new read-only command under existing owner | Process/shell invocation owner; Tauri command seam + API/type additions | Settles verdict shape (`present`/`not-found`/`not-verifiable` + version), timebox, no-execution proof; supplies 200/203 | 1 — independent |
| [200 Prereq skill + dialog surfacing](200-prerequisite-skill-dialog-surfacing.md) | 199 detect contract | Pinned skill resources; Add/Edit dialog clarify render; copy per 167 | Appends Prereqs section only; clarify choices surface detect results; warn-never-block; supplies 203 | 2 — after 199 |
| [201 Main-thread offload wave 1](201-main-thread-offload-wave-1.md) | None — same contracts, new executor | Plan, backup, logs, settings-search owners; async command conversion | `async` + `spawn_blocking`, short locks, cancellation kept; repro budget gates; supplies 203 | 1 — independent |
| [202 Motion token rollout](202-motion-token-rollout.md) | ADR-0034 + 196 off-switch assumed | Token sheet; Dialog/Disclosure/accordion/input/menu/card components | Two tokens, six surfaces, asymmetric enter/exit, 300ms cap, 196-accordion supersede note; supplies 203 | 1 — independent |
| [203 Round verification](203-round-verification-integration.md) | 199 + 200 + 201 + 202 contracts | Coordinator-owned: checks, repro, matrices, docs | Combined verification, manual matrices, glossary/ADR/research reconciliation, ownership gate + verified sync | 3 — after 199–202 |

The **spec-198 integration coordinator** (ticket 203) owns shared copy/contract reconciliation, parent status, 199↔200 detect/skill handoff, and final combined verification. Workers update their own ACs/results, never rewrite global docs. Claims above are estimates to recheck against code at dispatch (CodeGraph first). If executed as a concurrent batch, follow `docs/agents/parallel-tickets.md` with one coordinator. No parallel implementation runs during this publication session.

## Acceptance and verification

- [ ] User confirms detect sources v1, warn-never-block UX with honest `Not verifiable` wording, wave-1 offload list, and motion token values/scope.
- [ ] 199 verifies verdict matrix, timebox, no-execution proof, ownership gate.
- [ ] 200 verifies skill append-only diff, clarify surfacing with choices, both shells, no disclosure widening.
- [ ] 201 verifies repro-tab-freeze green before/after, cancellation preserved, existing suites green.
- [ ] 202 verifies On/Off × reduce matrix, token-only audit, keyboard/DPI/light-dark.
- [ ] 203 records checks, manual matrices, reconciles CONTEXT/ADR/research status, publishes each completed unit through `node tools/ownership-gate.mjs` + verified `tools\sync.ps1 -Up` (twice, expect 0 copied).

No application behavior changed in this planning session.
