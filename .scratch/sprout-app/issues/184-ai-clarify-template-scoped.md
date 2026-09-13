# 184 — AI clarify template, scoped-down grill (skill structure untouched)

**What to build:** The pinned `create-quick-action` skill gains a scoped clarification template — vague requests ask back with pickable choices instead of guessing — with zero change to the skill's tone, language, rules, or section structure.

**Blocked by:** None — can start immediately. Assumes the existing draft/clarify seam + 0018 fixture corpus; supplies 183/185. Makes no dialog-template edits (183 owns the view).

**Status:** done — all ACs checked 2026-09-12 (automated evidence green)

**Parent:** [181 — Discord + AI two-view + clarify (spec)](181-discord-presence-ai-two-view-clarify-spec.md). Extends [ADR-0032](../../../docs/adr/0032-model-recommendations-and-skills-ship-with-app.md) skill content (dated Amendment in this unit); ADR-0030/0031 boundaries unchanged.

## Scope

- Pinned skill resources + prompt assembly + request/output checks + deterministic fixtures only. Appends one **Clarification template** section to `create-quick-action` skill: when the request is vague (unknown path e.g. "Download folder", ambiguous app match, unknown prerequisite), return `Clarify` with 2–4 pickable choices plus a free-text slot — the grill-with-docs frontier question scoped to command generation.
- Enforcement stays app-side: request checks before inference, output checks before usability; `Clarify` carries no executable code; answering never widens disclosure (cloud re-asks); discovery stays bounded + locally bound via opaque references; shell inference validated by non-executing checks. No custom/editable/remote skills, no new execution/disclosure path.
- Explicitly not built: dialog rework (183), new provider/runtime support (146/150–152), diagnosis skill changes (153), persistent chat history, whole-disk indexing.

## ACs

- [x] Skill diff is append-only to the Clarification section: tone/language/rules/structure of existing sections byte-identical except the new template; NOTICES/attribution retained; bundled-resource packaging unchanged.
- [x] Vague fixtures (unknown folder, ambiguous app, unknown prerequisite) return `Clarify` with choices and expose no usable draft via preview/stream/Copy/Save; answering with a pick regenerates; unknown/stale/modified references rejected and rebound locally.
- [x] Both shells covered; destructive/7-only/missing-module fixtures keep existing refuse/clarify verdicts; edited-candidate recheck preserved; zero script-execution calls across generate/clarify/validate/cancel/save (managed startup + fixed read-only discovery distinguished, as before).
- [x] Cloud path: no request before consent, no raw discovery data without preview+grant, no destination/redirect bypass, no credential leakage (assert outbound payloads).
- [x] Relevant backend checks/tests + fixture battery green; `node tools/ownership-gate.mjs` passes.

## Implementation notes

- Sources: 0018 corpus + `ai-eval-fixtures.json` + shared-rules/diagnose skills as prior art; spec 145 testing decisions for seam choice (highest reusable seam, one deep module). Claims above are estimates to recheck against code at dispatch (CodeGraph first).

## Verification

- Fixture + combination + path-binding checks per spec 145 (malformed/truncated/unsupported/cancel/timeout/uncertain/refused, spaces/quotes/metachars/Unicode/dupes/missing/stale/reparse escapes); packaging check (installed app serves pinned skills without checkout); ownership gate before sync.

## Result — 2026-09-12 (implementation)

