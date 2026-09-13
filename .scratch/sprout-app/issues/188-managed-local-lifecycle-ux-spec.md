# 188 — Managed local lifecycle UX: Start/Stop, status, resources, switching, install progress, exe name, presence assets, motion (spec)

**Status:** ready-for-agent — planning package only, no application behavior changed here.

**Parents / related (read before implementing):**
- [145 AI-assisted Quick Action authoring](145-ai-assisted-quick-action-authoring-spec.md) + [151 managed lightweight setup](151-ai-managed-lightweight-local-setup.md) (complete) + [152 stronger model choice](152-ai-stronger-model-choice-and-release-recommendations.md) + [186 managed cleanup](186-ai-managed-local-cleanup.md) — own the managed install/lifecycle/selection contract this round extends. Reconcile with 152's explicit-switch + 186's Off-stops-server + keep/remove grammar; do not re-litigate them.
- [181 presence/two-view/clarify](181-discord-presence-ai-two-view-clarify-spec.md) + [182 presence static](182-discord-presence-offline-static.md) + [183 two-view dialog](183-quick-action-ai-two-view-dialog.md) — own `aiReady` gating, two-view tabs, static presence payload. This round keeps zero-chrome + content-gated strip; adds one plain-text pointer only.
- [166 field cleanup / dock filter](166-field-cleanup-dock-filter-companion-navigation-spec.md) + [167 selective guidance](167-selective-field-guidance-cleanup.md) — copy rules, Details disclosure ownership. Tier-first row copy extends 167 coverage; do not re-add removed hints.
- ADRs: 0028 (design system + disclosure), 0029 (one owner per Windows command), 0030 (draft-only), 0031 (providers + scoped context), 0032 (bundled recommendations + skills), 0033 (static presence), 0012 (self-update asset pattern). Where this round changes an accepted decision, a dated Amendment ships in the delivery ticket — no silent overwrite.

## Problem Statement

Managed local AI works but its lifecycle is invisible and its install flow loses state. There is no Start/Stop (only `Off`, which changes provider); no running indicator so users cannot tell whether Stop applies; no resource view for the owned runtime; no way to pick the active installed model (install/remove implicitly retargets); install progress is tab-local `$state` with no bar/% and vanishes on tab switch; a failed repeat install reports `An incomplete or incompatible installation already occupies this model revision` with no recovery action; the `sprout.exe` process name risks confusion with an unrelated Steam game; presence ships no brand art despite portal-ready PNGs; motion has no in-app control (OS query only) and context menus open instantly while Discord-familiar surfaces fade.

## Solution

Keep the lazy lifecycle (stopped until Generate, 5-min idle reap, Off stops + keeps downloads) and add the missing surface: explicit Start/Stop on the active model (never touches provider) with a cheap running indicator; lazy resource Details (polled only while disclosed); explicit Active-model radio with tier-first labels (full artifact one level down); app-level install progress (store + backend events, bar + % + bytes, cancel everywhere) with explicit Remove-and-retry recovery; exe-only rename to `sprout-windows-desktop.exe` (display/product/data/identifier unchanged); static presence assets (`rp_large`/`Sprout`, `rp_small`/`Ready`); a Settings → General Animation switch gating all motion including a new minimal menu fade; and one plain-text Quick Action empty-state pointer routing to the Settings page (expand AI group + focus, focus-ring only — no scroll lib, no pulse).

## User Stories

1. As a managed user, I want a Stop button on the running model so I free memory now without changing my provider, so re-enable is instant.
2. As a stopped user, I want a Start button so I pre-warm before Generate, so first draft is fast.
3. As a cautious user, I want a Running/Stopped indicator so I know which button applies, so I never guess.
4. As a hardware-aware user, I want RAM/CPU/uptime under Details so I see what the model costs, so I can stop it informed.
5. As an efficient user, I want zero polling cost until I open Details, so the app stays lean.
6. As a multi-model user, I want an Active-model radio so I choose Lightweight vs stronger explicitly, so installs never retarget silently.
7. As a non-technical user, I want tier names first (`Lightweight — 1.5B Q4`) with full artifact in Details, so I choose by cost not hash.
8. As an installer, I want progress with bar + % surviving tab switches, so I trust a 1 GB download.
9. As an interrupted installer, I want Cancel anywhere plus honest resume/Remove-and-retry, so failure is recoverable.
10. As a Steam user, I want the process named `sprout-windows-desktop.exe` so it never collides with the game `Sprout`, so task managers stay clear.
11. As an updater, I want self-update + CI + worker/autostart intact after the rename, so releases keep working.
12. As a Discord user, I want brand art + clean hover texts so presence looks finished, so friends see `Sprout` / `Ready`.
13. As a motion-sensitive user, I want one Animation switch so I kill all motion app-wide regardless of OS, so the app stays usable.
14. As a keyboard user, I want menus/dialogs to respect that switch plus OS reduced-motion, so nothing animates past my setting.
15. As an unconfigured drafter, I want `No managed model is on — Enable it` routing to Settings, so I discover setup without a teaser hero.
16. As a maintainer, I want all of the above through normal releases with no new Windows-invocation site, so ADR-0029 holds.

