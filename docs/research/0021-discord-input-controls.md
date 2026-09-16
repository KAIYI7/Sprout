# Discord input controls — what Sprout adopts

Date: 2026-09-15. Status: research note behind ticket 204. Discord is treated
as direction, not a spec to copy (same stance as research 0020): no
Discord-official input measurements were sourced, so every value below maps
onto existing Ledger tokens per ADR-0028.

## User hypothesis

- Textbox/textarea/dropdown inputs should share one custom look; the current
  outer line reads harsh/next-to-Discord.
- The dropdown's open state renders the OS-default option list, which looks
  out of sync with the app.

## What verifies

- **Focus glow on filled inputs** (research 0020 evidence update): Discord
  FluidUI (`Nightwielder23/discord-fluidui`, real shipped CSS) gives
  message-input and search bars a glow on focus; OpenDesign Discord tokens
  give the focus ring as `0 0 0 3px rgba(88,101,242,0.3)`. Sprout's
  `--ring-glow` is the same idiom in Ledger neutrals — adopted in ticket 202.
- **Filled rest, hairline edge, hover wash** (same sources): Discord text
  entries rest on a filled surface whose edge nearly disappears into the
  background, wash slightly lighter on hover, and spend accent only at focus.
  Sprout's current inputs do the opposite — `bg-page` fill with a
  16%-alpha `border-strong` edge at rest — which is the harsh outer line in
  the report. The fix is staging, not recoloring: rest on the sunken fill
  with the quiet 9%-alpha `border` edge, hover adds the wash every
  row/card/menu already uses (`bg-hover` + `border-strong`), focus keeps
  accent + glow. No new tokens.
- **Dropdowns are custom popouts, not OS lists** (FluidUI: "dropdowns slide
  down"; context menus slide in with the same grammar). A native `<select>`
  popup listbox is rendered by the OS/WebView chrome and cannot be themed to
  the app's tokens, so no amount of trigger styling keeps the open state in
  sync. Sprout already owns the replacement grammar: the shared
  `ContextMenu` (`ContextMenu.svelte`) — surface fill, strong hairline,
  `shadow-dialog`, `menu-in` 120ms rise + settle, checked radio rows,
  anchored placement with `matchAnchorWidth`, keyboard nav, focus return
  (precedent: `DockVisibilityFilter`, the companion site picker, research
  0006 pattern 10 + 0008 rule 3). A select rebuilt on that component is reuse,
  not invention.
- **Option tooltips survive the move**: native `<option title>` becomes the
  menu row's `title` (version policies, winget-less products). Native
  `<optgroup>` grouping has no menu equivalent — it degrades to disabled rows
  carrying the explanation as their tooltip, which keeps the constraint
  visible at the point of choice.

## What Sprout adopts (ticket 204)

- One filled frame for every text-like input (textbox, search, textarea,
  compact row inputs): `bg-sunken` + `border` + `radius-lg`, hover
  `bg-hover` + `border-strong`, focus accent + `ring-glow`. Paint-only
  transitions on the ticket-202 property list. Compact row inputs keep their
  `radius-sm` density.
- `Select` becomes a trigger button in that frame plus a `ContextMenu`
  popout (`align start`, `matchAnchorWidth`), with checked current value,
  disabled-row support, and title tooltips. Trigger keeps the external
  `<label for>` association and a `data-value` readout for tests.
- One reviewed deviation (ADR-0028 slot, ticket 204): the shared menu gains a
  viewport-capped max-height so long option lists (products, sites) scroll
  inside the popout instead of overflowing the window.

## Rules applied

- 0006 pattern 6 (one reserved accent — spent at focus only), 0005 rule 5
  (same-kind controls share one treatment), 0008 rules 3–4 (menu scent,
  shared chrome), 0004 rule 2 (frequency split: quiet rest, wash on hover,
  glow on commitment), ADR-0028 (tokens/components only) + ADR-0034 motion
  tokens (chevron spring, menu-in fast ease-out).

## Evidence update — 2026-09-15: Discord field rhythm + hint-below

Discord is treated as direction, not a spec to copy (same stance as above
and research 0020): no Discord-official spacing measurements were sourced,
so every value below maps onto existing Ledger tokens per ADR-0028.

Observed Discord settings-form idiom (settings panels, channel/role
creation, FluidUI shipped CSS + OpenDesign tokens, same sources as above):

- **Eyebrow label above, never beside.** Uppercase, ~12px semibold, muted —
  Sprout's `field__label` already matches (10px mono uppercase muted).
- **Label → control ≈ 8px.** The label breathes before the box; Sprout's
  `field` internal gap was `space-1` (4px), which glues the 10px eyebrow to
  the frame.
- **Hint below the control, ≈ 8px under it.** Description/help is muted,
  sentence-case, smaller than body, `line-height` ~1.25–1.4, never uppercase,
  never beside the input. Sprout's `field__hint` already sits below in
  `TextInput` + the Quick Action manual fields — the placement is right, the
  4px glue above it is not.
- **Field → field ≈ 20px.** The gap between one field's bottom (hint, or
  input when hintless) and the next label is ~2.5× the internal gap, so
  stacked fields read as separate items. Sprout's `form` stack gap was
  `space-4` (16px) — only 4× the internal 4px, so a name input and the next
  Shell/Command eyebrow visually collide.
