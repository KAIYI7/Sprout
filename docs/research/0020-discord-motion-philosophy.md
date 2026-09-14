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