- Appended one Clarification template section to `src-tauri/resources/ai-skills/create-quick-action.md`; existing sections byte-identical, NOTICES retained, `include_str!` packaging unchanged.
- `DraftOutcome::Clarify` now carries `choices: Vec<String>` (serde default for backward compat); `RequestVerdict`/`OutputVerdict` carry choices app-side. `clarify_reason` covers unknown-folder (Downloads/Documents/Desktop without an exact path; exact `C:\`/`\\` paths never clarify), ambiguous-app, unknown-prerequisite, PS7-only, scope-uncertain, and bypass-uncertain — each with 2–4 safe picks, no executable code.
- Added `ambiguous-unknown-folder-ps` + `ambiguous-unknown-folder-cmd` fixtures (both shells) and `vague_requests_clarify_with_pickable_choices_and_no_draft` backend test (choices length, no code, both shells). Answering regenerates via the dialog's pick/free-text + Regenerate (183); unknown/stale/modified references still rejected/rebound in `ai_discovery` (existing tests green).
- Destructive/7-only/missing-module fixtures unchanged in verdict; `recheck_candidate` preserved; no execution added (generation/clarify/validate/cancel/save never run text). Cloud path untouched (grants none, no pre-consent request, no redirect follow, no credential path).
- `cargo test`: 613 passed, 0 failed. `node tools/ownership-gate.mjs`: pass.

## Fix — 2026-09-12 (answered-clarification protocol)

- Added the `clarified choice:` answer tag (`CLARIFIED_CHOICE_MARKER` in `ai_assist.rs`): `classify_request` stands the vague detectors down for one real attempt once the user answers; `check_output` takes a `clarified` flag with the same one-attempt rule while refusal and shell-compatibility always apply. `recheck_candidate` and discovery binding keep the full checks. Regression test `answered_clarifications_regenerate_without_re_asking` locks it: answers draft, refusal still wins, PS7-only output still clarifies. No Tauri contract change (`aiGenerateDraft` signature and `DraftOutcome` shape unchanged).

## Follow-up — 2026-09-13 (bounded grill consensus, no round cap)

Process note: appended here per owner rule — amend the existing clarify
ticket instead of opening a new one.

**Change:** each vagueness aspect asks at most once per generation thread;
answers chain (`clarified choice [aspect]: …`), an answered question never
repeats, a distinct one still asks back, and the user may draft anyway
(`draft-anyway:` override) — refusal and shell-compatibility still guard every
draft. Requires the dated `## Amendment — 2026-09-13` in ADR-0032 (same unit).

## ACs (follow-up — done 2026-09-13)

- [x] Answered aspect never re-fires though its trigger words remain; a second
  distinct aspect still clarifies with its own key; bogus keys silence nothing.
- [x] `draft-anyway` proceeds past vagueness at request stage; refusal still
  wins; shell-compatibility still clarifies with no aspect, answered or not.
- [x] Dialog chains answers onto the previous narrowed request (guarded against
  mid-grill request edits), echoes aspect keys, shows the question indicator,
  and offers Draft anyway; legacy bare tags still stand everything down.
- [x] `cargo check` 0 warnings, `cargo test` green, `npm.cmd run check` 0
  errors, vitest green, ownership gate pass.

## Results (follow-up 2026-09-13)

- `ai_assist.rs` (same single check seam, extended interface): `ClarifyAspect`
  (5 slugs) + `AnsweredAspects` (stateless chain parsing: aspect tags, legacy
  bare tag, `draft-anyway` override); `clarify_reason` split into always-on
  `shell_request_hint` and stand-down-aware `vague_aspect`; `RequestVerdict` /
  `OutputVerdict` / `DraftOutcome::Clarify` carry optional aspect (serde
  backward compatible). `recheck_candidate` and discovery binding use the
  unanswered checks, unchanged in behavior.
- Dialog chains answers with aspect keys, shows `Question N — each question is
  asked once`, adds Draft anyway beside Continue/Dismiss; `AiDraftOutcome`
  gains optional `aspect`; skill template gains the one-shot wording.
- Decision: dated `## Amendment — 2026-09-13` in ADR-0032.
- Verification: `cargo check` 0 warnings (77 lib-test warnings are the
  pre-existing `tempfile::into_path` deprecation, untouched by this unit),
  `cargo test` 620 passed / 0 failed (4 new consensus tests),
  `npm.cmd run check` 0 errors (2 pre-existing warnings in untouched dialog),
  vitest 251 passed, ownership gate pass.
