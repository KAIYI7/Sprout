# 215 — Bezel mode backend (collapsed/peek/open geometry + zero reservation)

**Parent:** [214 — Bezel-tab dock mode + copy/i18n (spec)](214-bezel-tab-dock-mode-copy-i18n-spec.md)

**What to build:** The third dock mode as a backend vertical slice: `mode="bezel"` accepted beside `fixed`/`auto-hide`, with a collapsed tab rect protruding from the edge, a wider hover-peek rect, a full-width open rect, and zero workspace reservation while collapsed — all geometry owned by the existing dock/AppBar modules.

**Blocked by:** None — can start immediately (first mover; defines the contract others consume).

**Status:** applied + validated 2026-09-17 (`cargo test` 706 passed / 0 failed; `npm run check` 0 errors; ownership gate pass)

- [x] `mode="bezel"` accepted everywhere `dockMode` is validated/stored (settings validation, per-monitor memory, `resolve_dock_prefs_migrated`); unknown-mode fallback behavior unchanged
- [x] Collapsed rect: ~12–14px visible width × monitor-relative height (`bezel_h_ratio` ≈ 12% of docked monitor height, clamped to physical-px bounds, proposed 64–160px — ticket fixes exact numbers); constants live in `constants/window.rs` (single size source, ADR-0021)
- [x] Hover-peek rect (~22–26px wide) derived from the same constants; peek is geometry-only, never triggers open (open stays click-gated — ticket 217 owns the interaction)
- [x] Open rect = full dock width at the resolved width-% with a documented cap (proposed: auto-hide-like overlay cap since bezel reserves nothing extra — ticket records the chosen cap)
- [x] Collapsed + peek reserve zero workspace (auto-hide pattern: zero-width `reserve`/`ABM_SETPOS`); open overlays like auto-hide, never squeezes other windows while collapsed
- [x] Dimensions the frontend needs (tab rect, peek width, Y center) exposed via Tauri commands — Svelte never hard-codes them (conventions.md window-sizing rule)
- [x] Rust tests for the new geometry (collapsed/peek/open rects, clamp behavior, broken stored values fall back honestly) + `npm run check` 0 errors + `cargo test` green — 9 new/extended bezel tests, all passing in the full suite (706 passed, 0 failed, 3 ignored 2026-09-17); `npm run check` 0 errors (3 pre-existing warnings elsewhere); freed `target/debug/incremental` (build outputs kept) to unblock the test-harness link
- [x] Dated ADR amendments appended (never rewritten): ADR-0011 (third mode + zero-reservation collapsed), ADR-0019 (driver placement for bezel; driver-owns-motion boundary kept), ADR-0021 (new constants + open cap)
- [x] `docs/CONTEXT.md`: add **bezel tab** with a `planned` qualifier + spec-214 link (glossary only, no behavior prose)
- [x] `node tools/ownership-gate.mjs` passes (extend `quick_window`/`appbar` owners, never a second invocation site) — pass 2026-09-16 (62 owned references, all in owners)

**Worker notes (r1):**
- Pinned numbers: collapsed 14px, peek 24px, height 12% clamped 64–160px physical, Y default centered ratio 0.5 clamped 0..1, open cap 60% overlay, zero reservation collapsed+peek. Key name `dock.bezel.tooltip` pinned for 219 — backend holds no UI strings.
- Deviation from brief: `apply_width` does NOT retarget `last_rect` for bezel (the collapsed tab's width is constant, so there is no geometry to retarget — the memory save is the whole update; open derives the stored % on demand in 217). It still places nothing, preserving the single-writer/no-slide intent.
- Follow-ups owned elsewhere: Y persist/drag (216), open/close interaction (217), Settings UI incl. `QuickLaunchDockState["mode"]` union + `dockModeOptions` (218 — `src/lib/types.ts` untouched per allow-list).

**Explicitly not built:**

- Y-position drag/persist (ticket 216); open/close interaction states, tooltip, pulse (ticket 217); Settings UI (ticket 218); copy/loader (tickets 219–220); any change to `fixed`/`auto-hide` behavior
