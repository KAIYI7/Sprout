// Copy voice + dictionary seam (spec 214; research 0027; ADR-0028 amendment
// 2026-09-17; ADR-0033 wording-only for presence).
//
// Voice rules every value follows: concise noun titles ("Theme", not "Pick a
// theme" — the control beside it carries the verb), Discord-tone descriptions
// (terse neutral statements: what the setting does, then its constraints and
// consequences; no second-person lead), front-loaded keywords in hints, at
// most 140 characters per hint, plain words instead of fancy ones. Ticket 33's
// bans hold on top: no "Sprout never ..." phrasing, no plant/grow wordplay
// outside the logo, and a single "Loading ..." that this dictionary never
// redefines. Identical text reuses one key so translation later is a file
// swap.
//
// File-based by design: the English source of truth lives in en.json beside
// this loader and never in the database, so machine-local boundaries stay
// intact and a future locale only adds a file (ticket 220 owns zh-CN).

import en from "./en.json";
import zhCN from "./zh-CN.json";
import { localeState } from "./locale.svelte";

type EnglishCopy = Record<string, string>;

// WHY: a single flat table keyed `section.key` keeps lookups and coverage
// checks trivial — nesting would add traversal code for no second consumer
// (conventions: no shared/ module for a single EN/zh-CN pair).
const ENGLISH: EnglishCopy = en as EnglishCopy;

// WHY the second table lives beside the first: a locale is a whole-file swap,
// so the active table is picked per key below and English stays the
// key-by-key fallback — a partial translation reads through instead of
// blanking (ADR-0028; files only, never the database).
const SIMPLIFIED_CHINESE: EnglishCopy = zhCN as EnglishCopy;

export type CopyKey = keyof typeof en & string;

/** The locales the dictionary ships. English is the source of truth and the
 *  fallback; Simplified Chinese is the whole-file swap. */
export type Locale = "en" | "zh-CN";

const TABLES: Record<Locale, EnglishCopy> = {
  en: ENGLISH,
  "zh-CN": SIMPLIFIED_CHINESE,
};

/** The active-locale string for a key — Simplified Chinese when selected,
 *  English otherwise — falling back key-by-key to English, or to the key
 *  itself when unknown, so the UI never blanks. */
export function t(key: string): string {
  // WHY read the reactive locale inside the lookup: every existing call site
  // renders through this one path, so switching languages re-renders each
  // surface live with no second lookup and no per-call-site wiring.
  const table = TABLES[localeState.current] ?? ENGLISH;
  return table[key] ?? ENGLISH[key] ?? key;
}

// WHY: locales arrive as whole-file swaps, so a partial translation must read
// through to English per key rather than fail or blank — the fallback lives
// here so every consumer gets it without reimplementing the lookup.
export function resolveCopy(
  key: string,
  overrides?: Record<string, string>,
): string {
  if (overrides && overrides[key] !== undefined) return overrides[key] as string;
  return t(key);
}

/** Fill a `{count}` slot in a keyed string (e.g. saved-site counts). */
export function tCount(key: string, count: number): string {
  return t(key).replace("{count}", String(count));
}

export const englishCopy: EnglishCopy = ENGLISH;
