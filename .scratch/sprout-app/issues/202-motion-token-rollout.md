# 202 — Motion token rollout (six surfaces, asymmetric enter/exit)

**What to build:** The app moves as one system: two new motion tokens roll through dialogs, chevrons, accordions, inputs, menus, and packet cards with fast-in/slow-settle timing — same look, new feel, instantly off for anyone who needs stillness.

**Blocked by:** None — can start immediately (ADR-0034 accepted, 196 off-switch assumed). Supplies 203. Makes no detector/offload edits.

**Status:** ready-for-agent

**Parent:** [198 — Prerequisite + offload + motion (spec)](198-prerequisite-offload-motion-spec.md). Implements [ADR-0034](../../../../docs/adr/0034-motion-tokens-ease-out-restrained-spring.md); reconciles [196](196-global-animation-toggle-menu-motion.md) accordion scope (note below).

## ACs

- [ ] Tokens only: `--dur-slow:280ms` + `--ease-spring: cubic-bezier(0.34, 1.3, 0.64, 1)` added; `--ease-out`, `--dur-fast:120ms`, `--dur:200ms` unchanged; zero ad-hoc durations/easings (token audit passes).
- [ ] Dialog: in 200ms decelerate / out 100ms accelerate + fade. Chevron: rotate 120ms. Accordion: 200ms opacity/transform — this supersedes 196's `accordions stay instant-open` when Animation is on; off/reduced-motion stays instant.
- [ ] Inputs (textbox/textarea/search): border-color 120ms only — no layout-property animation. Context menu keeps `menu-in` values. Packet-card entrance capped at 300ms.
- [ ] Transform/opacity (+ border-color/color for inputs) only; no `transition: all`; no layout properties anywhere in the six surfaces.
- [ ] Animation off or OS reduced-motion renders every end state instantly (existing switch path, extended to new tokens); focus order, announcements, and 0004-rule-5 feedback preserved.
- [ ] Matrix green: On/Off × reduce on/off, light/dark, real DPI, keyboard-only; frontend check + ownership gate green.

## Implementation notes

- Sources: research [0020](../../../../docs/research/0020-discord-motion-philosophy.md) (Fluent asymmetric menu recipe, bounce ≤0.15); 196 toggle matrix as verification prior art; `Disclosure`/`Dialog`/`ContextMenu`/`PacketCard` current values as the edit list. No new animation library. Claims above are estimates to recheck against code at dispatch (CodeGraph first).

## Verification

- Toggle matrix table pasted in-ticket; property audit (no `all`, no layout props) attached; ownership gate before sync.
