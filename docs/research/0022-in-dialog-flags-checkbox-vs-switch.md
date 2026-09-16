# In-dialog flags: checkbox vs radio vs switch — what Discord actually does

Date: 2026-09-15. Status: research + recommendation behind the user
question; no code changed, implementation pending a build decision.
Discord is treated as direction, not a spec to copy (same stance as
research 0020/0021): Discord publishes no official control-spec, so the
Discord claims below rest on corroborated third-party walkthroughs, and
every adoption maps onto existing Ledger tokens per ADR-0028.

## User hypothesis (with one correction)

- Discord's Create Channel dialog uses an animated control plus a
  "Create channel" button instead of checkboxes — so Sprout's
  Quick Action Details flags (Show Stop button, Run at Sprout start,
  Show in dock) should use the same, and Sprout's current checkboxes
  look uncustomized next to the rest of the app.
- Correction: the animated control in Create Channel is a **toggle
  switch** ("Private Channel", right side of its row), not a radio
  button. Discord's radios live one step earlier — channel *type*
  (Text / Voice / Announcement). Member/role picking in the same flow
  uses tickboxes (checkboxes). Sources below.

## What Discord actually uses (one dialog, three controls, each correct)

- **Radios for channel type** — Text / Voice / Announcement are mutually
  exclusive, exactly-one-selected: the textbook radio slot
  (https://www.webproeducation.com/how-to/create-channel-on-discord-server/,
  2025-03-05).
- **Animated switch for Private Channel** — "click the grey toggle
  button next to 'Private Channel' to turn it green… then hit Next"
  (https://allthings.how/how-to-create-and-set-up-a-discord-server/,
  2022-05-05); "select the Private Channel toggle and select Create
  Channel" (https://www.twilio.com/en-us/blog/send-receive-sms-messages-discord-twilio-node-js,
  2022-08-11). The switch is custom-animated (slide + grey→green);
  no Discord-official measurements exist.
- **Checkboxes for members/roles** — "check the tickbox next to the
  necessary members and/or roles… When you're done, click 'Create
  Channel'" (same allthings.how walkthrough).
