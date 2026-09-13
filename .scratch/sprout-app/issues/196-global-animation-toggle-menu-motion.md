# 196 — Global Animation switch with minimal menu motion

**What to build:** One app-wide Animation switch that governs every motion surface, plus the single justified addition (menu fade) it gates.

**Blocked by:** None — can start immediately.

**Status:** complete — all ACs closed (batch-152-153-194-195-196-20260913, combined validation 2026-09-13).

**Parent:** [188 — Managed local lifecycle UX](188-managed-local-lifecycle-ux-spec.md).

## ACs

- [x] Settings → General switch beside Theme (switch with On/Off word, immediate, never dirty, indexed in settings search); new persisted Settings key; store/`data-` hook forcing the reduce path regardless of OS; default On.
- [x] Scope: Dialog fade gap closed (no JS fade when off), `ctx-menu/submenu opacity/scale var(--dur-fast) var(--ease-out)` (instant when off), chevron rotate, packet entrance, three infinite pulses → frozen ring/dot with `0004 rule 5` feedback preserved, future progress-bar width under same tokens.
- [x] Tokens-only (`--dur-fast/--dur/--ease-out`), no new durations/easings/components; OS `prefers-reduced-motion` still honored; accordions stay instant-open.
- [x] No page-features-menu or moment-of-use placement (app-global per `0008 rule 1`).

## Verification

Toggle matrix (On/Off × OS reduce on/off): menus/dialogs/pulses instant when either off, tokens-accurate when both on; search finds the switch; Settings suites + frontend check + ownership gate green.

## Implementation notes

Estimates to recheck at dispatch: Settings General group, settings persistence/search, tokens, `ContextMenu`/`Dialog`/`Disclosure`, pulse sites. Supplies the motion contract for 190/193 progress transitions.
