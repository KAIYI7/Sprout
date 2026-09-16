# 218 — Bezel Settings UI (mode option + edge + Y + width grey-out link)

**Parent:** [214 — Bezel-tab dock mode + copy/i18n (spec)](214-bezel-tab-dock-mode-copy-i18n-spec.md)

**What to build:** The Settings surface for bezel mode: pick it globally and per display, adjust the tab's vertical position without dragging, and stop being lied to by the global Dock width slider when per-display widths are in charge.

**Blocked by:** [215 Bezel mode backend](215-bezel-mode-backend.md) (mode value exists) + [216 Y-position store](216-bezel-y-position-store-drag.md) (Y pref exists).

**Status:** ready-for-agent

- [ ] `bezel` appears in the global Dock mode options and in every per-display mode list, with the rewritten hint from ticket 219's dictionary (falls back to current hint prose until 219 lands)
- [ ] Tab vertical-position control binds to `bezel_y_ratio` (slider or numeric — mirrors the drag value both ways); edge stays the existing Left/Right selects
- [ ] Global Dock width slider disables exactly when `displays.length > 1`, showing hint text with a working anchor link to `#per-monitor-title` ("Per-display widths are active — adjust below"); single display keeps it enabled
- [ ] New knobs join the existing search index + dirty-bar/save flow like their neighbors (no bespoke persistence)
- [ ] Frontend tests for option presence, Y binding, and the multi/single-display disabled logic + `npm run check` 0 errors
- [ ] `node tools/ownership-gate.mjs` passes; no ADR text changes

**Explicitly not built:**

- Backend geometry, persistence, or interaction (tickets 215–217); copy rewrite itself (ticket 219 — this ticket only binds the strings); translation (ticket 220)
