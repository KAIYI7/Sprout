# 212 — Unified window chrome: Discord-style main-window header (spec)

**Status:** planning package only — no application behavior changed here. Ticket 213 `ready-for-agent` (created next in this unit).

**Parents / related (read before implementing):**
- [11 app shell + design foundation](11-app-shell-and-design-foundation.md) — `+layout.svelte` shell (`NavRail` + `stage`), tokens/components ownership. This round adds one header above the split; rail/stage contracts unchanged.
- [76 boot-to-tray + dock restore](76-boot-to-tray-dock-restore.md) + ADR-0013 — main-window lifecycle (destroy-on-close, tray-resident). Close keeps that contract; no lifecycle change here.
- [52 Quick Launch window](52-quick-launch-window.md) + [53 dock AppBar](53-quick-launch-dock-appbar.md) + ADR-0011 — dock/window chrome already custom. Explicitly untouched in this round.
- [204 custom input controls](204-custom-input-controls-discord-frame.md) — prior art for a Discord-direction standalone UX pass (research note + token/component-only + reviewed deviation slot).
- ADRs: 0010 (tray-resident), 0013 (boot-to-tray, conf declares no windows), 0021 (single size source), 0028 (design system + disclosure), 0029 (one owner per Windows command), 0034 (motion tokens). **No ADR text changes in this round:** frameless is a visual flag inside the existing `open_main_window` builder (reversible, surprising to nobody given research 0025, token/component-only) — it moves no decision boundary, so per the sparing rule no new ADR and no amendment ships. Where window-button commands land, the owning ticket records the owner (ADR-0029 inventory: extend, never duplicate).
- Research: NEW in this unit: [0025 unified window chrome](../../../docs/research/0025-unified-window-chrome.md) — Discord Feb-2025 header evidence + adopted rules (0004:2, 0005:1/2/5/6, 0006:5/6, 0008:1). Standing rules cited per decision, not re-derived.
- Glossary: no `docs/CONTEXT.md` change — `window chrome` / `unified header` are UI synonyms for the existing shell, not new domain terms.

## Problem Statement

The main window shows two stacked brands: the native OS titlebar (icon + "Sprout" + min/max/close) above the left NavRail's own mark + "Sprout". That double row wastes vertical space and never reads as one app the way Discord's single continuous header does.

## Solution

Ship one full-width `UnifiedHeader` above the existing shell split — small Sprout mark + section breadcrumb left, empty middle, native-idiom min/max/close right in the same strip — over a frameless main window. Visual-only: close still destroys the main window with the tray alive, min/max keep native meaning, all sizes/tokens/components reused.

## User Stories

1. As a Sprout user, I want one continuous top bar with context + window controls, so the double titlebar + rail-brand row disappears.
2. As a Windows user, I want min/max/close to look and behave natively (double-click toggles maximize, right-click shows the system menu), so no muscle memory breaks.
3. As a maximized-window user, I want controls clear of the screen edge with square corners, so the bar reads correct snapped and restored.
4. As a NavRail user, I want the rail brand, order, update pill, and every page header untouched, so nothing learned moves.
5. As a keyboard/screen-reader user, I want labeled, focusable, contrast-safe window controls, so custom chrome stays operable.
6. As a cautious user on odd DPI/drivers, I want a native-frame fallback in Settings, so a frameless failure never traps me.

## Shared understanding (grill close-out, R1 Q1–Q6 + R2 Q7–Q12)

