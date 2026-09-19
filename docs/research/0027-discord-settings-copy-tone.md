# 0027 — Discord settings-copy tone: noun labels, neutral descriptions

Date: 2026-09-17. Status: accepted design (applied to `src/lib/copy/en.json`
the same day; ADR-0028 carries the dated amendment). First-hand support-page
evidence plus secondary brand-voice sources; no user study, no live client
string extraction beyond the quoted support text.

## Question

Sprout's dictionary shipped verb-led labels ("Pick a theme", "Use a window
frame") and second-person descriptions ("You choose where installs land").
The user found the labels redundant beside self-explanatory controls and the
"You ..." descriptions robotic. How does Discord — the reference the copy
round already claimed — actually write settings labels and descriptions?

## Sources

- Discord Support, *Notifications Settings 101* (first-hand quoted UI
  strings) —
  https://support.discord.com/hc/en-us/articles/215253258-Notifications-Settings-101 —
  row labels are noun phrases ("Suppress @everyone and @here", "Suppress
  Highlights", "Mute New Events", "Mobile Push Notifications"); row
  descriptions are third-person present-tense statements about what the
  setting does ("Prevents notifications from @everyone and @here mentions
  across the entire server.", "Controls email notifications for the best
  content from servers you follow.", "Receive notifications for every
  message", "Disable all notifications", "Silence everything including
  @mentions"). Option labels are single words ("All", "Mentions", "Nothing",
  "Mute"). Navigation labels are 1–3 short words ("User Settings >
  Notifications").
- Discord official blog, *How to Manage Your Discord Desktop Notifications*
  (2026-06-18) — https://discord.com/blog/how-to-manage-your-discord-desktop-notifications —
  settings rows referenced as bare nouns ("Enable Desktop Notifications",
  "Sounds", per-sound toggles "Message", "Mention", "Deafen", "Mute").
- Discord Brand Guidelines 2021 deck, slides 11–12 (secondary transcription)
  — https://www.deck.gallery/discord-brand-guidelines-2021/slide/11-brand-values-tone-of-voice/ —
  three messaging tiers: brand lines, community quotes, and product-value
  copy. Settings copy is the third tier, not the playful brand tier; voice
  pillars (playful, original, relatable, reliable) shape marketing, while
  product-value copy stays straightforward. Corroborated by Tonelab's brand
  summary: statement-style sentences, present tense, straightforward
  expression — https://www.tonelab.so/brand/discord
- Do Words Good, *Discord copywriting & messaging case study* (2026-02-02,
  secondary) — https://dowordsgood.com/discord-copywriting-messaging-case-study/ —
  clarity-first rules from inside Discord's own content practice: short
  sentences, clear verbs, one idea per piece, strong headings for skimming,
  practical steps over vibes. Cut "clever" lines that hide the point.
- Standing repo evidence: research 0014 findings 2–3 (Notion/Discord labels
  are 1–3 short words, never sentences; Apple "omit unnecessary words"),
  0004 rule 4 (tab hygiene: labels of 1–3 words), 0017 (Settings helpers must
  keep constraints, defaults, validation, and consequential behavior while
  shortening).

## Findings

1. **Labels are nouns; the control carries the verb.** Discord never labels a
   row "Pick your theme" — the label names the thing ("Appearance",
   "Notifications", "Suppress Highlights") and the adjacent control
   (toggle/select/radio) supplies the action. A verb-led label beside a
   self-explanatory control repeats information twice.
2. **Descriptions are neutral third-person statements, ordered
   what-then-constraints.** "Prevents notifications from X across the entire
   server" states the effect first; scope and consequences follow. No
   second-person lead ("You ...") appears in row descriptions; second person
   survives only mid-sentence where the user is genuinely the subject
   ("When you're signed in on your Android or iOS device, you'll get push
   notifications ...").
3. **Action tooltips stay imperative.** Icon-only buttons keep verb-led
   tooltips ("Open Sprout", "Dock to the left edge") — the tooltip *is* the
   control's verb (0004 rule 4: icons require tooltip + `aria-label`; Vercel
   icon-button guidance in 0008 sources). Finding 1 governs titles, not
   button tooltips.
4. **Fixed vocabularies stay fixed.** Presence states and the seam reason are
   wording-locked by ADR-0033 and tests; tone changes do not touch them.

## Rules for Sprout (applied to `src/lib/copy/en.json` 2026-09-17)

1. **Every Settings label is a concise noun** ("Theme", "Animation",
   "Window frame", "Auto-start", "Managed setup") — never a verb-led
   sentence. Same-category labels elsewhere follow the same rule.
2. **Every description is a terse neutral statement**: what the setting does
   first, then ranges/defaults/consequences; no "You ..." lead. Constraints,
   defaults, validation, and non-obvious consequences from 0017 are
   preserved, shortened — never dropped.
3. **Hints stay ≤140 characters**; ticket 33's bans hold unchanged.
4. **Action tooltips remain verb-led**; presence/seam strings unchanged.
5. This supersedes the spec-214 voice rule (2nd person + verb-led labels)
   recorded for ticket 219 — warmth now comes from brevity and plain words,
   not from addressing the user.
