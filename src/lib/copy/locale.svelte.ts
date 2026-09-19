/// The app-wide UI locale store ( Simplified-Chinese file swap).
///
/// One source of truth for the Settings screen's Language knob and the locale
/// every `t()` lookup reads. The locale ("en" or "zh-CN") is persisted by the
/// backend beside the theme — machine-local, never in backups — with a
/// localStorage cache that applies it before the first paint so a restored
/// language never flashes English. Reads reconcile from the backend; writes
/// apply instantly and persist on their own, without saving the rest of the
/// Settings form.
///
/// WHY the store lives beside the dictionary instead of in a shared module:
/// the locale is the dictionary's active-table pointer with exactly one
/// consumer pair (EN/zh-CN), so a shared/ module would be a thin
/// pass-through (conventions). WHY dynamic backend imports: this module loads
/// beside the copy loader in unit tests, which run without Tauri — persistence
/// is only ever needed on a real user gesture or window start.

import type { Locale } from "./index";

const STORAGE_KEY = "sprout.locale";

export const localeState = $state<{
  /// The locale the UI currently renders.
  current: Locale;
}>({
  current: "en",
});

/// A stored locale the menu does not offer reads back as English — the same
/// fallback the backend applies, so a broken value never blanks the UI.
export function parseLocale(value: unknown): Locale {
  return value === "zh-CN" ? "zh-CN" : "en";
}

// Module scope runs before the layout renders, so the cached locale is in
// place before the first paint. Guarded for non-browser imports: unit tests
// load this module in Node, where storage simply does not exist.
let cached: Locale = "en";
try {
  cached =
    typeof localStorage !== "undefined"
      ? parseLocale(localStorage.getItem(STORAGE_KEY))
      : "en";
} catch {
  cached = "en";
}
localeState.current = cached;

function cacheLocale(locale: Locale) {
  try {
    localStorage.setItem(STORAGE_KEY, locale);
  } catch {
    // Storage unavailable — the backend still holds the persisted locale.
  }
}

/// Applies a locale read from the backend without writing it back — the value
/// already is the persisted one (used on window start and on change events).
export function restoreLocale(locale: Locale): void {
  localeState.current = parseLocale(locale);
  cacheLocale(localeState.current);
}

/// The Settings screen's picker — applies instantly and persists on its own,
/// without saving the rest of the form. Rejects if the backend refused.
export async function selectLocale(locale: Locale): Promise<void> {
  const next = parseLocale(locale);
  localeState.current = next;
  cacheLocale(next);
  const { updateLanguage } = await import("$lib/api");
  await updateLanguage(next);
}

/// Test and fallback seam: sets the rendered locale without touching
/// persistence — production switches go through select/restore above.
export function setLocaleForTest(locale: Locale): void {
  localeState.current = parseLocale(locale);
}

/// Re-reads the persisted locale (a fresh install or a cleared cache still
/// lands on the stored language).
async function refreshLocaleFromBackend(): Promise<void> {
  try {
    const { getLanguage } = await import("$lib/api");
    restoreLocale(parseLocale(await getLanguage()));
  } catch {
    // Offline or locked DB: the cache still holds the last language.
  }
}

let started = false;

/// One-time wiring: backend reconciliation plus a re-read whenever any window
/// persists a language change. The backend emits `quick-launch-changed`
/// beside every language save (like the theme), so the Quick Launch webview
/// follows the new language without its own wiring — the layout calls this
/// once per window.
export function startLocale(): void {
  if (started) return;
  started = true;
  void refreshLocaleFromBackend();
  void import("@tauri-apps/api/event")
    .then(({ listen }) =>
      listen("quick-launch-changed", () => {
        void refreshLocaleFromBackend();
      }),
    )
    .catch(() => {
      // Not a Tauri window (unit tests): the cached locale stands.
    });
}
