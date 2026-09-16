# 205 — Prerequisite true-kind glossary (append-only skill fix)

**What to build:** Close the fuzzy-language hole that let a live draft assume a `PowerShell module 'playwright'` that does not exist: one appended true-kind paragraph in the create skill's Prerequisites section, enforced by a prompt-content test plus one eval fixture, with a dated ADR-0032 amendment. Append-only — existing skill sections stay byte-identical so ticket 200's verification basis holds.

**Blocked by:** None — can start immediately. Follows the 200 append-only pattern; covered by [203](203-round-verification-integration.md)'s round (203 AC "ADR-0032 amendment (if 200 changed skill text)" now also spans this unit — 203 itself is untouched by this ticket, coordinator reconciles).

**Status:** implementation-complete — awaiting round verification (203)

**Parent:** [198 — Prerequisite + offload + motion (spec)](198-prerequisite-offload-motion-spec.md). Amends [ADR-0032](../../../../docs/adr/0032-model-recommendations-and-skills-ship-with-app.md) (dated amendment in this unit). Leaves [200](200-prerequisite-skill-dialog-surfacing.md)'s delivered text and verdicts unchanged.

## ACs

- [x] Skill diff is one appended paragraph in `## Prerequisites` of `create-quick-action.md`: every prerequisite named by its true kind with the exact detector key where one exists, "module" elsewhere means one of those kinds, inventing a `PowerShell module 'X'` forbidden unless the user named one. Existing sections byte-identical, NOTICES retained, `include_str!` packaging unchanged.
- [x] Prompt-content test `prerequisite_kinds_rule_out_invented_powershell_modules` green in `ai_assist.rs` (pins both the prohibition and the Playwright true-kind line in the loaded prompt).
- [x] Eval fixture `allow-ps-playwright-open-url` added to `ai-eval-fixtures.json` and green in `eval_fixtures_reach_their_expected_verdicts` (pins allow-draft for the corrected request shape; wording pinned by the prompt-content test).
- [x] ADR-0032 dated amendment appended (2026-09-15 true-kind glossary); original plus prior amendment text untouched; Status pointer updated.
- [ ] Round verification (203): fixture battery + backend suite + ownership gate re-run green in the round; manual prereq matrix unaffected (no detector/behavior change — skill text only).

## Explicitly not built

- Revising the loose "module" wording on other skill lines or the `Prerequisite` entry in `docs/CONTEXT.md` (Scope B, declined — would break 200's append-only basis; revisit only with explicit re-validation of 200).
- Detector, dialog, or contract changes (199/200 behavior unchanged; no new Windows-invocation site — ADR-0029 holds).

## Verification

- `cargo test ai_assist` green (new test + fixture battery); `node tools/ownership-gate.mjs` pass; ownership gate before sync.
- Evidence: see Validation section below.

## Validation 2026-09-15 (single session)

- `cargo test ai_assist`: all pass, including new `prerequisite_kinds_rule_out_invented_powershell_modules` and `eval_fixtures_reach_their_expected_verdicts` with the added fixture.
- `node tools/ownership-gate.mjs`: pass (no new Windows ops — docs/skill/fixture/test only).
- 200's prior validation stands (no behavior change under it); 203's round re-covers the battery.