## Implementation Decisions

### Current vs accepted vs proposed

- Current (verified via CodeGraph/source): lazy `RunningRuntime` (single process, `begin_request`/`ensure_runtime`, 30s health, idle reap 5 min, `stop_for_disable` on provider change, `shutdown` on exit); `ai_stop_managed_runtime` backend-only, no frontend caller; `aiReady` = config-ready (`managed` + `ai_model` ∈ installed), not process-alive; no Active picker; install `$state`-local, no events, `HttpDownloader` counts bytes without emitting; `final_dir.exists()` guard with staging cleanup only; `[package] name = "sprout"`; presence details/state/timestamps only; motion tokens `--dur-fast 120ms / --dur 200ms / --ease-out`, instant accordion/menu, Dialog fade 140ms, OS media-query guard only.
- Accepted-but-unimplemented assumed: 152 explicit stronger-tier switch + recommendation/inventory split; 186 Off-stops + keep/remove + ownership shapes; 167 copy rules; 0014 Settings-single-route + no per-group deep routes until threshold.
- Proposed here: surface only — no lifecycle-policy, execution-model, backup-format, or catalog-qualification change. Three states stay: `Off` (persistent disable, keeps downloads), `Stop` (transient kill, config intact, Generate restarts), Active radio (exactly one installed model when any exist; last-remove forces `off` as today — no `None` mode per grill analysis).

### Start/Stop + status (ticket 189)

- Same control morphs Start↔Stop per `0006 pattern 6` Run-accent/Stop-danger + Quick Action precedent; Stop calls existing `ai_stop_managed_runtime`, never touches provider; Start pre-warms via `ensure_runtime` health path.
- Status line under `Managed local model` knob only when `managed` + active installed: green `Running — {Tier} · up {m}` + Stop (danger) vs gray `Stopped` + Start (accent). Static dot, no pulse (companion still-under-reduced-motion precedent).
- Cheap status poll (~5s) while Settings open; zero when closed. Extends `ai_managed_status` (running/active/uptime); owner stays `ai_managed.rs` (ADR-0029).

### Resource Details (ticket 190)

- `Disclosure` primitive only (ADR-0028; `0006 pattern 7` collapsible, `0004` progressive disclosure). Collapsed by default, tooltip-grade numbers (working set, CPU %, uptime, active tier/endpoint).
- Closed = zero cost (no command/interval); open + running = ~2s poll of a new `ai_managed_resource_usage` owned by `ai_managed.rs`; open + stopped = static `Stopped — will start on next Generate`. Closed cancels.

### Active radio + tier-first (ticket 191)

- Explicit `Active model` radio, Save-deferred like provider (ADR-0031 explicit selection); lists installed models only; Start/Stop acts on active. Idle switch stops old + starts new; busy switch (`active_requests>0`, different model) honestly refuses with existing `Another managed model…` error. No silent switch (ADR-0032).
- Row: `{Tier} — {params} · {size} · {RAM}` via frontend id→tier map (no catalog schema change); full `candidate_artifact`/source/revision/hash/license stays in Details dialog (existing). Blocked tier stays hidden until qualified.

### Empty-state pointer (ticket 192, blocked by 191 contract)