- **Disclosures are sections, not fields.** Details/Advanced/Pre-action
  chevron rows carry top breathing plus a divider when open; closed they
  still stand off the previous field. Sprout's `.advanced` had zero own
  spacing beyond the form gap, so the Details chevron read as glued to the
  Files block above it.

## Follow-up — 2026-09-15: the gap has to own the actual parent

The spacing round above shipped but the reported stitching survived in
Quick Action → Manual: the Name box still touched the Shell eyebrow with
0px. Cause: the manual fields are not direct children of `form` — a
plain panel wrapper (`#panel-manual`, from the two-view tab split) sits
between, and flex `gap` only separates direct children, so the 24px stack
gap never reached Name/Shell/Command/Files. Same trap did not exist in
Command/Clip/Product/GroupName (fields are direct `form` children;
Svelte `{#if}` blocks create no element) or in Preset (`.meta` owns its
gap) — verified by markup audit, not assumed.

Fix, same tokens: the panel owns the rhythm (`.manual`: flex column,
`space-5`), per 0005 rule 6 — vertical rhythm is owned by the component
that is actually the parent. Lesson for future tabbed/panel dialogs:
whenever a wrapper is introduced between a stack and its fields, the gap
moves onto the wrapper; styling the outer stack alone changes nothing.

## What Sprout adopts (spacing round)

- `.field` internal gap `space-1` → `space-2` (4px → 8px): label → control
  8px, control → hint 8px. One-token change, no new values.
- `.form` stack gap `space-4` → `space-5` (16px → 24px): field → field 24px,
  the nearest Ledger token to Discord's ~20px (20px would be ad-hoc,
  banned by ADR-0028).
- `.field__hint` normalized everywhere it renders: `margin 0`,
  `text-xs`, `text-muted`, `line-height tight` — the Discord hint voice in
  Ledger type. Placement stays below the control, never beside.
- `.advanced` gains `margin-top: space-1` (4px over the 24px stack gap, so
  the Details/Advanced/Pre-action chevron stands ~28px off the previous
  field) and keeps its dashed `border-top` + `space-3` body padding when
  open. Closed it breathes; open it sections.
- Scope: the shared `TextInput` owner plus every hand-rolled dialog copy
  (`QuickAction/Command/Clip/Product/Preset/GroupName`, Companion
  `site-form`, Settings `knob__body` label→hint). The copies are the
  0005-drift this round fixes; a shared form stylesheet is future work, not
  this ticket.

## Rules applied (spacing round)

- 0005 rule 5 (same-kind controls share one treatment) + 0005 rule 6
  (vertical rhythm owned by the component — changing it once changes it
  everywhere), 0004 rule 2 (frequency split: tight inside a field, loose
  between fields, loosest at sections), ADR-0028 (tokens/components only).
