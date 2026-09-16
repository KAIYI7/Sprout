# 205 — Companion picker cap, Python detection + shell, file Download, staging grace (spec)

**Status:** planning package only — no application behavior changed here. Tickets 206–211 `ready-for-agent` (created next in this unit).

**Parents / related (read before implementing):**
- [166 field cleanup / dock filter / companion navigation](166-field-cleanup-dock-filter-companion-navigation-spec.md) + [170 site picker](170-companion-saved-site-picker.md) — dock picker is an ordered saved-site menu with active-mark, single-site plain label, main-app authoring; 170 automated validation complete, native matrix pending. This round caps the picker, changing no lifecycle, isolation, or docked-only visibility.
- [171 navigation investigation](171-companion-navigation-failure-investigation.md) — stays investigation-only; no routing/new-window policy in this round.
- [173 files + placeholder round](173-quick-actions-files-clips-logs-companion-spec.md) + [179/180 files backend/frontend] — `quick_action_files(action_id FK CASCADE, filename, bytes)` table, 5MB/file + 20MB/action caps, basename-only names, `<FilesDir>` one-pass expansion shell-quoted by the `windows_execution` owner, per-run staged dir with conditional handoff grace, inert-without-placeholder. This round adds Download and repairs the grace hole; staging/quote ownership unchanged.
- [198 prereq + offload + motion](198-prerequisite-offload-motion-spec.md) + [199 detect backend](199-prerequisite-detect-backend.md, complete) + [200 skill/dialog surfacing](200-prerequisite-skill-dialog-surfacing.md) — read-only `detect_prerequisites` under the process/shell owner, closed catalog, verdicts `present | not-found | not-verifiable`, 15s/probe + 60s total budgets. This round extends the catalog with the `py` launcher; detect/read-only boundaries unchanged.
- [145 AI authoring](145-ai-assisted-quick-action-authoring-spec.md) — AI qualification for any new shell follows the spec-145 pattern separately; out of scope here.
- [147 shells + backup compat](147-quick-action-shells-and-backup-compatibility.md, implemented) — explicit shell field, legacy = PowerShell, shell-aware identity + backup envelope v2. The Python shell ticket follows this shape as prior art; no new backup format.
- ADRs: 0014 (one evolving backup document), 0017 (hidden/unelevated/stoppable/tracked runs), 0022 (single isolated docked Companion), 0026 (machine-local + merge-by-identity + ordered lists), 0029 (one Windows-invocation owner). No ADR text changes in this round; where behavior moves (grace lifetime), the owning ticket records the rationale and amends only if a decision boundary moves.
- Research: 0004 (frequency split, two disclosure levels, feedback), 0005 (PageHeader, one primary, toolbar search/filter), 0006 (visibility-on-surface pattern 1, near-object controls pattern 4, content-gated Favorites pattern 11), 0008 (classify knobs before placing). NEW in this unit: [0024 Companion site picker limit](../../../docs/research/0024-companion-site-limit.md) — pinned-5 evidence (Miller/Cowan, Hick's law, 340px dock, Apple popover, MS Settings guidance, Notion Favorites, WAI-APG menu-button).
- Glossary: `docs/CONTEXT.md` gains **Quick Action file** + **`<FilesDir>`** in this unit (grill Q11); `site dropdown` unqualified means the dock Companion picker.

## Problem Statement

Three papercuts, one failure mode. The dock Companion picker renders every saved site with no bound, so it stops being a fast palette as lists grow. Python — the most common Quick Action dependency — is invisible to prerequisite detection under its launcher name (`py`), and there is no native Python shell, so users hand-roll `python script.py` inside cmd/PowerShell with no verified signal. Attached action files can be staged into a run but never downloaded back out, and opening them straight from the per-run temp dir is racy: `cmd start` without an empty title eats the quoted path, and the staged dir can be deleted before a handed-off viewer opens it (pdf popup, mp3 access error).

## Solution

Cap the dock picker at the first 5 sites in user order plus a `Manage in Sprout…` row; keep Settings uncapped and the manager unlimited (filter deferred to ~15+). Extend prerequisite detection with launcher-first Python probing; add a native `python3` shell on the 147 shape. Add per-file Download plus Download-all-zip for persisted action files, byte-identical. Make the staging handoff grace unconditional so handed-off opens always land. Document the `start ""` / `Start-Process` / `explorer` forms and lint the cmd title-trap in the dialog.

## User Stories

1. As a Companion user with 6+ saved sites, I want the dock picker to stay a one-glance menu of my first 5 plus a management row, so every pick stays fast.
2. As a site collector, I want the main-app manager to keep all my sites with no cap, so the dock cap never deletes or hides my data.
3. As a Settings user, I want the Active-site control to keep showing every site, so the dock cap never constrains full-surface configuration.
4. As a Python requester, I want `py`/`python` presence and version detected read-only, so drafts and editor hints stop guessing about my runtime.
5. As a Quick Action author, I want a native `python3` shell choice with honest missing-runtime errors, so I stop wrapping `python script.py` in cmd.
6. As an action owner with attached pdf/mp3/data files, I want per-file Download and Download-all-zip from the main app, so I can get my exact bytes back (mp3 to mp3, pdf to pdf).
7. As a cmd author opening an attached file, I want the dialog to warn me when `start` will eat my quoted path, so `start "" …` works first time.
8. As a viewer/mp3 opener, I want the staged file to survive until my app has it, so cold-start opens never hit a deleted temp path.
9. As a privacy-conscious user, I want detection to stay read-only with no Python vendored into Sprout, so runtime updates can never break the app.

## Shared understanding (grill close-out, Q1–Q11 + bug triage)

1. **Picker cap (Q1/Q5/Q6):** first-5 in user order + `Manage in Sprout…` row opening the `/companion` manager. No dock search, no lazy-load-in-dock, no hard total cap. Settings `Select` stays uncapped; main-app filter + virtualization deferred to ~15+. Evidence: research 0024 (this unit).
2. **Python phased (Q2/Q7/Q8):** Phase-1 is a catalog extension (`py` allow-list + `route()`, probe order `py -3`, then `python`, then `python3`, same 15s/60s budgets, existing install guidance). Phase-2 is a native `python3` shell (`python_argv` inside `windows_execution`, same hidden/unelevated/tracked/logged/stop-same-shell behavior, follows 147 field/backup shape). Sprout never vendors Python; updates change only the reported version token.
3. **Download (Q3/Q9/Q10):** persisted rows only (Add-dialog staged files need nothing); byte-identical round-trip; attach-time caps unchanged; new bytes command in the `quick_actions` owner (list stays meta-only); per-file Download in edit dialog + row menu, `Download all (.zip)` iff 2 or more files, main app only, OS Save-As wins, backup format unchanged.
4. **Authoring guidance:** cmd `start` needs `start "" <FilesDir>\file` (title-trap); powershell uses `Start-Process <FilesDir>\file`; `explorer <FilesDir>\file` works in both. Dialog lints a bare `start` under cmd.
5. **Staging grace (authorized):** unconditional short grace — release always lingers past shell exit even with no observed children (repairs the `release_staged_dir_with_grace` TOCTOU hole behind the pdf-popup/mp3-access failures). Existing post-drain handoff retained.
6. **Mp3 `Invoke-Item`:** not Sprout permission gating (strings absent from source; runs as current user per ADR-0017; temp ACL inherited). Same lifetime race or Store-app sandbox; Download cures the class.

## Current vs accepted vs proposed

- Current (verified via CodeGraph/source, not prose): picker renders full `companionUrlList` with no limit/search; shells `powershell|cmd` only (147); files are DB blobs surfaced via staging + backup only, no bytes command; `release_staged_dir_with_grace` graces only when `children_alive(shell_pid)`; detect catalog probes `python|python3`, no `py`, no launcher.
- Accepted-but-unimplemented assumed: 147 shell-aware identity + backup v2 (implemented, awaiting publish — python shell reuses, never redoes); 167 copy rules; 170 native matrix; 171 investigation.
- Proposed here: items 1–6 above. No elevation/tracking/backup-format change, no second invocation site (ADR-0029), no tabs/omnibox/bridge/floating Companion, no AI qualification for the new shell, no main-app filter work until ~15+.

## Seams (for implementer + reviewer confirmation)

One owner per area, existing seams preferred: `windows_execution/` (probe catalog + `python_argv` + staging/grace — extended, never duplicated); `quick_actions` (bytes command); `QuickActionFormDialog.svelte` + quick-actions page (Download UI + lint); dock picker in the Quick Launch window (cap render only). No new seams, no `shared/` module (no second adapter).

## Design rules every implementer applies

- **Codebase-design vocabulary (docs/agents/conventions.md):** deep modules at clean seams; deletion test; extend the Windows-invocation owner, never a second site; ownership gate must pass.
- **UI/UX (docs/agents/ui-ux.md):** reuse tokens from `src/lib/styles/tokens.css` and components from `src/lib/components/`; no ad-hoc styling — deviation goes in the ticket for review. Cite the applied research rule per ticket (0004:1–3, 0005:4, 0006:1/4/11, 0008:1, 0024). New UI evidence lands as a note extension or new numbered note — no taste-only UI.
- **Planning (docs/agents/planning.md):** distinguish current/accepted/proposed per ticket; record assumed pendings (147 publish, 170 native matrix, 171); no silent ADR overwrite.

## Ticket map and integration ownership

| Ticket | Behavioral prerequisites | Likely paths / owner symbols | Shared contract + integration edits | Candidate wave |
| --- | --- | --- | --- | --- |
| [206 Picker cap](206-companion-picker-cap-first-five.md) | 170 delivered lifecycle | dock picker render, `/companion` manager row target | First-5 user order + manage row; active-mark; single-site label untouched; no dock search | 1 — independent |
| [207 Prereq py extension](207-prerequisite-py-launcher-probe.md) | 199 verdict contract | `prereqs.rs` allow-list/route/probe order, `prereqGuidance.ts` | `py` routes to RuntimePython; order launcher then PATH; budgets/wording unchanged; supplies 208 | 1 — independent |
| [208 Python3 shell](208-quick-action-python3-shell.md) | 207 probe order; 147 field/backup shape | `python_argv` in owner, shell enum/CHECK, dialog shell choice, backup compat | Same hidden/unelevated/tracked/logged/stop; missing-runtime honesty; legacy rows untouched | 2 — after 207 |
| [209 Download backend](209-quick-action-file-download-backend.md) | 179 table/caps | bytes command in `quick_actions` owner, `api.ts`/`types.ts` | List stays meta-only; byte-identical; caps unchanged; supplies 210 | 1 — contract owner |
| [210 Download frontend + lint](210-quick-action-file-download-frontend-lint.md) | 209 bytes contract | `QuickActionFormDialog.svelte`, row menu, command editor | Per-file + zip-iff-2-or-more, Save-As wins, main-app only; `start` title-trap lint + copy per 167 | 2 — after 209 |
| [211 Staging grace fix](211-staged-dir-unconditional-grace.md) | 179 lifetime | `release_staged_dir_with_grace`, `children_alive` (read-only use) | Unconditional linger past shell exit; drain-then-handoff retained; unit tests via grace param | 1 — independent |

**Integration owner (spec-205 coordinator):** owns glossary/ADR/research reconciliation (0024 published here, CONTEXT entries landed here), 207 to 208 and 209 to 210 contract handoffs, 147-overlap coordination for 208, backup-merge overlap (none expected — format unchanged), and final combined verification. Workers update their own ACs/results, never rewrite global docs. Claims above are estimates to recheck against code at dispatch (CodeGraph first). If run as a concurrent batch, follow `docs/agents/parallel-tickets.md` with one coordinator.

## Out of Scope

AI qualification for the `python3` shell (spec-145 pattern, separate round); main-app site filter/virtualization (deferred to ~15+); tabs/omnibox/bridge/floating Companion; navigation/new-window policy (171); second backup format; any elevation/tracking change; dock dimension changes.

## Acceptance and verification

- [ ] User confirmed picker cap, phased Python, Download A+B, glossary text, unconditional grace (this grill).
- [ ] 206 verifies 0/1/5/6+ cases, active-mark, manage-row target, keyboard + narrow-dock behavior; frontend check clean.
- [ ] 207 verifies `py`/`python`/`python3` present/not-found/not-verifiable matrix incl. launcher-first order; backend prereq tests green.
- [ ] 208 verifies run/stop/test under `python3`, missing-runtime honesty, legacy rows + backup round-trip, ownership gate pass.
- [ ] 209+210 verify byte-identical single + zip-iff-2-or-more downloads, Save-As overwrite, dialog + row-menu surface, lint fires only on the cmd title-trap shape.
- [ ] 211 verifies handed-off opens survive cold start (pdf + audio), no behavior change for non-file runs, existing staging tests green.
- [ ] Coordinator records Rust/frontend checks, reconciles CONTEXT/ADR/research status, publishes each completed unit through the ownership gate + verified sync-up (twice, expect 0 copied).

No application behavior changed in this planning session.

## Further Notes

- to-spec seam check: seams above are all existing owners; reviewer confirms at dispatch.
- The mp3 `Invoke-Item` isolating test (manual copy to Documents) remains a useful pre-211 datapoint but gates nothing.
- Quarantined pre-sync planning artifacts (local-only 205–218 + 0024 draft + `src-tauri/mac` scaffold) are backed up at `Temp\opencode\sprout-phantoms-20260916`; 0024 is republished verbatim in this unit, the rest stay retired.