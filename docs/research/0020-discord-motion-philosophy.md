# Discord motion philosophy — what Sprout adopts

Date: 2026-09-14. Status: research note behind ADR-0034. Hypotheses from the user prompt verified against primary sources; Discord-official curve numbers were not found, so Discord is treated as direction, not a spec to copy.

## User hypothesis

- Spring physics over linear timing; elements bounce slightly on arrival.
- Ease-out dominance, e.g. `cubic-bezier(0.215, 0.610, 0.355, 1.000)`: explode into motion, decelerate into place.
- Micro 100–200ms (hover, server-list expand); large 250–350ms (settings menu).
- Related: Discord icon animations (`share.google` link — Google-share redirect, unresolvable; canonical URL still needed).

## What verifies

- `cubic-bezier(0.215, 0.61, 0.355, 1)` is standard `easeOutCubic` (bendc `easing.css` gist; richtabor `motion-design/references/easing-tokens.md`; MDN `cubic-bezier`). Sprout `--ease-out: cubic-bezier(0.22, 1, 0.36, 1)` (`src/lib/styles/tokens.css:96`) is already ease-out dominant, slightly stronger (near `easeOutQuint`). Direction holds; no curve replacement needed.
- Micro 100–200ms / large 250–350ms matches Fluent (`durationFaster 100ms`, `durationNormal 200ms`; menu open 200ms decelerate / close 100ms accelerate), richtabor (`--dur-1 120ms` micro, `--dur-4 300ms` upper bound), Apple/macOS springs (system animations top out ~0.35s; `.snappy` bounce ~0.15). Sprout `--dur-fast:120ms`, `--dur:200ms`, `Dialog.svelte:150` fade 140ms already fit. Gap: no large token; `PacketCard.svelte:153` `rise 360ms` exceeds budget.
- Signature Discord move per opendesigner Discord `DESIGN.md`: server avatar rounded-square → circle on hover. Maps to Sprout chevron/hover micro, not a full motion spec.
- Reduced-motion + off-switch already in `tokens.css:326-349` (`prefers-reduced-motion`, `html[data-animation="off"]`); `Disclosure.svelte:68` chevron is transform-only; `animation.svelte.ts` store gates `Dialog` fade. Keep.

## Spring vs easing — the split that matters

Primary sources agree (userinterface.wiki `to-spring-or-not-to-spring`; Apple HIG motion via macOS Sequoia research 2026-04-05 / WWDC23 sessions 10157–10158; Emil Kowalski `emil-design-eng`; `react-spring` docs; Android AEP physics-motion guideline 2026-08-26):

- Springs: gesture-attached, interruptible motion (drag, chevron toggle spam, toast stack). They preserve velocity and retarget mid-flight; easing restarts from zero.
- Easing: system announcements (dialog open/close, menu open, settings-panel enter). Fixed duration + decelerate-in / accelerate-out reads as intentional.
- Adopt: easing by default; subtle spring (bounce 0–0.15) only for chevron/accordion/toast arrival. Never spring dialogs or dock geometry (ADR-0019 driver owns dock slide ~180ms).

## What does not matter / what to skip

- Exact Discord server-list numbers: no primary Discord engineering source found; do not copy the pasted paragraph verbatim.
- Bouncy presets (`bounce ~0.3`), layout-animating properties (width/height/top/left), `transition: all` (already banned by `quickActionDetails.test.ts:72`). Animate transform/opacity (and border-color/color for inputs) only.
- New springs library v1: CSS `cubic-bezier` with slight overshoot (`0.34, 1.3, 0.64, 1`) covers the arrival bounce; Svelte `spring` reserved for a later interruptible-drag ticket if needed.

## Adopted token delta (ADR-0034)

- Keep `--ease-out`, `--dur-fast:120ms`, `--dur:200ms`. Add `--dur-slow:280ms` (large dialogs/sheets) and `--ease-spring: cubic-bezier(0.34, 1.3, 0.64, 1)` (subtle overshoot, chevron/accordion/toast only).
- Asymmetric open/close: enter 200ms decelerate, exit 100ms accelerate (Fluent menu recipe). Dialog: in 200ms / out 100ms + fade; chevron rotate 120ms; accordion 200ms opacity/transform; inputs border-color 120ms only.
- Scope v1: `Dialog`, `Disclosure` + `GroupAccordion`, `TextInput`/`SearchInput`/textarea, `ContextMenu` (`menu-in`), `PacketCard` (cap 300ms). Visual design (colors/type/radii) unchanged per ADR-0028. Dock driver untouched per ADR-0019.

