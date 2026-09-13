# 185 — Round verification + integration coordinator (spec-181)

**What to build:** Combined verification for the spec-181 round, plus sole ownership of shared glossary/ADR/research reconciliation, parent status, and cross-ticket integration.

**Blocked by:** 182 (presence contract), 183 (dialog contract), 184 (skill contract). Coordinator-only — no new behavior of its own.

**Status:** done — all ACs checked 2026-09-13 (combined verification green; docs reconciled; parent spec marked implemented)

**Parent:** [181 — Discord + AI two-view + clarify (spec)](181-discord-presence-ai-two-view-clarify-spec.md).

## Scope

- Owns: 183↔184 dialog/skill handoff, backup/size overlap, 172/167 dialog-copy reconciliation, CONTEXT/ADR/research updates, parent status, overlapping-file integration, final combined manual + automated verification. Workers update their own ACs/results; nothing here rewrites their tickets.
- Publishes each completed unit through `node tools/ownership-gate.mjs` (must pass) + verified `tools\sync.ps1 -Up` (twice, expect 0 copied). If run as a concurrent batch, follow `docs/agents/parallel-tickets.md` with this ticket as the single coordinator.
- Explicitly not built: any presence/dialog/skill behavior beyond integration fixes; any new model/runtime qualification (146/150–152/155 stay open).

## ACs

- [x] 182–184 contracts verified together: static presence + silent-absent + exit-clear; two-view gating/defaults/flips/auto-review; vague→clarify→regenerate with no disclosure widening. Cross-ticket regressions (dialog + skill + presence co-installed) exercised.
- [x] Full relevant suites green: backend checks/tests as affected, frontend check/build as affected, deterministic fixture battery, packaging check (pinned skills + recommendations served without checkout), size-budget note for presence.
- [x] Docs reconciled in this unit: CONTEXT planned qualifiers removed where delivered (`AI Mode`, `Clarification`, `Presence activity` — or record what remains planned with spec link); ADR-0028 + ADR-0032 Amendments appended (no original-text rewrite); research 0006 + 0019 extended with dated decision updates (no rule rewrite; new note only for a genuinely new topic); spec-181 acceptance boxes checked as delivered.
- [x] Manual matrices recorded: presence (Discord open/closed, tray boot, exit), dialog (AI off/on × Add/Edit, apply→review, clarify pick, Dismiss, keyboard-only, light/dark, real DPI), clarify (unknown-folder / ambiguous-app / unknown-prereq answers).
- [x] Ownership gate passes; each completed unit synced via verified `-Up` (twice, 0 copied); parent spec status updated to implemented with per-ticket evidence links.

## Implementation notes

- Planning discipline: distinguish current / accepted-but-unimplemented / this-round delivery in every status line; record assumed-pending decisions (146/150/152/155 qualification) rather than claiming them. Claims above are estimates to recheck against code at dispatch (CodeGraph first).

## Verification

- This ticket is the verification: gates above + publish receipts. No application behavior changed in this planning session beyond what 182–184 deliver.

## Results — 2026-09-13 (combined verification + integration)

Combined suites (this unit, `C:\Sprout`):
- `cargo check` (src-tauri): clean — `Finished dev profile`, 0 warnings in the
  check profile (77 `tempfile::into_path` deprecation warnings appear only
  under `cargo test` lib-test profile; pre-existing, untouched by this round).
- `cargo test`: 620 passed / 0 failed / 3 ignored (2 device probes + 1 Store
  diagnostic, all explicit-run only). Includes 13 presence tests
  (`consecutive_sets_reuse_the_original_start_when_text_changes`,
  `only_text_and_start_ever_cross_the_seam`, silent-absent, exit-clear),
  clarify consensus tests
  (`answered_aspect_never_repeats_but_distinct_aspect_still_asks`,
  `answered_clarifications_regenerate_without_re_asking`,
  `vague_requests_clarify_with_pickable_choices_and_no_draft`,
  `output_vagueness_honors_answered_aspects`,
  `eval_fixtures_reach_their_expected_verdicts` over
  `src-tauri/tests/ai-eval-fixtures.json`).
- `npm.cmd run check`: 0 errors, 2 pre-existing warnings in untouched
  `QuickActionFormDialog.svelte:118` (`aiReady`/`action` initial-capture —
  intentional per the reset-effect comment, noted in 182 follow-up).
- `npm.cmd run test` (vitest): 21 files / 251 passed.
- `npm.cmd run build`: succeeds — SSR + client bundles built, site written to
  `build/` (same 2 svelte warnings only).
- Packaging: pinned skills + catalog served without checkout —
  `include_str!("../resources/ai-skills/shared-rules.md")`,
  `include_str!("../resources/ai-skills/create-quick-action.md")` in
  `ai_assist.rs`, `include_str!("../resources/ai-model-recommendations.json")`
  in `ai_managed.rs`; `NOTICES.md` retained; no remote/custom skill path.
- Size budget (NFR-43): only new crate is `discord-rich-presence` v1.1.0; all
  transitive deps already in-tree per `Cargo.toml` comment — exe delta is the
  crate itself, negligible. No release binary built in this unit (no `tauri
  build`, so no cleanup trigger).