- Unready dialog keeps zero AI tabs/hero; at most one plain-text pointer (`No managed model is on right now — Enable it` / `Set up AI assistance in Settings`) doing `goto("/settings")` + existing `expandGroups(["ai"])` + `focus(provider)`. Focus-ring is the highlight — no smooth-scroll lib, no pulse (pulse = run-active; `0005` consistency; 0014 no `#hash` routes). `0004 rule 2` discoverability-home proviso satisfied without duplicating config (`0006 pattern 1`, `0004 rule 3`).

### Install progress + recovery (ticket 193)

- App-level Svelte store fed by backend Tauri events: phase (runtime vs model), bytes, %, cancellable from row + dialog. Survives tab switch; lost on reload with honest `Interrupted — safe to retry`. Bar + % + bytes per `0004 rule 5`; minimal-until-content (`0006`). Row-level compact bar + dialog full bar; `Downloader::fetch` gains progress callback/channel; single-flight `installing` stays in-memory.
- Guard-hit offers explicit `Remove that revision and retry` (calls `ai_remove_managed` for that id) + `Keep files`; never auto-deletes. Copy splits incomplete (re-readable as not-installed → retry offered) vs incompatible (needs Sprout update). Staging cleanup unchanged.

### Exe rename, exe-only (ticket 194)

- `[package] name → sprout-windows-desktop`; keep `productName=Sprout`, `identifier=com.sprout.app`, `%LOCALAPPDATA%\Sprout` data dir, tray/shortcut display `Sprout`, updater `SETUP_ASSET_PREFIX=Sprout_` + `release.yml` glob. Keep `[lib] name = sprout_lib`. Update `tools/repro-*` paths, worker/autostart/shell test strings; confirm `current_exe()` relaunch + NSIS `MAINBINARYNAME` migration + autostart Run rewrite. Full product rename explicitly excluded (orphans uninstall keys/shortcuts/Run, breaks published-asset matching → manual-reinstall cutover).

### Presence assets (ticket 195)

- `large_image=rp_large` text `Sprout` (identity, VS Code convention) + `small_image=rp_small` text `Ready` (availability). Static, ≤128 chars, no user data; complements all nine `state` lines without repeating; STE-clean (technical noun + approved adjective). `buttons/party/secrets/urls/activity_type` stay banned. Ships dated ADR-0033 amendment + shape-test update (`assets` now present). Manual prerequisite: upload both PNGs to portal app `1548219927264235580` → Rich Presence → Art Assets → Save before code lands.

### Animation toggle + menu motion (ticket 196)

- Settings → General switch beside Theme (app-global per `0008 rule 1`; switch not checkbox per `0008:92-98`; immediate, never dirty like Theme). New persisted Settings key + `settingsSearch` entry + store/`data-` hook forcing the reduce path regardless of OS. Default On. Scope: Dialog fade 140ms gap, new `ctx-menu/submenu opacity/scale var(--dur-fast) var(--ease-out)`, chevron rotate, packet entrance, three infinite pulses → frozen ring/dot (existing `QuickActionRunControl` precedent), future progress-bar width. Tokens-only, no new durations; `prefers-reduced-motion` still honored.

### Modules and test seams

- One managed seam (`ai_managed.rs`: status/usage/progress-events/install/remove/lifecycle) + Settings managed UI + Quick Action dialog readiness; one presence seam (`presence.rs` payload-exactness via fake IPC); one motion seam (tokens + `ContextMenu`/`Dialog`/`GroupAccordion` + Settings switch). Reuse `ai_generate_draft`, validation/save, backup-exclusion, update/release seams — no new dispatcher, no second Windows-invocation site, no shared pass-through module.

## Testing Decisions

- Observable behavior at seams with deterministic fakes; no prompt-string snapshots or source-text assertions.
- Managed: stop-keeps-provider + Generate-restarts; status reflects running/stopped; usage polls only while disclosed; radio retarget + idle-switch + busy-refuse; progress %/bytes/cancel across tab switch; guard-hit retry/keep paths; exe rename keeps update/worker/autostart green.
- Presence: payload-exactness with assets + hover strings; Discord-absent silent no-op; no token/user-ID; exit clears.
- Motion: switch On/Off flips all in-scope motion regardless of OS; OS reduce still honored; menus/dialogs instant when off.
- Prior art: `managedSettings.close.test.ts`, `aiDraftDialog`/`quickActionDialog` tests, `ai_assist`/`ai_managed` backend tests, presence fake-IPC tests, Settings dirty-guard tests, ownership gate + `npm.cmd run check` + `cargo check/test` + build.

