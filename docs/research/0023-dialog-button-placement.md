# 0023 - Dialog button placement: footer-right vs field-below, never full-width

Date: 2026-09-15. Status: research + recommendation behind the user
question; code changes in this round cover the AI-field stitching and the
dock-flag copy only — placement itself already matches the rule below, so
no button moved sides.

Discord is treated as direction, not a spec to copy (same stance as
research 0020/0021/0022): no Discord-official button-spec was sourced, so
every adoption maps onto existing Ledger tokens and the shared `Button`
foundation per ADR-0028.

## User hypotheses

1. Field-attached buttons (Generate, Diagnose, Attach, Test, Check) — should
   they sit on the right of their row, or below the control?
2. When the row is otherwise empty (no files, no outcome, single Generate),
   should the button stretch full width?
3. Discord pattern for the above.

## What verifies

- **Dialog footers go right.** Sprout's `.form__actions` (`justify-content:
  flex-end` — Cancel + Save/Add bottom-right) already matches the Discord
  modal footer idiom (Create Channel: Cancel + Create bottom-right;
  corroborated by the 0022 walkthrough sources showing "then hit Next" /
  "select Create Channel" as the footer's confirm). NN/g modal-dialog
  guidance keeps the confirm at the footer's end of the reading flow.
  No change: `QuickAction/Command/Clip/Product/Preset/GroupName` footers
  stay right-aligned.
- **Field-attached actions go below, left-aligned.** Generate belongs to the
  Describe textarea, Diagnose to its error textarea, Attach/Insert to Files,
  Test/Check to their probes. Research 0006 pattern 4 (contextual controls
  live near their object) plus 0005 rule 6 (rhythm owned by the actual
  parent) place them as the last child of their `.field` (8px internal gap),
  never as a detached `.ai`/`.form` sibling (24px stack gap) and never
  right-aligned away from the field's left reading edge. Right is reserved
  for the dialog footer — spending it twice per dialog breaks the one-path
  scan (label → control → action → next field).
- **Never full-width when empty.** A full-width secondary reads as the
  dialog's primary (0006 pattern 6: one reserved accent, one primary per
  view per 0005 rule 2). All field actions here are `secondary`/`ghost`;
  the only primary in these dialogs is the footer Save/Add. Full-width
  would also shift layout on first content (Attach jumps when the first
  file lands; Generate jumps when the outcome appears) — stability beats
  emphasis for an empty state. Discord agrees in practice: empty surfaces
  keep their buttons in place (Create Channel stays footer-right with zero
  members picked; member tickboxes stay checkboxes, 0022).
- **The reported AI gap was real, and it was nesting, not side.** `.ai`
  `gap: space-5` separates *fields* (24px). Describe's textarea, the
  "Stopped — Generate will restart." status, and Generate were three
  siblings under `.ai`, so 24px + 24px = ~48px stranded the button from
  the textarea it belongs to. Fix (this round): the Describe `.field`
  owns all three (label → textarea → status → actions, 8px internal),
  and Diagnose owns its own the same way. Side unchanged (left-below);
  status voice drops to `field__hint` so it reads as field help, not a
  section break.

## Related validations (same round, existing notes stand)

- **Shell hint ownership (extends 0021, no new rule).** The
  "PowerShell runs with…" line is a DOM child of the Shell `.field`, so it
  belongs to the dropdown — and 0021's evidence update already validates
  below-control placement (Discord: eyebrow label → control ≈8px → hint
  below ≈8px, never beside, never below-title). The user's "right below
  the title" memory is the label side, not the hint side. The defect was
  copy, not placement: "Multi-line is fine." describes the Command
  textarea, not the shell. Fix: Shell hint keeps execution semantics only
  ("Runs with -NoProfile -NonInteractive." / "Runs as: cmd /c …");
  multi-line is already evident from the 6-row textarea.
- **Checkbox voice (extends 0022, no new rule).** All three dialogs already
  share one `Checkbox` component (0022 verdict 3 delivered) — the
  divergence was prop choice, not component. Validated rule: short
  consequence inline (`hint`), long mechanics behind the InfoTip (`info`).
  Command's dock flag was the correct short case; Quick Action's and
  Clip's dock flags said the same thing through an InfoTip. Fix: both
  switch to `hint="Uncheck to keep it in the main app only."` — identical
  treatment per 0005 rule 5. Stoppable and auto-run keep their InfoTips
  (long, dependency-bearing — Apple HIG hierarchy case cited in 0022).

## Follow-up — 2026-09-15: one missed dock copy + hint/info coexistence

The dock-flag unification missed a fourth copy: the image-clip form in
`src/routes/clips/+page.svelte` carries its own `Show in dock` Checkbox
with the old InfoTip voice. Same fix (hint-inline, no InfoTip) — the
"same component" covers the box, never the per-instance props, so every
copy has to be converted individually; grep for `<Checkbox`, not the
component file. Separately, `Show Stop button` and `Run at Sprout start`
keep their InfoTips (long, dependency-bearing) and gain a one-line `hint`
summary alongside — the component renders both (hint inside the label
hit-target, InfoTip as its sibling trigger), so every Details row now
shares the title + hint-sub-line anatomy while long mechanics stay behind
ⓘ. 0005 rule 5 holds both ways: same flag same voice across dialogs,
same row anatomy within a dialog.

## Rules applied

- 0006 pattern 4 (controls near their object) + pattern 6 (one reserved
  accent / one primary), 0005 rules 2/5/6 (one primary, same-kind same
  treatment, rhythm owned by the actual parent), 0004 rule 2 (frequency
  split: tight inside a field, loose between fields), 0008 rule 2
  (submit-deferred stays checkbox — untouched), ADR-0028 (tokens/components
  only) + ADR-0034 motion tokens (no new durations; nesting change only).

## Follow-up — 2026-09-15: restart hint removed

The `Stopped — Generate will restart.` status line described above was
removed (ticket 192 amendment): Generate restarts a stopped runtime on
use, so the hint stated the obvious — the user discovers it by doing.
The Describe `.field` now owns label → textarea → actions only. The
nesting fix stands unchanged; only the status child is gone, with its
`aiRuntimeStopped` prop and the caller's liveness read deleted alongside
it (deletion test: the chain existed solely for that line).
