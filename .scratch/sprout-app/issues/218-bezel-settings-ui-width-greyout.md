# 218 — Bezel Settings UI (mode option + edge + Y + width grey-out link)

**Parent:** [214 — Bezel-tab dock mode + copy/i18n (spec)](214-bezel-tab-dock-mode-copy-i18n-spec.md)

**What to build:** The Settings surface for bezel mode: pick it globally and per display, adjust the tab's vertical position without dragging, and stop being lied to by the global Dock width slider when per-display widths are in charge.

**Blocked by:** [215 Bezel mode backend](215-bezel-mode-backend.md) (mode value exists) + [216 Y-position store](216-bezel-y-position-store-drag.md) (Y pref exists).

**Status:** applied + validated 2026-09-17 (`npm run check` 0 errors; bezel settings + settingsSearch + copy + dock + bezelY + bezelInteraction tests pass; ownership gate pass; 1 pre-existing unrelated failure in `managedSettings.close.test.ts` — see notes)

- [x] `bezel` appears in the global Dock mode options and in every per-display mode list, with the rewritten hint from ticket 219's dictionary (falls back to current hint prose until 219 lands)
- [x] Tab vertical-position control binds to `bezel_y_ratio` (slider or numeric — mirrors the drag value both ways); edge stays the existing Left/Right selects
- [x] Global Dock width slider disables exactly when `displays.length > 1`, showing hint text with a working anchor link to `#per-monitor-title` ("Per-display widths are active — adjust below"); single display keeps it enabled
- [x] New knobs join the existing search index + dirty-bar/save flow like their neighbors (no bespoke persistence)
- [x] Frontend tests for option presence, Y binding, and the multi/single-display disabled logic + `npm run check` 0 errors
- [x] `node tools/ownership-gate.mjs` passes; no ADR text changes

**Worker notes (r1):**
- `dockWidthMaxForMode` now caps `bezel` at 60 like auto-hide (both overlay, zero reservation) — mirrors the backend `dock_width_max_pct_for_mode("bezel") == 60`; fixed stays 30.
- Y knob label reuses `quickwindow.bezelPosition` ("Bezel tab position") instead of a new key — ticket 219's dedup rule (identical text reuses one key). New keys: `settings.opt.mode.bezel`, `settings.dock-width.multiHint`, `settings.bezel-y.hint`, `search.bezel-y.desc` (+ EN/ZH in lockstep; zh key set stays equal per copy test).
- Global Y knob shows only on a single display (a view onto that display's memory, same rule as width); several displays get one slider per per-monitor row (research 0006 pattern 4). No mode-gating: the ratio is remembered position state like edge/width, not auto-hide-only behavior tuning.
- Copy voice follows the loader's current rules (noun titles, Discord-tone descriptions, ≤140-char hints); `settings.dock-mode.hint` / `settings.dock-width.hint` / `settings.per-monitor.hint` + search descs extended for bezel within the cap.
- Pre-existing failure (not this ticket): `managedSettings.close.test.ts` → "lists not-yet-installed models once under Available" expects the string "Available to install", which exists nowhere in `src/` (the app renders `settings.ai-available.label` = "Available models"). Ticket 191 is still `ready-for-agent`; its test anticipates unimplemented copy. Left untouched.

**Explicitly not built:**

- Backend geometry, persistence, or interaction (tickets 215–217); copy rewrite itself (ticket 219 — this ticket only binds the strings); translation (ticket 220)
