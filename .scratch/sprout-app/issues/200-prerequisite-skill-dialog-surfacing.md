# 200 — Prerequisite skill section + dialog surfacing (warn, never block)

**What to build:** The pinned skill learns prerequisites and the dialog shows verified results: drafts depending on X carry the check outcome in `assumptions`, vague requests clarify with grounded choices, and missing prerequisites render as warn-plus-install-guidance — saving is never blocked, nothing auto-installs.

**Blocked by:** [199 detect backend](199-prerequisite-detect-backend.md) (needs the verdict contract). Assumes the existing draft/clarify seam + 0018 fixture corpus. Makes no detector-owner edits (199 owns those).

**Status:** implementation-complete — awaiting round verification (203)

**Parent:** [198 — Prerequisite + offload + motion (spec)](198-prerequisite-offload-motion-spec.md). Appends to the ADR-0032 skill (dated Amendment in this unit if skill text changes); ADR-0030/0031 unchanged.

## ACs

- [x] Skill diff is append-only Prereqs section: target 5.1/CMD explicitly, disclose-or-detect unknown prerequisites, record them in `assumptions`; existing sections byte-identical, NOTICES retained, packaging unchanged.
- [x] `unknown-prerequisite` clarify choices surface detect results where available (`not-found for X` / `not-verifiable`); no usable draft leaks through clarify; answering regenerates.
- [x] Honest wording: offline/uncertain reads `Not verifiable` (e.g. offline model cannot search online) — never a fabricated version or path.
- [x] Warn-never-block: missing prereq shows concise guidance (constraint + install command, per 167) beside the draft; Save/Test/Run paths unchanged; no auto-install path exists.
- [x] Both shells covered; destructive/7-only fixtures keep existing refuse/clarify verdicts; no disclosure widening on cloud path.
- [x] Backend fixture battery + relevant frontend dialog tests + ownership gate green.

## Implementation notes

- Sources: 184 append-only pattern + `ai-eval-fixtures.json` style for new prereq fixtures; 175/176 warn-dialog as UX prior art. Claims above are estimates to recheck against code at dispatch (CodeGraph first).

## Verification

- New fixtures (present / not-found / not-verifiable × both shells) green; packaging check (installed app serves pinned skills without checkout); ownership gate before sync.

## Validation 2026-09-15 (single session)

- Skill: `## Prerequisites` appended to `create-quick-action.md`; prior sections byte-identical; `NOTICES.md`/`shared-rules.md` untouched; `include_str!` packaging unchanged. ADR-0032 carries the dated amendment.
- Dialog drives detection (199's handoff): `detectPrerequisites` runs on draft `assumptions` and on `unknown-prerequisite` clarify request text, through the closed v1 catalog only (`src/lib/prereqGuidance.ts`) — no detect call when nothing detectable is named, so unknown prose never becomes a verdict.
- Backend: 25 fixtures (3 new: module-cmd, sdk-ps, sdk-cmd) green; `cargo test ai_assist` 35/35; destructive/7-only verdicts unchanged; aspect-keyed grill intact.
- Frontend: `prereqGuidance.test.ts` 8/8; `prereqWarn.close.test.ts` 3/3 (warn+guidance, quiet-present, clarify surfacing with no draft leak, grill controls live); `npm run check` 0 errors (2 pre-existing warnings); ownership gate + full suites at round verification.
