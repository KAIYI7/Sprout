# 213 — Unified window chrome build

**Parent:** [212 — Unified window chrome (spec)](212-unified-window-chrome-spec.md)

**What to build:** The Discord-style main-window header as one vertical slice: frameless window + `UnifiedHeader.svelte` (mark + breadcrumb left, empty middle, min/max/close right) + drag grammar + maximized reflection + Settings fallback + a11y. Visual-only; close keeps destroy-window/tray-alive.

**Blocked by:** None — can start immediately (needs only the existing builder + shell).

**Status:** implemented 2026-09-16

- [x] Frameless: `decorations(false)` in the existing `open_main_window` builder (`src-tauri/src/lib.rs`); no new size constants (`constants/window.rs` stays single source); Quick Launch window/dock untouched
- [x] Window-button commands under the existing main-window owner (ADR-0029: extended `lib.rs`, never duplicated): `minimize_main_window`, `toggle_main_window_maximize`, `main_window_is_maximized`, `update_native_frame` (close reuses `destroy_main_window`); `api.ts`/`types.ts` seam only. Owner choice: main-window commands live in `lib.rs` beside `destroy_main_window`/`open_main_window` — adopt into the ticket-135 inventory.
- [x] `UnifiedHeader.svelte`: full-width above the shell split; mark + section breadcrumb (shared route map, also feeds presence); empty middle; ─ □ ✕ via shared `Icon`, grayscale rest, danger-hover close only, max glyph toggles; `RunBanner` stays below full-width; no primary/search in the header
- [x] Drag grammar: header background `data-tauri-drag-region="deep"`, button cluster opts out; double-click toggles maximize (button presses excluded)
- [ ] Right-click native system menu: **deferred, not built.** Showing the real Win32 system menu needs `GetSystemMenu`/`TrackPopupMenuEx` against the live HWND with foreground-window discipline — unsafe code that cannot be verified headless. Double-click + buttons cover the grammar; the Settings fallback covers failure. Revisit with a live HWND test session.
- [x] Maximized reflection: radius 0 + end-padding when maximized; restored geometry unchanged; rail/pages/banner untouched
- [x] Fallback + a11y: Settings `Window frame` knob (`native_frame`, immediate-apply like theme/animation, search-indexed); `aria-label`s, 30px hit areas (24px AA floor), focus-visible ring, keyboard operation; contrast-check pairs pass both modes
- [x] Scope additions from product review (grill Q11 superseded, research 0025 updates): rail brand removed (header keeps the only mark); rail + header share one hairline-free `bg-surface` against `bg-page` content; header mark aligns with the rail pills' left edge; content stage is a page-surface panel with `radius-lg` top corners (square when maximized, flush under native frame)
- [x] Verification: `npm run check` 0 errors (3 warnings: 2 pre-existing + 1 intentional explicit `banner` landmark); `npm run test` 28 files / 334 tests green (18 new `windowChrome` + 1 new search test); `cargo test` 684 passed / 0 failed; `node tools/ownership-gate.mjs` pass; `node tools/contrast-check.mjs` pass (token-only, both modes); MANUAL matrix owed (light/dark × maximized/restored × 100/150/200% DPI × keyboard-only)

## Reviewed deviations (ADR-0028 slot)

- `Icon` gains additive `square` + `restore` glyphs (maximize idiom needs them; no existing glyph fits; same stroke treatment, tokens only).
- Explicit `role="banner"` + accessible name on the header: states what AT already infers so the double-click shortcut's host is a named landmark (the action itself stays keyboard-covered via the Maximize button). Surfaces as the 1 new `a11y_no_redundant_roles` warning — accepted.

**Explicitly not built:**

- Rail-brand removal; header search/inbox/help; primaries in the header; banner/pill relocation; Quick Launch/dock chrome; new size constants; backup-format changes; per-page header redesigns
