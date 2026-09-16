# Unified window chrome — what Sprout adopts

Date: 2026-09-16. Status: research note behind spec 212 / ticket 213. Discord is treated
as direction, not a spec to copy (same stance as research 0020/0021): no
Discord-official window-geometry measurements were sourced, so every value below maps
onto existing Ledger tokens per ADR-0028 and the single size source per ADR-0021.

## User hypothesis

- The native Windows titlebar (OS-drawn icon + "Sprout" + min/max/close) plus the
  left NavRail brand reads as two stacked rows; they should become one smooth
  continuous bar like Discord, with navigation context and window controls in the
  same strip.

## What verifies

- **Discord Visual Refresh header (Feb 2025, rolled to all users):** one continuous
  custom header bar across the top carrying the current context label (server name /
  "Direct Messages") plus inbox + help, with window controls integrated at the right
  end of that same bar. No second native titlebar row. (Whop writeup of the Feb-2025
  refresh: "header bar at the top of the app ... the server you're currently viewing,
  the inbox, and the help button".)
- **Titlebar-into-header is the shipped community grammar:** `visual-refresh-compact-title-bar`
  moves the titlebar buttons down into the channel header to save a row
  (`--vr-header-snippet-space` reserves the right-end width for the buttons);
  `RichardSepsi/Discord-Merge-Titlebar` merges titlebar + menubar the same way.
  The merge direction is always buttons-into-content-header, never a second chrome row.
- **Mechanism is frameless + drag strip + app-drawn controls:** frameless window
  (`decorations: false` equivalent) + `data-tauri-drag-region` drag strip + app-drawn
  min/max/close calling window APIs. Vesktop/Vesktop custom-titlebar patches exist
  precisely to keep this grammar working across Discord UI updates; Tauri documents the
  same `data-tauri-drag-region` contract Sprout already uses in the Quick Launch window
  (`src/lib/quickLaunchTitleBar.ts`, `"deep"` floating / `"false"` docked).
- **Sprout current (verified via CodeGraph/source, not prose):** `src/routes/+layout.svelte`
  shell is left `NavRail` + `stage` (`RunBanner` + `main`) with no custom main-window
  titlebar; brand lives at the top of the left rail (`NavRail.svelte`). Main window is
  built in `src-tauri/src/lib.rs:1218-1232` (`.title("Sprout")`, transparent,
  `skip_taskbar`, sizes from `constants/window.rs`) with no `.decorations(false)` —
  the top strip is native OS chrome. No minimize/maximize/close Tauri commands exist
  for the main window yet. Quick Launch window/dock chrome (ADR-0011) is already custom
  and out of scope.

## What Sprout adopts (spec 212, ticket 213)

- One full-width `UnifiedHeader` above the existing shell split: small Sprout mark +
  section breadcrumb left (same section map as presence in `+layout.svelte`), empty
  middle, min/max/close right in the same strip. Frameless (`decorations: false` in the
  `open_main_window` builder) + header-background drags + controls/breadcrumb-links opt
  out; double-click toggles maximize; right-click shows the native system menu.
- Close keeps the existing destroy-main-window / tray-stays-alive contract (ADR-0010/0013);
  min/max keep native meaning. No behavior change, purely visual.
- Maximized/snapped is CSS reflection only: header radius to 0, extra end-padding so
  controls never kiss the screen edge. No new size constants — `constants/window.rs`
  stays the single size source (ADR-0021).
- Buttons use the Windows-11 native idiom (─ □ ✕) drawn with shared `Icon` + tokens:
  grayscale rest, danger-hover only on close, max glyph toggles when maximized. Accent
  is never spent on window controls (0006 pattern 6).
- `NavRail` keeps its mark + wordmark + clusters + update pill; `PageHeader` keeps its
  h1 + single primary; `RunBanner` stays below the header full-width. Header never holds
  a primary button or search/inbox clones. Two marks co-exist in this round; rail-brand
  removal is explicitly deferred (0006 pattern 5 relearning cost).
- Fallback + a11y gate: frameless by default with a native-frame fallback behind a
  Settings flag (durable preference → 0008 rule 1 says Settings, not a header menu);
  all three buttons get `aria-label`, full-size hit area, focus-visible ring,
  keyboard operation, contrast-checked pairs.

## Rules applied

- 0004 rule 2 (frequency split: empty middle, no rarely-touched chrome in the strip),
  0005 rules 1/2/5/6 (one header impl, one primary per row — never in the header,
  same-kind treatment, rhythm owned by the component), 0006 patterns 5/6 (relearning
  cost keeps the rail brand; one reserved accent), 0008 rule 1 (classify the fallback
  knob before placing it → Settings), ADR-0028 (tokens/components only) + ADR-0034
  motion tokens + ADR-0029 (extend the window owner, never a second invocation site).

## Decision update — 2026-09-16: single mark, seamless surface

The grill's two-mark coexistence (0006 pattern 5 relearning caution) is
superseded by product review: the rail brand is removed and the header keeps
the only mark, and the header/rail/content hairlines go so the three read as
one surface (Discord + Task Manager direction). The rail's cluster dividers
stay — they organize frequency groups (0004 rule 2), not chrome regions.

## Decision update — 2026-09-16: validation + content panel

Product review challenged two points — validated here before building:

- **One surface for nav+header, distinct from content: confirmed.** Discord's
  own Mar-2025 changelog moved the Inbox into the title bar
  (discord.com/blog/discord-update-march-25-2025-changelog) with the stated
  goals of legibility, less visual noise, and desktop/mobile consistency
  (discord.com/blog/player-release-q12025; Verge). The shipped refresh styles
  columns with distinct background tokens and zeroes divider hairlines
  (community theme repos overriding `border-radius`/`::after` separators —
  direction, not a spec). Task Manager agrees on the no-divider half (single
  Mica, content carried by cards). Sprout inference: rail + header share
  `bg-surface`, content stays `bg-page` — the surface/page split the Ledger
  already encodes (tokens.css), applied to chrome per 0005 rule 5.
- **Rounded inner content corner: confirmed.** The refresh's signature is
  rounded panels throughout (Whop). Sprout inference: the content stage is a
  page-surface panel with `radius-lg` (8px, existing token — no ad-hoc value)
  top corners over the chrome surface; square when maximized (native
  maximized windows meet the screen edge) and flush under the native-frame
  fallback. Motion tokens unchanged (ADR-0034).

## Sources

- Whop, *Discord desktop design change* (Feb-2025 Visual Refresh rollout, header-bar
  consolidation) — https://whop.com/blog/discord-new-design/
- `surgedevs/visual-refresh-compact-title-bar` (titlebar buttons moved into the channel
  header, `--vr-header-snippet-space` reservation) — https://github.com/surgedevs/visual-refresh-compact-title-bar
- `RichardSepsi/Discord-Merge-Titlebar` (titlebar/menubar merge) — https://github.com/RichardSepsi/Discord-Merge-Titlebar
- Vencord/Vesktop custom-titlebar patches + Windows/macOS transparency controls (frameless
  mechanism precedent) — https://deepwiki.com/Vencord/Vesktop/5.2-windows-and-macos-features
- Sprout source: `src/routes/+layout.svelte`, `src/lib/components/NavRail.svelte`,
  `src-tauri/src/lib.rs` (`open_main_window`), `src-tauri/src/constants/window.rs`,
  `src/lib/quickLaunchTitleBar.ts`, `src/lib/styles/tokens.css`.
