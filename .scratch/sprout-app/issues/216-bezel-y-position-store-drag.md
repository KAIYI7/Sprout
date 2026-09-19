# 216 — Bezel Y-position store + drag + per-monitor restore

**Parent:** [214 — Bezel-tab dock mode + copy/i18n (spec)](214-bezel-tab-dock-mode-copy-i18n-spec.md)

**What to build:** The bezel tab parks where the user drags it vertically and stays there per display: drag the tab up/down the edge, and that height is remembered under the same per-monitor identity pattern as edge/mode/width, restored on redock and reboot.

**Blocked by:** [215 Bezel mode backend](215-bezel-mode-backend.md) (store keys + geometry contract).

**Status:** applied + validated 2026-09-17 (`cargo test` 711 passed / 0 failed; `npm run check` 0 errors; ownership gate pass)

- [x] `bezel_y_ratio 0..1` (fraction of the docked monitor's height) saved beside existing dock memory under the identity-key pattern (EDID identity preferred, device-name fallback — same `memory_key` seam as `save_dock_edge/mode/width_pct`)
- [x] Drag affordance on the docked bezel tab (vertical drag moves the tab live; release persists); keyboard-accessible nudge alternative so drag is never the only path
- [x] Clamp on load (ratio pinned to visible edge bounds at current resolution); monitor disconnect/resize falls back to vertical center — never off-screen or lost
- [x] Side stays the existing Left/Right setting + per-display edge override (no new side store; reuses edge memory)
- [x] Rust + frontend tests for save/restore/clamp/fallback (incl. identity-preferred-over-device-name, like the width-% precedent) + `npm run check` 0 errors
- [x] Dated ADR-0020 amendment appended (bezel Y joins the identity-keyed memory family); `docs/CONTEXT.md`: add **bezel Y position** with `planned` qualifier + spec-214 link
- [x] `node tools/ownership-gate.mjs` passes (extend the `db`/`quick_window` dock-memory owner)

**Explicitly not built:**

- Geometry constants or caps (ticket 215); open/close interaction (ticket 217); Settings knobs (ticket 218); copy/loader (tickets 219–220)
