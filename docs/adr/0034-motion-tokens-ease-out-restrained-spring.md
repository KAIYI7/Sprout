# Motion tokens: ease-out dominance with restrained spring

> Status: accepted 2026-09-14.

Sprout adopts a full-app motion-token extension behind ADR-0028: keep `--ease-out` dominance and 120/200ms micro budgets, add `--dur-slow:280ms` for large dialogs/sheets and `--ease-spring: cubic-bezier(0.34, 1.3, 0.64, 1)` (subtle overshoot) for interruptible arrivals only. Easing carries system announcements (dialog open/close, menus); spring carries chevron/accordion/toast arrival. Micro stays 100–200ms, large 250–300ms, transform/opacity (plus border-color for inputs) only, with `prefers-reduced-motion` and the Animation off-switch collapsing to instant. Visual design is unchanged; the dock driver still owns dock geometry per ADR-0019.

## Considered options

Spring-everywhere (including dialogs and dock slides) was rejected: dialogs are announcements, not gestures, and dock geometry belongs to the poll driver — springs there fight interruption handling and ADR-0019 ownership. Copying pasted Discord numbers verbatim was rejected: no primary Discord engineering source was found, so Discord is direction (research 0020), not a spec.

## Consequences

- `tokens.css` gains two tokens; v1 scope is `Dialog` (in 200ms decelerate / out 100ms accelerate + fade), `Disclosure` chevron 120ms, `GroupAccordion` 200ms, inputs 120ms border-color only, `ContextMenu` `menu-in`, `PacketCard` capped at 300ms. No new animation library v1.
- Prerequisite validation (`detect` under `windows_execution/`, honest `Not verifiable` when offline/uncertain) and the CPU offload audit (`compute_plan`, backup inspect/import, logs, settings-search index to `spawn_blocking`; JS keeps render only) ride as implementation tickets under this round's consensus, not as separate ADRs.
