# 202 — Motion token rollout (six surfaces, asymmetric enter/exit)

**What to build:** The app moves as one system: two new motion tokens roll through dialogs, chevrons, accordions, inputs, menus, and packet cards with fast-in/slow-settle timing — same look, new feel, instantly off for anyone who needs stillness.

**Blocked by:** None — can start immediately (ADR-0034 accepted, 196 off-switch assumed). Supplies 203. Makes no detector/offload edits.

**Status:** awaiting-validation — code + automated checks green; manual toggle matrix pending (see Verification 2026-09-14).

**Parent:** [198 — Prerequisite + offload + motion (spec)](198-prerequisite-offload-motion-spec.md). Implements [ADR-0034](../../../../docs/adr/0034-motion-tokens-ease-out-restrained-spring.md); reconciles [196](196-global-animation-toggle-menu-motion.md) accordion scope (note below).

## ACs

- [x] Tokens only: `--dur-slow:280ms` + `--ease-spring: cubic-bezier(0.34, 1.3, 0.64, 1)` added; `--ease-out`, `--dur-fast:120ms`, `--dur:200ms` unchanged; zero ad-hoc durations/easings (token audit passes).
- [x] Dialog: in 200ms decelerate / out 100ms accelerate + fade. Chevron: rotate 120ms. Accordion: 200ms opacity/transform — this supersedes 196's `accordions stay instant-open` when Animation is on; off/reduced-motion stays instant.
- [x] Inputs (textbox/textarea/search): border-color 120ms only — no layout-property animation. Context menu keeps `menu-in` values. Packet-card entrance capped at 300ms.
- [x] Transform/opacity (+ border-color/color for inputs) only; no `transition: all`; no layout properties anywhere in the six surfaces.
- [x] Animation off or OS reduced-motion renders every end state instantly (existing switch path, extended to new tokens); focus order, announcements, and 0004-rule-5 feedback preserved.
- [ ] Matrix green: On/Off × reduce on/off, light/dark, real DPI, keyboard-only; frontend check + ownership gate green.

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
