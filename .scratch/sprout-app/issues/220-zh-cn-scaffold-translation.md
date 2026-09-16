# 220 — zh-CN scaffold + fallback + translation (after EN)

**Parent:** [214 — Bezel-tab dock mode + copy/i18n (spec)](214-bezel-tab-dock-mode-copy-i18n-spec.md)

**What to build:** Simplified Chinese as a pure file swap: the loader resolves `zh-CN` keys with English fallback, the language switch is wired in Settings, and every V1 English string ships translated — only after EN is final, so translation sources one frozen text.

**Blocked by:** [219 Copy voice guide + en.json](219-copy-voice-guide-en-dictionary.md) (EN keys frozen).

**Status:** ready-for-agent

- [ ] `src/lib/copy/zh-CN.json` scaffold ships with TODO values for every 219 key (no new keys of its own); loader falls back key-by-key to EN with a coverage test proving no blank renders
- [ ] Language switch in Settings (persisted preference, machine-local like theme); documented whether it applies live or needs restart — one behavior, stated in the hint
- [ ] Full Simplified-Chinese translation of all 219 V1 keys, reviewed against the voice rules (natural, plain, no machine-translation artifacts)
- [ ] Switching to zh-CN and back leaves no missing-key gaps; whole-app backup behavior unchanged (preference travels like other Settings, copy files ship with the app)
- [ ] `npm run check` 0 errors + `node tools/ownership-gate.mjs` passes; no ADR text changes

**Explicitly not built:**

- New copy or new keys (belongs to 219's scope); additional languages (same pattern later, not here); database-backed storage (never — files only)