- **Confirm button applies everything** ("Create Channel" / "Next") —
  including the switch. Discord's own switch is submit-deferred: flipping
  Private Channel changes nothing server-side until Create is clicked
  (corroborated by the API-docs discussion showing the toggle is UI over
  a permission overwrite sent with the create request:
  https://github.com/discord/discord-api-docs/discussions/6361).

## Verdict 1: radio buttons are ruled out — unanimously

- NN/g, *Checkboxes vs. Radio Buttons* (2004, still the cited standard):
  "Radio buttons are used when there is a list of two or more options
  that are mutually exclusive and the user must select exactly one
  choice… Checkboxes are used when… the user may select any number of
  choices" (https://www.nngroup.com/articles/checkboxes-vs-radio-buttons/).
  NN/g 2024 checkboxes guidance repeats it: "each checkbox functions
  independently" (https://www.nngroup.com/articles/checkboxes-design-guidelines/).
- Apple HIG, *Toggles*: "Prefer a set of radio buttons to present
  mutually exclusive options. If you need to let people choose multiple
  options in a set, use checkboxes instead… To present a single setting
  that can be on or off, prefer a checkbox"
  (https://developer.apple.com/design/human-interface-guidelines/toggles).
- Material M1 selection controls: "Radio buttons allow the selection of
  a single option from a set" vs "Checkboxes allow the selection of
  multiple options from a set"
  (https://m1.material.io/components/selection-controls.html).

Sprout's three flags are independent booleans with 8 valid combinations
(stop × auto-run × dock). Radios would need six (an on/off pair per
flag) and would promise mutual exclusion that does not exist. This is
NN/g's "mistake #1" pattern (checkboxes where radios belong, inverted):
the controls must *predict* multi-select, and radios predict the
opposite. Discord agrees in practice — its radios are reserved for the
actually-exclusive channel-type choice.

## Verdict 2: switches are ruled out for these three — same rule as 0008

Research 0008 rule 2 already refused a checkbox → toggle conversion for
exactly these flags (stoppable, auto-run): they apply on Save, and a
switch promises immediacy. Current primary sources re-confirm that call:

- Material M3, *Switch*: "The effects of a switch should start
  immediately, without needing to save" and "Avoid using a switch to
  select multiple options that require people to save. Switches should
  be immediate. Use checkboxes instead"
  (https://m3.material.io/components/switch/guidelines). (M1's
  "single option → switch" line is superseded by M3's immediacy rule;
  M3 is the current guidance.)
- NN/g checkboxes-vs-radio guideline 11: "the changed settings should
  not take effect until the user clicks the command button" — the
  dialog-checkbox contract our Save button already keeps.
- Apple HIG, *Toggles*: "In general, don't replace a checkbox with a
  switch" — plus the hierarchy clause that fits our exact layout: "Use
  a checkbox instead of a switch if you need to present a hierarchy of
  settings… you can show dependencies, such as when the state of a
  checkbox governs the state of subordinate checkboxes." Show Stop
  button governs the subordinate Stop command field; it is the cited
  case, almost verbatim.
- Discord's Private Channel switch does not rescue the switch case: per
  the evidence above it is itself submit-deferred (applies on Create),
  i.e. Discord deviates from the immediacy rule here rather than
  demonstrating it. It also has an immediate *in-dialog* effect
  (revealing the member list / flipping the button to Next) that our
  flags lack — except stoppable's reveal, which the checkbox already
  handles.

## Verdict 3: the valid grievance is styling, not control type

"Checkboxes not customized to fit the app" verifies: the three flags
(plus Command/Clip/Product dialogs' equivalents) render the native OS
checkbox with only `accent-color: var(--accent)` — the one unstyled
control family left after tickets 202/204 themed inputs, selects, and
menus (research 0021). The fix that keeps every rule above is a shared,
Ledger-styled **checkbox** component — square box, custom checkmark,
accent spent on the checked state only (0006 pattern 6), `ring-glow`
focus to match inputs (0021), label-wrap single hit target preserved
(web-design-guidelines anti-pattern list: label + control share one hit
target, no dead zones), native `input[type=checkbox]` underneath so
keyboard/SR behavior stays free.

## Recommendation (pending build decision)

1. Keep checkbox semantics for Show Stop button, Run at Sprout start,
   Show in dock (and their Command/Clip/Product siblings). No radios
   (wrong exclusivity promise), no switches (wrong immediacy promise).
2. When built: one shared `Checkbox`/`CheckRow` component in
   `src/lib/components/` (title + hint + InfoTip slots, mirroring the
   current `.stoppable` anatomy), rolled out to every dialog copy per
   0005 rule 5 — the same copy-fanout the 0021 spacing round did.
3. Amends nothing: this corroborates 0008's applied-history refusal
   (2026-09 round) with current Apple/Material/NN/g sources and adds
   the styling half the original question raised.

## Rules applied

- 0008 rules 1–2 (classify the knob: submit-deferred in-dialog flags
  are checkboxes, not switches; decision already recorded, now
  corroborated), 0005 rule 5 (same-kind controls share one treatment —
  the future shared component), 0006 pattern 6 (accent reserved for
  checked/focus state), NN/g checkboxes-vs-radio #2/#3/#11 + Apple HIG
  Toggles + Material M3 Switch (primary-source semantics).

## Decision update — 2026-09-15: styled Checkbox delivered

The styling half is built: one shared `Checkbox` component in
`src/lib/components/` (Ledger tokens only — sunken rest, hover wash,
accent fill on checked, `ring-glow` focus matching text inputs, animated
tick, native `input[type=checkbox]` underneath), rolled out to every
in-dialog flag copy per 0005 rule 5: Quick Action (Show Stop button, Run
at Sprout start, Show in dock), Command (Show a window, Show in dock),
Clip and image-clip (Show in dock). Semantics unchanged — still
checkboxes, still submit-deferred; no switch conversion. Selection-list
checkboxes (Plan in-run, export picker, Settings includes, Preset
dependency chips) stay native pending their own round.