1. **Scope:** main window only; Quick Launch window/dock chrome untouched.
2. **Semantics:** visual-only; close = destroy main window, tray alive (ADR-0010/0013 unchanged).
3. **Shape:** full-width header above the shell split (Discord pattern), not an L-strip over content only.
4. **Left content:** mark + section breadcrumb (presence section map); `PageHeader` keeps h1 + single primary; header never holds a primary/search/inbox.
5. **Middle:** empty breathing room; `RunBanner` stays below the header full-width; update pill stays in the rail.
6. **Brand:** two marks co-exist this round; rail-brand removal deferred (0006 pattern 5).
7. **Frameless:** `decorations: false` in the existing builder + drag region + app-drawn controls, with a Settings-flag native fallback.
8. **Buttons:** ─ □ ✕ idiom via `Icon` + tokens, grayscale, danger-hover on close only, max glyph toggles.
9. **Styling:** tokens/components only; deviation (if any) recorded in the ticket for review before shipping.

## Current vs accepted vs proposed

- Current (verified via CodeGraph/source, not prose): no custom main-window titlebar; `open_main_window` (`lib.rs:1218-1232`) sets title/transparent/`skip_taskbar`/sizes with no `decorations(false)`; no main-window min/max/close commands; sizes owned by `constants/window.rs`.
- Accepted-but-unimplemented assumed: none touching this surface.
- Proposed here: items 1–9 above. No lifecycle/size/backup/tracking change, no second Windows-invocation site, no new domain term.

## Seams (for implementer + reviewer confirmation)

One owner per area, existing seams preferred: main-window builder + window-button commands under the existing main-window owner in `lib.rs` (extend per ADR-0029 — new `minimize/maximize/close` Tauri commands reusing the destroy-on-close path for ✕; record the owner choice in the ticket so the gate inventory can adopt it); new `UnifiedHeader.svelte` (+ tiny maximized-state/section-label helper beside it, no `shared/` module — no second adapter); `+layout.svelte` composes header above shell; Settings owns the fallback flag. No new seams beyond the header component.

## Design rules every implementer applies

- **Codebase-design vocabulary (docs/agents/conventions.md):** deep modules at clean seams; deletion test; extend the window owner, never a second invocation site; ownership gate must pass.
- **UI/UX (docs/agents/ui-ux.md):** reuse tokens from `src/lib/styles/tokens.css` and components from `src/lib/components/`; cite the applied research rule per change (0025 + 0004:2, 0005:1/2/5/6, 0006:5/6, 0008:1). No ad-hoc colors/type/radii/dimensions.
- **Planning (docs/agents/planning.md):** distinguish current/accepted/proposed; record assumed pendings (none); no silent ADR overwrite — none ships in this round by design.

## Out of Scope

- Quick Launch window/dock chrome changes; rail-brand removal; header search/inbox/help clones; primary buttons in the header; `RunBanner`/update-pill relocation; new size constants; backup/Settings-shape changes beyond the one fallback flag; per-page header redesigns.

## Ticket map and integration ownership

| Ticket | Behavioral prerequisites | Likely paths / owner symbols | Shared contract and integration edits | Candidate wave |
| --- | --- | --- | --- | --- |
| [213 Unified header build](213-unified-window-chrome.md) | None — vertical slice over the existing builder + shell | `src-tauri/src/lib.rs` (`open_main_window` + window-button commands, ADR-0029 owner); new `UnifiedHeader.svelte`; `src/routes/+layout.svelte`; Settings fallback flag; `src/lib/api.ts` + `src/lib/types.ts` seam | Owns frameless flag, drag grammar, maximized reflection, breadcrumb, buttons, fallback, a11y, contrast + ownership gates; single implementer, no handoff | 1 — alone |

Single ticket is the whole round: one vertical slice, one owner, no parallel wave, no coordinator beyond the ticket itself. Claims above are estimates to recheck against code at dispatch (CodeGraph first).

## Acceptance and verification

- [x] User confirmed main-window-only, visual-only, full-width header, mark + breadcrumb, empty middle, frameless-with-fallback, token-only styling (grill R1–R2).
- [ ] 213 verifies frameless + drag/double-click/system-menu, breadcrumb, maximized geometry, untouched rail/pages/banner, fallback flag, keyboard/screen-reader/DPI/light-dark, checks + tests + ownership + contrast gates.

No application behavior changed in this planning session.
