# 206 - Mac Quick-access port, portable Python shell, and dual-train releases (spec)

**Status:** design accepted in grill (2026-09-16); tickets 207–218 published `ready-for-agent` (2026-09-16). L tickets split per review (208→208/209/210, 209→211/212/213).

## Verified baseline

Source-verified via CodeGraph (2026-09-16), not prose:

- Current app is Windows-only: `bundle.targets: ["nsis"]`, unconditional `winreg` / `windows-sys` / `winvd` / `webview2-com` deps, `windows_execution/` single owner (`powershell_argv` / `cmd_argv` / `action_argv`), `winget/` facade + `Add-AppxPackage` bootstrap, Win32 AppBar dock, Run-key autostart. Excluding shell features alone does not make a Mac build.
- Release flow (`docs/release/release-process.md`): single `Cargo.toml` version, bare `v*` tag must equal it, CI builds `windows-latest` only, publishes `Sprout_*_x64-setup.exe` + `.sig`; updater matches that asset pattern (ADR-0012).
- Companion picker: label-plus-chevron over saved sites accepted (ADR-0022 amendment 2026-09-08); single-live-page lifecycle accepted (2026-09-09); dock width 340 physical px. No picker cap exists today.
- Accepted-but-unimplemented (assumed, not re-litigated): CMD shell extension + shell-aware backup v2 (spec 145 / ticket 147, ADR-0014 amendment 2026-09-06, ADR-0017 amendment 2026-09-06); per-site UA/zoom (156-round); picker delivery (170); AI authoring 145–155.

## Problem Statement

Windows users have the full Sprout installer app. Mac users have nothing, and the Quick Action command model (`powershell`/`cmd`) cannot travel across OSes. Meanwhile the dock Companion picker has no cap, so a long saved-site list becomes a slow truncated dropdown in a 340 px strip. Releases assume one Windows artifact per version, so a Mac train has nowhere to live.

## Solution

Ship a Quick-access-only Mac app from the same repo and release process: no Product / Preset / Plan / winget / Run on Mac — Launch entries, Quick Actions, Clips, Companion + dock only, reusing the Svelte component/token layer with a new Mac execution owner underneath. Make `python3` the portable Quick Action shell implemented on both OSes (`zsh`/`sh` on Mac; `powershell`/`cmd` stay on Windows). Cap the dock Companion picker at 5 pinned sites with management in the main app. Move releases to namespaced dual trains (`win-v*` / `mac-v*`) so either platform can ship alone or both can ship together.

## User Stories

1. As a Mac user, I want a Sprout dock with my Launch entries, so that I get one-click starts like Windows users.
2. As a Mac user, I want to author Quick Actions in `zsh`, so that my everyday shell one-liners run natively with no install.
3. As a Mac user, I want POSIX `sh` actions to run too, so that shared snippets work without rewriting.
4. As a Mac user, I want missing-`python3` failures to tell me how to install it, so that I am never left with a silent failure.
5. As a Windows user, I want a `python3` shell choice alongside PowerShell/cmd, so that I can write actions once and run them on both OSes.
6. As a user with both machines, I want `python3` actions to restore on either OS, so that my backup is portable.
7. As a user importing a foreign-shell action, I want an honest incompatible-shell message with a re-author hint, so that nothing ever mis-executes under the wrong shell.
8. As a dock user with many saved sites, I want the picker to show my 5 pinned sites, so that every pick stays one click with no truncation.
9. As a dock user with 5+ sites, I want a `Manage in Sprout…` row, so that I know where the rest live.
10. As a main-app user, I want the Companion manager to stay unlimited (with a filter once large), so that the cap never deletes my sites.
11. As a Windows user, I want zero behavior/size regression from the Mac work, so that the <10 MB budget and all current flows hold.
12. As a release manager, I want to tag `mac-v0.x.y` alone for a Mac-only fix, so that Windows users see no phantom update.
13. As a release manager, I want to tag `win-vA.B.C` + `mac-vX.Y.Z` together for a shared fix, so that both platforms ship from one commit.
14. As an updater user on either OS, I want only my platform's releases offered, so that I never download the wrong installer.

## Implementation Decisions