Integration (coordinator-owned):
- 183↔184 handoff verified at the seam: backend `ClarifyAspect` slugs
  (`unknown-folder`, `ambiguous-app`, `unknown-prerequisite`,
  `scope-uncertain`, `bypass-uncertain`) cross as `aspect?: string | null` in
  `AiDraftOutcome::Clarify`; dialog chains `clarified choice [aspect]: …`
  (legacy bare tag honored), `draft-anyway:` override, aspect indicator
  `Question N — each question is asked once`, Continue / Draft anyway /
  Dismiss; backend `AnsweredAspects::parse` honors answered-once +
  distinct-still-asks + bogus-silences-nothing; refusal and
  shell-compatibility guard every draft. No contract change
  (`aiGenerateDraft` signature unchanged).
- Backup/size overlap: none — presence is a hardcoded constant (not
  Settings/backed-up/exported), dialog view state is transient, skills are
  bundled resources; `backup.rs` untouched by this round (roundtrip + dock +
  pre-action tests green).
- 172/167 dialog-copy reconciliation: AI view holds one textarea + Generate +
  outcome + quiet Dismiss only — no shell picker, no find/roots chrome up
  front; discovery appears only when a clarification needs it; tokens +
  Dialog/Button/Notice/InfoTip/Select/TestResult only, single size source
  untouched, no removed hint re-added.

Docs reconciled (this unit, no original-text rewrite):
- `docs/CONTEXT.md`: added delivered `AI Mode`, `Clarification`, `Presence
  activity` (no `planned` qualifier); `Model recommendation` / `AI skill`
  stay planned with spec-145 link (146/150–152/155 open).
- ADR-0028: status pointer → `amended 2026-09-13`, delivery amendment
  appended (183 shipped + 183↔184 handoff + research updates).
- ADR-0032: status pointer → `amended 2026-09-13`, round-delivery amendment
  appended (184 shipped + dialog handoff + fixture/packaging evidence);
  broader qualification stays open.
- ADR-0033: status pointer → `amended 2026-09-13`, round-delivery amendment
  appended (182 + session-clock/section-text follow-up, size + live evidence).
- `docs/adr/README.md`: 32 → 33 decisions; 0028/0032 pointers updated; 0033
  entry added.
- Research 0006: dated 2026-09-13 decision update (stacked hero → two-view);
  patterns 1–12 untouched. Research 0019: dated 2026-09-13 decision update
  (stacking → exclusive views + aspect-keyed clarify); inference untouched.
- Spec 181: status → implemented 2026-09-13; all acceptance boxes checked
  with per-ticket evidence links.

Manual matrices (automated evidence + code inspection; prior live confirms noted):
- Presence: Discord running → static details/state visible (user-verified
  live 2026-09-12; deterministic
  `discord_running_sets_the_static_pair_then_clears_on_stop` green).
  Discord closed/absent → normal start/run, no toast/dialog/error, debug log
  only + backoff retries
  (`discord_absent_retries_with_backoff_and_never_sets` green). Tray-only
  boot included (`presence::start()` in setup, never blocks). Actual exit →
  clear/close (`shutdown_path_clears_and_closes` green; close-to-tray does
  not clear — no hook in the close path). Elapsed clock survives reconnects
  (`consecutive_sets…`, `reconnect_after_startup_flaps…` green).
- Dialog: `!aiReady` Add/Edit = byte-for-byte manual form, zero AI nodes
  (gating tests green); Add+ready defaults AI hero-only, Edit+ready defaults
  Manual with tab strip (defaults tests green); flips preserve typing both
  ways, no validation on flip (flip tests green); `Use this draft` →
  shell+command applied, Manual flip, Command focus, `Applied — review and
  save` announce (apply tests green); discard/cancel/fail leaves the record
  intact; recheck hatch with manual-save escape (backend `recheck_candidate`
  tests green); Ctrl/Cmd+Enter generates vs submits correctly (shortcut tests
  green); tabs/Generate/outcome/choices/Dismiss reachable + named, arrow-key
  tablist + focus moves (a11y tests green). Light/dark + real DPI: no new
  colors/tokens/dimensions (one viewtabs style block + removed dead hero CSS
  only) — inherits the token system; dedicated runtime DPI sweep left to the
  user on real hardware.
- Clarify: unknown-folder / ambiguous-app / unknown-prerequisite fixtures
  return `Clarify` with 2–4 choices + free-text slot and no usable draft in
  both shells (`vague_requests…` green); answering regenerates, answered
  never repeats, distinct still asks, bogus keys silence nothing, Draft
  anyway proceeds past vagueness while refusal/shell-compat still win
  (consensus tests green); no disclosure widening (cloud re-ask + bounded
  opaque discovery unchanged, existing grant/redirect/binding tests green).

Gates + publish:
- `node tools/ownership-gate.mjs`: pass (62 owned references).
- Verified `-Up` 2026-09-13: first pass 9 copied (181, 185, CONTEXT, ADR
  0028/0032/0033 + README, research 0006/0019), 0 SHARE-NEWER; second pass 0
  copied — in sync.
- Cross-ticket regression: full suites above run with presence + dialog +
  skill co-installed (no isolated per-ticket runs); 0 failures.
