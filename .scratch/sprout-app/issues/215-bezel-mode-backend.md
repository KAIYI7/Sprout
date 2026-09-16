# 215 — Bezel mode backend (collapsed/peek/open geometry + zero reservation)

**Parent:** [214 — Bezel-tab dock mode + copy/i18n (spec)](214-bezel-tab-dock-mode-copy-i18n-spec.md)

**What to build:** The third dock mode as a backend vertical slice: `mode="bezel"` accepted beside `fixed`/`auto-hide`, with a collapsed tab rect protruding from the edge, a wider hover-peek rect, a full-width open rect, and zero workspace reservation while collapsed — all geometry owned by the existing dock/AppBar modules.

**Blocked by:** None — can start immediately (first mover; defines the contract others consume).

**Status:** ready-for-agent

- [ ] `mode="bezel"` accepted everywhere `dockMode` is validated/stored (settings validation, per-monitor memory, `resolve_dock_prefs_migrated`); unknown-mode fallback behavior unchanged
- [ ] Collapsed rect: ~12–14px visible width × monitor-relative height (`bezel_h_ratio` ≈ 12% of docked monitor height, clamped to physical-px bounds, proposed 64–160px — ticket fixes exact numbers); constants live in `constants/window.rs` (single size source, ADR-0021)
- [ ] Hover-peek rect (~22–26px wide) derived from the same constants; peek is geometry-only, never triggers open (open stays click-gated — ticket 217 owns the interaction)
- [ ] Open rect = full dock width at the resolved width-% with a documented cap (proposed: auto-hide-like overlay cap since bezel reserves nothing extra — ticket records the chosen cap)
- [ ] Collapsed + peek reserve zero workspace (auto-hide pattern: zero-width `reserve`/`ABM_SETPOS`); open overlays like auto-hide, never squeezes other windows while collapsed
- [ ] Dimensions the frontend needs (tab rect, peek width, Y center) exposed via Tauri commands — Svelte never hard-codes them (conventions.md window-sizing rule)
- [ ] Rust tests for the new geometry (collapsed/peek/open rects, clamp behavior, broken stored values fall back honestly) + `npm run check` 0 errors + `cargo test` green
- [ ] Dated ADR amendments appended (never rewritten): ADR-0011 (third mode + zero-reservation collapsed), ADR-0019 (driver placement for bezel; driver-owns-motion boundary kept), ADR-0021 (new constants + open cap)
- [ ] `docs/CONTEXT.md`: add **bezel tab** with a `planned` qualifier + spec-214 link (glossary only, no behavior prose)
- [ ] `node tools/ownership-gate.mjs` passes (extend `quick_window`/`appbar` owners, never a second invocation site)

**Explicitly not built:**

- Y-position drag/persist (ticket 216); open/close interaction states, tooltip, pulse (ticket 217); Settings UI (ticket 218); copy/loader (tickets 219–220); any change to `fixed`/`auto-hide` behavior
