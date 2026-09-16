# 206 — Companion picker cap, first five

**Parent:** [205 — Companion picker cap, Python detection + shell, file Download, staging grace (spec)](205-companion-cap-python-download-staging-grace-spec.md)

**What to build:** The dock Companion picker stays a fast one-click surface no matter how many sites are saved: it shows the first 5 sites in the user order plus a management row, while Settings and the main-app manager stay unlimited.

**Blocked by:** None — can start immediately (needs only the delivered 170 lifecycle).

**Status:** done — implemented 2026-09-16, ACs verified (see Done notes).

- [x] Dock picker renders at most the first 5 sites in user order with the active site marked, plus a `Manage in Sprout…` row opening the `/companion` manager; no search bar or lazy loading inside the dock
- [x] Single-site plain label, Off/floating absent-pane behavior, and the 170 switch lifecycle (replace live page, keep profile/identity/zoom, active no-op) unchanged
- [x] Settings Active-site control still lists every site; saved list stays unbounded in settings/backup; nothing is deleted by the cap
- [x] Verification: picker-model tests green (0/1/5/6+/unnamed-site cases), keyboard + narrow-dock behavior, frontend check clean

**Explicitly not built:**

- Main-app list filter/virtualization (deferred until real lists approach ~15)
- Any change to the single-site lifecycle, isolation, or docked-only visibility

## Done notes (2026-09-16)

- Cap lives in `src/lib/companion.ts`: `COMPANION_DOCK_PICKER_LIMIT = 5`,
  `companionDockPickerSites` slices the first 5 in user order (pure, never
  mutates the saved list). Dock menu in
  `src/routes/quick-launch-window/+page.svelte` builds from the capped set
  with active-mark, ends with a `separator` + `Manage in Sprout…` row into
  `open_companion_manager` (`/companion`); no search/lazy in dock. Settings
  Active-site control maps the whole list and never imports the cap.
- Verification: `src/lib/companionPicker.test.ts` 19/19 pass (0/1/5/6+/
  unnamed-site cases, active-mark, manage-row routing, Settings uncapped);
  `npm.cmd run check` 0 errors (3 pre-existing warnings).