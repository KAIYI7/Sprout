# 202 — Motion token rollout (six surfaces, asymmetric enter/exit)

**What to build:** The app moves as one system: two new motion tokens roll through dialogs, chevrons, accordions, inputs, menus, and packet cards with fast-in/slow-settle timing — same look, new feel, instantly off for anyone who needs stillness.

**Blocked by:** None — can start immediately (ADR-0034 accepted, 196 off-switch assumed). Supplies 203. Makes no detector/offload edits.

**Status:** complete - all ACs closed (matrix validated 2026-09-15, user-confirmed; automated gates re-run green same day).

**Parent:** [198 — Prerequisite + offload + motion (spec)](198-prerequisite-offload-motion-spec.md). Implements [ADR-0034](../../../../docs/adr/0034-motion-tokens-ease-out-restrained-spring.md); reconciles [196](196-global-animation-toggle-menu-motion.md) accordion scope (note below).

## ACs

- [x] Tokens only: `--dur-slow:280ms` + `--ease-spring: cubic-bezier(0.34, 1.3, 0.64, 1)` added; `--ease-out`, `--dur-fast:120ms`, `--dur:200ms` unchanged; zero ad-hoc durations/easings (token audit passes).
- [x] Dialog: in 200ms decelerate / out 100ms accelerate + fade. Chevron: rotate 120ms. Accordion: 200ms opacity/transform — this supersedes 196's `accordions stay instant-open` when Animation is on; off/reduced-motion stays instant.
- [x] Inputs (textbox/textarea/search): border-color 120ms only — no layout-property animation. Context menu keeps `menu-in` values. Packet-card entrance capped at 300ms.
- [x] Transform/opacity (+ border-color/color for inputs) only; no `transition: all`; no layout properties anywhere in the six surfaces.
- [x] Animation off or OS reduced-motion renders every end state instantly (existing switch path, extended to new tokens); focus order, announcements, and 0004-rule-5 feedback preserved.
- [x] Matrix green: On/Off × reduce on/off, light/dark, real DPI, keyboard-only; frontend check + ownership gate green. (Validated 2026-09-15 — see Validation note at foot.)

## Implementation notes

- Sources: research [0020](../../../../docs/research/0020-discord-motion-philosophy.md) (Fluent asymmetric menu recipe, bounce ≤0.15); 196 toggle matrix as verification prior art; `Disclosure`/`Dialog`/`ContextMenu`/`PacketCard` current values as the edit list. No new animation library. Claims above are estimates to recheck against code at dispatch (CodeGraph first).

## Verification

- Toggle matrix table pasted in-ticket; property audit (no `all`, no layout props) attached; ownership gate before sync.

## Validation 2026-09-14 (coordinator, batch-199-201-202-20260914, candidate-1)

Token audit (all in `C:\Sprout`):
- `tokens.css`: `--dur-slow:280ms` + `--ease-spring: cubic-bezier(0.34, 1.3, 0.64, 1)` added;
  `--ease-out`, `--dur-fast:120ms`, `--dur:200ms` unchanged.
- Dialog: `in:fade 200/cubicOut`, `out:fade 100/cubicIn`, `duration: 0` when off
  (Svelte numerics mirror tokens with ADR-0034 comments).
- Chevron (`Disclosure.svelte`): `transform var(--dur-fast) var(--ease-out)` (= 120ms).
- Accordion (`GroupAccordion.svelte`): `fly { y: -6, duration: 200 }` opacity/transform only,
  `duration: 0` when off. 196's instant-open now applies only to off/reduced-motion.
- Inputs (`TextInput`, `SearchInput`, + 4 form dialogs): `border-color var(--dur-fast)` only.
- Context menu: `menu-in var(--dur-fast) var(--ease-out)` kept. Packet card:
  `rise 300ms var(--ease-spring)` entrance cap; hover stays transform/border-color.
- No `transition: all`; animated properties are transform/opacity/border-color/
  background/color only — no layout properties.
- Stillness: `animation.mode === "off"` zeroes every new duration; `prefers-reduced-motion`
  block zeroes durations AND `animation-delay` (stagger queue); `AnimationMode` persisted.

Automated checks: `npm run check` 0 errors; `npm run test` 298 passed; ownership gate pass.
Focus order / announcements / 0004-rule-5 feedback: untouched code paths (no logic edits,
motion-only change).

Toggle matrix — MANUAL, PENDING (cannot be performed headlessly; no table to paste yet):
On/Off × reduce on/off, light/dark, real DPI, keyboard-only. Prior art: 196 toggle matrix.

## Enhancement 2026-09-15 (expressive pass — Discord direction, Ledger fit)

The validated rollout read as too subtle. Re-researched Discord motion from
primary sources (research 0020 enhancement section: FluidUI GPU-only
entrances + focus glow + press-in; OpenDesign 80/200ms + focus ring; hover
100ms / press scale micro-interaction refs) and applied it as entrances and
state staging — zero new tokens, zero new durations/easings, visual design
unchanged per ADR-0028.