## Out of Scope

- Dynamic presence (per-screen hover, counts, names), presence Settings toggle, buttons/party/secrets.
- Catalog re-qualification, stronger-tier pinning (152), cloud route (150/187), `None` active mode, per-product install dirs, auto-delete of occupying revisions, full product rename, per-group `/settings#` routes, smooth-scroll lib, new motion tokens/components.
- Runner/watchdog, backup-format, Preset/Quick Launch execution changes.

## Further Notes

- Research on delivery: extend `0006` (tier-first + pointer application) with dated update; no new numbered note unless a genuinely new topic emerges (animation-toggle placement already covered by 0008). Glossary: no new canonical terms; `Active model` is Settings-label for persisted `ai_model`, `Managed runtime status` is display for `RunningRuntime` — record usage in owning tickets, no CONTEXT edit this round.
- ADR-0033 amendment text is proposed below in ticket 195's ACs; the amendment itself ships with 195's delivery (same convention as 181→182). No other ADR text changes this round.
- Size budget NFR-43 holds; presence delta is two asset keys + strings.

## Ticket map and integration ownership

| Ticket | Behavioral prerequisites | Likely paths / owner symbols | Shared contract and integration edits | Candidate wave |
| --- | --- | --- | --- | --- |
| 189 Start/Stop + status | 151 lifecycle assumed; 186 Off-grammar assumed | Managed lifecycle owner + Settings managed knob; status extension | Settles running/stopped contract + Stop-without-provider-change; supplies 190/191 | 1 — managed owner |
| 190 Resource Details | 189 status contract | Same owner + Disclosure | Lazy poll contract (open-only); no status overlap | 1 — after 189 (same files) |
| 191 Active radio + tier-first | 189 contract; 152 explicit-switch assumed | Settings managed list; frontend tier map | Settles `aiReady = managed + active∈installed`; supplies 192 | 1 — after 189 (same files) |
| 192 Empty-state pointer | 191 `aiReady` contract; 183 dialog template assumed | Quick Action dialog empty state | Consumes 191 contract; plain-link only, no dialog execution change | 2 — after 191 |
| 193 Install progress + recovery | 151 install flow + 186 ownership shapes assumed | Downloader progress channel + store + Settings row/dialog | Progress-event shape; recovery copy split; supplies verification | 1 — managed owner (coordinate files with 189/191) |
| 194 Exe rename | None — mechanical | Packaging/CI/update/worker/autostart/tests | Renames `[package]` only; keeps asset prefix/identifier/data dir | 1 — independent |
| 195 Presence assets | 182 payload/seam assumed; portal upload prerequisite | Presence owner module + shape test | Assets + hover strings + ADR-0033 amendment; supplies 197 | 1 — independent |
| 196 Animation toggle + menu motion | None — new switch + token-gated transitions | Settings General + tokens + menu/dialog components | Motion-off hook contract; supplies 197 | 1 — independent |
| 197 Round verification | 189–196 contracts | Coordinator-owned checks/docs | Combined verification, ADR/CONTEXT/research reconciliation, ownership gate + verified sync | 2 — after all |

The **spec-188 integration coordinator** (ticket 197) owns shared glossary/ADR/research reconciliation, parent status, 189↔190↔191↔193 managed-file handoff, 191→192 contract, backup/size overlap, and final combined verification. Workers update their own ACs/results, never rewrite global docs. Claims above are estimates to recheck against code at dispatch (CodeGraph first). If executed as a concurrent batch, follow `docs/agents/parallel-tickets.md` with one coordinator. No parallel implementation runs during this publication session.

## Acceptance and verification

- [ ] User confirmed Start≠Off lifecycle, lazy resource disclosure, tier-first radio, app-state progress + Remove-and-retry, exe-only rename, `Sprout`/`Ready` static assets, global Animation switch + menu fade, plain-link pointer (grill rounds 1–3).
- [ ] 189–196 each verify their slice at the seams + relevant existing suites + ownership gate (ACs in owning tickets).
- [ ] 197 records combined checks, manual matrices, reconciles CONTEXT/ADR/research status, publishes each unit through `node tools/ownership-gate.mjs` + verified `tools\sync.ps1 -Up` (twice, expect 0 copied).

No application behavior changed in this planning session.
