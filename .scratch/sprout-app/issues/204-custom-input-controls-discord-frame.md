# 204 — Custom input controls: Discord filled frame + popout dropdown

**What to build:** Every text-like input (textbox, search, textarea, compact
row inputs) shares one filled Discord-direction frame, and `Select` stops
opening the OS-default option list — it opens a themed popout in the app's
own menu grammar instead.

**Status:** in-progress

**Parent:** none (standalone UX pass). Implements research 0021; ADR-0028
(token/component-only) and ADR-0034 (motion tokens) unchanged.

## ACs

- [x] Textbox (`TextInput`), search (`SearchInput`, add-panel + product-dialog twins), and every textarea twin (clip/command/Quick Action/note/stop/pre-action) share one frame: `bg-sunken` rest + hairline `border` edge + `radius-lg`; hover `bg-hover` wash + `border-strong`; focus accent + `ring-glow`. Compact row inputs (env/name/pinned) keep `radius-sm` density with the same staging.
- [x] `Select` renders a trigger button in the same frame (label + chevron, ellipsis, `<label for>` association kept) and opens a `ContextMenu` popout (`align start`, `matchAnchorWidth`): checked current value, disabled rows, title tooltips. No native `<select>` remains in the component.
- [x] All existing `Select` call sites migrated to `options` (settings ×9 incl. per-monitor + companion + AI provider, companion identity, command/QA shells, QA group, product/preset env actions, preset product picker + version policy). Optgroup explanation preserved as disabled-row tooltips.
- [x] Keyboard + screen-reader parity: Enter/Space/Arrows open, arrows/Home/End move, Enter picks, Escape/Tab closes with focus restored; `aria-haspopup="menu"` + `aria-expanded`; checked rows announce as `menuitemradio`.
- [x] `managedSettings` assertions updated to the trigger readout (`data-value`); new `Select` component tests (open/pick/Escape/disabled/keyboard) green.
- [x] `npm run check` 0 errors; `npm run test` green; ownership gate green before sync.

## Deviation recorded (ADR-0028 review slot)

- Shared `.ctx-menu` gains `max-height: min(320px, calc(100vh - 16px))` +
  `overflow-y: auto` so long option lists scroll inside the popout. No new
  color/type/radius; one behavior cap, reviewed here.
- `ContextMenuItem` gains an additive optional `title` (native option-title
  parity). No visual change to existing menus.

## Validation — 2026-09-15 (single session)

- `npm run check`: 0 errors, 2 warnings (both pre-existing in
  `QuickActionFormDialog.svelte:141`, untouched by this unit).
- `npm run test`: 27 files / 316 tests green (309 pre-existing + 7 new
  `selectDropdown.close.test.ts`).
- `cargo test ai_assist`: 35/35 (ticket-200 backend battery, untouched).
- `node tools/ownership-gate.mjs`: pass (62 owned references).
- `node tools/contrast-check.mjs`: all pairs pass, both modes (no new
  colors — token reuse only).
- Token audit: no ad-hoc durations/easings/`transition: all` in touched
  files; animated props stay transform/opacity/border-color/
  background-color/color/box-shadow.
- Matrix (light/dark × animation on/off × keyboard-only, real DPI):
  MANUAL, PENDING — same headless limit as tickets 196/202; owed to the
  203 round-verification matrix.
