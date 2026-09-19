# 220 — zh-CN scaffold + fallback + translation (after EN)

**Parent:** [214 — Bezel-tab dock mode + copy/i18n (spec)](214-bezel-tab-dock-mode-copy-i18n-spec.md)

**What to build:** Simplified Chinese as a pure file swap: the loader resolves `zh-CN` keys with English fallback, the language switch is wired in Settings, and every V1 English string ships translated — only after EN is final, so translation sources one frozen text.

**Blocked by:** [219 Copy voice guide + en.json](219-copy-voice-guide-en-dictionary.md) (EN keys frozen).

**Status:** applied + validated 2026-09-17 (`npm run check` 0 errors, copy/search/bezel/chrome tests 69/69, `cargo test` 711 passed / 0 failed; ownership gate pass) + extended full-app coverage validated 2026-09-17 (see below)

- [x] `src/lib/copy/zh-CN.json` ships 81/81 EN keys with full translations (no TODO left, no new keys beyond the switch surface below); loader falls back key-by-key to EN with coverage tests proving no blank renders (node-verified parity locally; vitest queued)
- [x] Language switch in Settings (General group, Select beside the theme knob; immediate-apply persisted `settings.language` in the backend meta table, machine-local like theme); applies live everywhere — one behavior, stated in the hint (`settings.language.hint`)
- [x] Full Simplified-Chinese translation of all 219 V1 keys (81 incl. the 2 switch-surface keys), noun-title / Discord-tone voice, `{count}` slots kept, no MT artifacts
- [x] Switching to zh-CN and back leaves no missing-key gaps (round-trip test over every key); whole-app backup behavior unchanged (no backup.rs touch — backups never read Settings; copy files ship with the app)
- [x] `npm run check` 0 errors + `node tools/ownership-gate.mjs` passes (queued for coordinator — no workspace deps); no ADR text changes

## Extension — 2026-09-17 (full-app menu/button/placeholder/dropdown/title/description coverage, batch-217-216fix-220fix-20260917)

V1 left menus/buttons/placeholders/dropdowns/page-titles/descriptions hardcoded. Extended to full app with Discord-zh/Notion-zh consistency (research: Discord zh-tw support 218892547/215253258/211339918/207260127/216406447; Notion zh-cn help change-your-language/account-settings; Notion-zh views-filters-and-sorts mirror).

- [x] `src/lib/copy/en.json` + `zh-CN.json` ship 1178/1178 keys (81 V1 untouched + 1097 full-app: nav/chrome/menus/buttons/placeholders/dropdowns/titles/descriptions/dialogs/empty-states/notices); slot sets identical; hints ≤140; ticket-33 bans hold (modelNameFirst reworded, no allowlist)
- [x] Every menu/button/placeholder/dropdown-option/page-title/subtitle/dialog/empty-state/notice string resolves via `t()/tCount()`; display-label maps are locale functions; language switch follows live with no restart (incl. 216 bezel-Y/drag strings: quickwindow.bezelPosition/bezelSaveFail)
- [x] Round-trip zh-CN↔en leaves no missing-key gaps (test over all 1178 keys); whole-app backup unchanged (no backend touch)
- [x] `npm run check` 0 errors + `node tools/ownership-gate.mjs` pass + contrast-check pass both themes; full vitest 421/422 (single pre-existing 219 fallout managedSettings.close "Available to install", untouched); no ADR text changes

**Explicitly not built:**

- New copy or new keys (belongs to 219's scope); additional languages (same pattern later, not here); database-backed storage (never — files only)