## Sources

- Apple HIG motion / macOS Sequoia motion reference (WWDC23 10157–10158), via macOS-app motion skill note 2026-04-05.
- Raphael Salaja, `userinterface.wiki/to-spring-or-not-to-spring` (2025-12-29).
- Emil Kowalski `emil-design-eng` SKILL.md (ease-out `cubic-bezier(0.23,1,0.32,1)`; spring bounce 0.1–0.3; transitions over keyframes for interruptibility).
- `react-spring` docs (`react-spring.dev`, `pmndrs/react-spring`); Motion spring-physics reference; `kinetics` spring gallery.
- Android AEP physics-based-motion guideline (2026-08-26); Fluent 2 motion (curves/durations); Carbon motion (`cubic-bezier(0.2,0,0.38,0.9)` productive etc.); richtabor easing tokens; bendc `easing.css`; MDN `cubic-bezier`.
- OpenDesign Discord `DESIGN.md` (server-avatar morph); Carbon/Fluent duration recipes.
- Sprout source: `tokens.css:95-98,326-349`, `Dialog.svelte:150`, `Disclosure.svelte:67-73`, `PacketCard.svelte:86,153`, `ContextMenu.svelte:459`, `animation.svelte.ts`, ADR-0019, ADR-0028.

## Evidence update — 2026-09-15: expressive Discord motion + the progressive frame

Ticket 202's first rollout read as too subtle, so the Discord direction was
re-researched against primary, fetchable sources (no Discord-official curve
numbers exist — direction, not a spec, per the note above).

- **Discord FluidUI** (`Nightwielder23/discord-fluidui`, real shipped CSS):
  GPU-only motion (transform/opacity everywhere, highlights in neutral white
  washes so they layer over any theme); entrances everywhere (context menus
  slide in, popouts/modals scale up with soft overshoot, pickers rise,
  dropdowns slide down, tooltips pop); hover micro-interactions (server icons
  spring-scale, rows gain scale + background wash, buttons grow on hover and
  press in on click); **focus glow** (message-input and search bars glow when
  focused); reduced-motion collapses everything to near-zero.
- **OpenDesign Discord tokens** (`open-design.ai/plugins/design-system-discord`):
  `--motion-fast 80ms`, `--motion-base 200ms`,
  `--ease-standard cubic-bezier(0.2,0,0,1)`, focus ring
  `0 0 0 3px rgba(88,101,242,0.3)`; server avatars morph rounded-square →
  circle on hover — expressiveness from shape/state change, not duration.
- **Micro-interaction references** (Rune Hub 2026-06-28; dylantarre
  `animation-principles`; christophacham `agent-skills-library`): hover 100ms,
  toggle 150–200ms spring, press `scale(0.97–0.98)`, button hover
  `translateY(-1px)` + shadow, input focus = border-color + soft ring
  transitioned together, error shake 3–5px; reduced-motion keeps opacity fades
  only.

### What Sprout adopts (enhancement, still token-only)

- **Announcements gain entrances, not durations**: dialogs fade + rise +
  settle-scale (200ms out-decelerate / 100ms accelerate, ease-out — the
  asymmetric recipe stands); menus slide + settle-scale in 120ms. New
  durations: none. New easings: none (spring stays chevron/accordion/toast —
  the chevron now actually uses it).
- **Hover is anticipation, focus is commitment** (0004 rule 2's frequency
  split applied to chrome): input frames rest quiet, wash faintly on hover,
  and glow on focus with the ring transitioned alongside the border — the
  Discord focus-glow in Ledger neutrals, accent spent at focus only (0006
  pattern 6). Press-in (`translateY(-1px)` scale) lands on packet cards only.
- **The frame fits the Ledger by staging, not recoloring**: rest keeps the
  neutral 1px frame, hover adds the wash every row/card/menu already uses
  (`--bg-hover`; 0005 rule 5 same-kind treatment), focus keeps accent + glow.
  No new tokens, no ad-hoc colors.
- **Property audit stays paint-only**: transform/opacity + border-color /
  background-color / color / box-shadow; layout properties and
  `transition: all` remain banned. The input list grows from border-color-only
  to border + wash + glow continuity — recorded as the ticket-202 deviation
  with this evidence, not a silent widening.