- **Mac v1 scope is Quick-access-only, compiled out.** Product/Preset/Plan routes, winget, Plan/Run backends are absent from the Mac bundle — not hidden nav. Shared layer is components, design tokens, API types only.
- **Single repo, seam-gated — not two folders.** Extend the existing `PlatformEngine` seam with a Mac adapter; Windows owners (`windows_execution/`, `winget/`, `engine/windows/`) become `cfg(windows)`; new Mac owner(s) are `cfg(macos)`. Windows-only crates move under target-gated deps so neither binary links the other's OS libs. Frontend excludes installer routes behind a Mac build flag so Vite drops them from the Mac JS.
- **Shell matrix.** Windows: `powershell` / `cmd` / `python3`. Mac: `zsh` / `sh` / `python3`. `python3` is the only roaming shell, served by one cross-platform owner (ADR-0029 applied to a portable runtime, not a per-OS copy). No auto-translation, ever — foreign shells reject honestly with a re-author pointer.
- **Python discovery.** Windows probes `py -3` first, falls back to `python` on PATH; Mac probes `python3`. Same hidden/unelevated/tracked/stoppable/logged model as ADR-0017; missing runtime is a first-class honest outcome in Test/Run. Extends ticket 147's shell work and spec-145 AI qualification to a third shell.
- **Backup evolution.** The shell-aware v2 line (147 / ADR-0014 amendment) advances to admit `python3`; legacy readers keep strict-reject on unknown shells/versions. One document format throughout (ADR-0014); portable-form stripping unchanged (ADR-0009); machine-local boundary unchanged (ADR-0026).
- **Companion cap per research 0024.** Dock picker = first-5 pinned in user order + `Manage in Sprout…`; no dock search/lazy-load (0004:3). Manager stays unlimited; filter/virtualization there only if lists approach ~15. Refines the ADR-0022 picker, no ADR reversal.
- **Dual-train releases (multi-tag).** Tags `win-v*` / `mac-v*` replace bare `v*`; versions split (workspace: existing Windows package keeps its version, new Mac package owns its `0.x` line). Either tag alone ships one platform; both tags on one SHA ship both (shared fix = bump both versions in one commit, push both tags, two workflows, two Releases). Updater matches per-platform prefix + asset (`*_x64-setup.exe` vs Mac `.dmg`); each side ignores the other's train. Requires an ADR-0012 amendment + `release-process.md` rewrite; bare-`v*` gate retires.
- **Size discipline.** Windows NFR-43 budget holds; new Mac budget set in the release ticket. Ownership gate extended to fail cross-OS references; CI asserts per-platform binary + JS bundle sizes.

## Seams for testing (highest first)

1. `PlatformEngine` seam (existing, ADR-0004) — Mac adapter vs test adapter; preferred seam for all executor work.
2. Shell `argv`/probe seam (execution owner boundary) — `python3` discovery tables driven without spawning.
3. Backup `inspect`/`import` seam — version/shell matrix as documents, not disk state.
4. Updater feed filter — tag/asset tables, no network.
5. Dock picker — pinned-5 + manage row over a site list model (existing `normalizeCompanionSites` + picker tests as prior art).

## Testing Decisions

- Test external behavior at the seam, never OS internals: outcomes, honest errors, and document verdicts — not spawned processes or WebView pixels.
- Prior art: `ai_assist` prompt/fixture battery, `run.rs` FakeEngine matrix, `companionPicker` + `normalizeCompanionSites` unit tests, `import/export` doc tests.
- Each ticket pins its failing-first test (contract) before the implementation; the release ticket runs both installers + updater-matrix + size-budget checks on real runners.

## Ticket map and integration ownership