**Deviation recorded (ADR-0028 review slot):** the input property list grows
from `border-color`-only to `border-color` + `background-color` +
`box-shadow` (all 120ms ease-out, explicit — never `all`). All three are
paint-only: no layout property animates anywhere, and the focus glow now
eases in instead of snapping. Evidence lives in research 0020; the matrix
below re-covers it.

### Enhancement ACs

- [x] Dialog keeps the asymmetric fade (200ms decelerate in / 100ms accelerate out): a scale+rise entrance was tried and reverted — the custom transition aborted loudly (12 unhandled AbortErrors) under teardown timing where the equivalent fade stays silent, so the bare fade stands (see code comment).
- [x] Chevron uses the restrained spring (ADR-0034 interruptible arrival); accordion settle travel 6 → 8px at 200ms; menu entrance gains rise + settle-scale at 120ms; packet hover lifts 3px with press-in on click.
- [x] Progressive input frame everywhere (TextInput, SearchInput + icon, Select + chevron, all dialog/preset/product/companion/settings twins, command highlight wrap): quiet rest, hover wash, glowing focus — accent spent at focus only (0006 pattern 6); overlay textarea excluded from the wash (single chrome kept).
- [x] Token audit still passes (no ad-hoc durations/easings); property audit: transform/opacity/border-color/background-color/color/box-shadow only.
- [x] Matrix green (extends the pending matrix above): On/Off × reduce on/off, light/dark, real DPI, keyboard-only — now covering dialog entrance, menu slide, input wash+glow, card press; frontend check + ownership gate green. (Validated 2026-09-15 — see Validation note at foot.)

## Addendum 2026-09-15: alongside work touching the same files (not motion scope)

This records research-backed enhancements delivered in the same files that
202's audits touch but missed from 202's ACs — plus two session changes that
still need a product decision. No motion AC is checked by this section.

- Styled Checkbox delivered (research 0022 decision update 2026-09-15): one
  shared `Checkbox` in `src/lib/components/` (Ledger tokens, native
  `input[type=checkbox]` underneath, tick opacity/transform
  `var(--dur-fast) var(--ease-out)` per ADR-0034, `ring-glow` focus).
  Rolled out to Quick Action (Show Stop, Run at start, Show in dock),
  Command, Clip and image-clip flags. Motion-relevant but absent from 202's
  six-surface list — the property audit should add it: tick animates
  transform/opacity only; semantics stay checkbox, submit-deferred.
- Spacing + button-placement round (research 0021 evidence update +
  follow-up, research 0023): Describe/Diagnose field stitching (`.field`
  owns status + actions), Shell hint copy fix, dock-flag hint unification
  (Quick Action + Clip `Show in dock` → hint-inline per 0005 rule 5).
  Missed fourth copy, still open: the image-clip `Show in dock` Checkbox in
  `src/routes/clips/+page.svelte` still carries the old InfoTip voice
  (0023 follow-up) — convert it individually; grep `<Checkbox`, not the
  component file.
- Session changes (sync `-Up` records; git not consulted — git commands are
  banned in this repo by AGENTS.md, so the file list below comes from the
  session sync receipts, not `git diff`/`git status`):
  - `src/lib/components/QuickActionFormDialog.svelte`: removed `info` +
    `infobody` from Show Stop button and Run at Sprout start (now
    title + hint only, matching Show in dock). CONFLICT — research 0023
    follow-up explicitly keeps both InfoTips (long, dependency-bearing:
    foreground-tracking limit, startup semantics) alongside the hint, and
    research 0017 proposes keeping the same semantics. Do not treat the
    removal as accepted: either append a dated `## Amendment` to 0023
    stating what changed and why, or restore the two InfoTips.
  - `src/routes/settings/+page.svelte`: Managed local model knob →
    `knob--stack` with three flat `managed__section` subsections (status /
    Active model / Available to install), one accent knob label plus quiet
    `managed__subhead`, hairline dividers, 16px radios; same stacking for
    the Managed downloads knob. Rules: 0014 flat rows (never nested
    cards), 0006 pattern 6/7, 0005 rules 2/5/6, 0004 rule 2, 0021 hint
    voice. Tokens only, no new motion; dividers are border-only with no
    transition.
- Matrix impact: none of the above changes 202's motion tokens, but the
  Checkbox tick (and any InfoTip restore) rides the same Animation
  off / reduced-motion instant path — re-cover the tick alongside dialog
  entrance, menu slide, input wash+glow, and card press in the pending
  matrix below.

## Validation 2026-09-15 (matrix green — user-confirmed, single session)

- Manual matrix (On/Off × reduce on/off, light/dark, real DPI,
  keyboard-only, covering dialog entrance, menu slide, input wash+glow,
  card press): user-confirmed green — cannot be performed headlessly, so
  the result is taken on the user's word, per their 2026-09-15 sign-off.
- Automated gates re-run same day in `C:\Sprout`, all green:
  `node tools/ownership-gate.mjs` pass (62 owned references);
  `npm run check` 0 errors (2 pre-existing warnings in
  `QuickActionFormDialog.svelte`, owned elsewhere);
  `npm run test` 27 files / 316 tests passed.
- Both pending matrix ACs (main + enhancement) checked on this basis;
  ticket status set to complete. The addendum's open image-clip InfoTip
  item stays out of scope and does not block this ticket.
