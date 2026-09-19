# 219 — Copy voice guide + en.json dictionary + wiring

**Parent:** [214 — Bezel-tab dock mode + copy/i18n (spec)](214-bezel-tab-dock-mode-copy-i18n-spec.md)

**What to build:** Every Settings label/hint, dock tooltip, and presence string rewritten in a human Discord/Notion voice and served from one file-based English dictionary — so identical text is reused by key and translation later is a file swap, never a hunt.

**Blocked by:** None — can start immediately (content work; key names for bezel consumers pinned in spec 214: e.g. `dock.bezel.tooltip`).

**Status:** applied + validated 2026-09-16 (`npm run check` 0 errors, copy tests 16/16, ownership gate pass); revalidated 2026-09-17 (see note)

## Revalidation — 2026-09-17 (noun titles + Discord-tone descriptions)

At the user's direction: every Settings label is now a concise noun
("Theme", "Animation", "Window frame", "Auto-start", "Managed setup") and
every description a terse neutral statement with no "You" lead, following
first-hand Discord evidence in research 0027 (ADR-0028 amendment
2026-09-17; spec-214 2nd-person/verb-led rule superseded). Keys, wiring,
presence/seam locked strings, action tooltips, 140-char cap, and ticket 33's
bans are unchanged — translation is still a file swap.

- [x] Voice rules recorded (2nd person, specific verb-led labels, front-loaded keywords, ≤140 chars per hint, no passive robot voice, no fancy words); ticket 33's bans hold (no "Sprout never…", no wordplay, single "Loading…")
- [x] New `src/lib/copy/en.json` keyed `section.key`, plus a loader with EN fallback for missing keys; `docs/CONTEXT.md`: add **copy dictionary** with `planned` qualifier + spec-214 link
- [x] V1 scope rewritten through the dictionary: Settings `knob__label` + `knob__hint` set, dock tooltips (incl. `dock.bezel.*`), presence `details`/`state` strings (ADR-0033 allowlist unchanged — wording only)
- [x] Dedup pass: no inline duplicate of a keyed string remains in V1 scope — same exact text reuses the key
- [x] Errors/onboarding/empty states untouched (follow-up, per spec); nothing stored in the database (files only — machine-local boundary kept)
- [x] Tests for loader fallback (missing key → EN) + key-coverage check over V1 scope + `npm run check` 0 errors
- [x] `node tools/ownership-gate.mjs` passes (new copy-dictionary seam recorded, single consumer pair EN/zh-CN)

**Explicitly not built:**

- Bezel behavior or Settings controls (tickets 215–218 — they consume these keys); Chinese translation (ticket 220 — scaffold only arrives there)