| Ticket | Behavioral prerequisites | Likely paths / owners | Shared contract and integration edits | Candidate wave | Effort |
| --- | --- | --- | --- | --- | --- |
| 207 - Dual-train versions, tags, CI, updater feeds | None (starts now) | Cargo workspace versions; release workflow; update feed filter; release-process.md; ADR-0012 amendment | Tag/version contract (`win-v*`/`mac-v*`, asset patterns, feed rule); retires bare-`v*` gate | 1 | M (~2–3 sessions) |
| 208 - Seam contract: PlatformEngine Mac adapter interface + owner map | 207 version contract (which package builds which tag) | PlatformEngine seam; test adapter; owner inventory (no code moves) | Small interface (execute/detect/stop intent in, outcome out), deep behind it; settles the contract 209–214 build against; `shared/` rejected here (single Mac adapter = hypothetical seam until 211/214 prove two) | 1 | S (~1 session) |
| 209 - Dep cfg-gating + owner relocation (exclusion) | 208 interface (what gets gated, not how) | windows_execution + winget + engine/windows under `cfg(windows)`; empty mac_execution stub under `cfg(macos)`; Windows-only crates under target-gated deps | Exclusion proof: Windows binary contains zero Mac bytes and vice versa (per-target check + size diff vs baseline); owner map updated per ADR-0029; deletion test: deleting an owner re-scatters its invocation knowledge across callers | 1 | M (~2 sessions) |
| 210 - Gate + size harness | 208 owner map | Ownership-gate extension (cross-OS reference = fail); per-platform binary + JS bundle asserts | One gate command, deep coverage (every call site); guards all later tickets | 1 | S (~1 session) |
| 211 - Mac shell argv/probe module (zsh/sh) | 208 interface; 209 exclusion in place | mac_execution (sole Mac argv owner); probe tables; reuse of timed-process shape via the seam, never a copy | Depth: one `argv(shell, command)` + `probe()` interface hides quoting/PATH/version parsing; deletion test passes (removing it re-scatters shell knowledge); `cfg(macos)`-only so Windows never compiles it | 2 | M (~2 sessions) |
| 212 - Mac run/stop/tracking parity | 211 argv/probe contract | Same mac_execution module (no new module — internal seams only); reaper + watchdog via the shared tracking interface | Reuse, not duplication: foreground-only tracking, stop-command-or-kill-tree, logging per ADR-0017; tested through the 208 seam | 2 | M (~2 sessions) |
| 213 - Shared python owner (both OSes) | 208 interface; 211 probe pattern as prior art | One cross-platform python module: Windows adapter (`py -3` → `python`) + Mac adapter (`python3`); qualifies for shared placement (genuine second adapter + version-parse complexity hidden) | Single `python_argv` + `probe_python` interface serves 214 and Mac callers; missing runtime = honest outcome on both | 2 | S–M (~1–2 sessions) |
| 214 - Windows python3 shell | 213 shared interface; 147 shell field shape | Cross-platform python owner (caller only); backup allowlist; Test/Run UX | No new invocation site (ADR-0029); extends 147 to a third shell | 2–3 (parallel with 211–212 after 208) | M (~2–3 sessions) |
| 215 - Frontend Mac build flag (route exclusion) | 208 (which routes exist per platform) | Installer routes; shared components/tokens; build flag; bundle-size check | Excluded-route list; shared UI stays common; Vite drops installer routes from Mac JS | 2 | M (~2 sessions) |
| 216 - Cross-shell import honesty + backup compat | 211 + 213 + 214 shell verdicts | Backup inspect/import; shell allowlist v3; incompatible-shell UX copy | Doc-version matrix; no auto-translate, ever | 3 | S–M (~1–2 sessions) |
| 217 - Pinned-5 Companion picker | None (independent; research 0024) | Dock picker; Companion manager pin/reorder; normalizeCompanionSites | Pinned-5 + manage-row contract; manager unlimited | 1 (independent) | S (~1 session) |
| 218 - Release + verification round (both installers) | All above | Both runners; size budgets; updater matrix; docs/ADR close-out | Integration owner: combined green, published docs, verified syncs | 4 (last) | M (~2 sessions) |

Integration owner is 218: owns the workspace/tag/feed contract reconciliation, parent status, overlapping doc edits, and final combined verification. Claims above are dispatch-time estimates to recheck against code, not file locks. If run as a concurrent batch, follow `parallel-tickets.md` with one coordinator; no parallel implementation runs during this publication session. Total: **≈18–25 sessions** across 12 tickets (2×S, 2×S–M, 5×M, 3×M-or-smaller core); wall-clock compresses where waves allow (207+208+209+210+217 → 211+212+213+214+215 → 216 → 218). Slicing adds ~2 sessions of contract overhead versus the old 2×L blobs and removes the unreviewable-big-ticket risk. Reuse/exclusion rules per ticket: one owner per OS command (ADR-0029), shared placement only with a genuine second adapter, `cfg`/target-gating plus gate + size asserts so unneeded code never compiles in.

## Out of Scope

- Auto-translating `powershell`/`cmd` ↔ `zsh`/`sh` (rejected; no library guarantees it).
- Porting winget/Product/Preset/Plan/Run to Mac (v1 is Quick-access-only by decision).
- Multi-tab/omnibox/browser chrome in Companion; floating Companion (ADR-0022 rejections stand).
- Bundling a Python runtime with either installer (system runtimes only).
- Converging `win-`/`mac-` version numbers (independent until any parity decision).

## Further Notes

- Multi-tag mechanics confirmed 2026-09-16: two tags may point at one SHA; shared fix = bump both versions, push both tags, two workflows, two Releases.
- CONTEXT.md stays untouched this round (no new glossary term beyond accepted `Companion site`, `Quick Action`, `Prerequisite` usage); `python3` shell and `win-v`/`mac-v` trains enter glossary only when their tickets land.
- Research 0024 is the picker evidence; no new ADR except the 0012 dual-train amendment inside 207/214.
