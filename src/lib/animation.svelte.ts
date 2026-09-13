/// The app-wide motion store.
///
/// One source of truth for the Settings screen's Animation knob and the
/// `data-animation` attribute the tokens' motion hook reads. The switch ("on"
/// plays every transition, "off" renders each end state at once) is persisted
/// by the backend; a localStorage cache applies it before first paint so an
/// off switch survives restarts without a flash of motion. The OS
/// `prefers-reduced-motion` setting keeps working on its own through the
/// tokens' media query — the switch only ever adds stillness, never motion.

import { getSettings, updateAnimation } from "$lib/api";

export type AnimationMode = "on" | "off";

const STORAGE_KEY = "sprout.animation";

export const animation = $state<{
  /// The switch position the user picked (or the default "on").
  mode: AnimationMode;
}>({
  mode: "on",
});

let started = false;

function parseMode(value: string | null): AnimationMode {
  return value === "off" ? "off" : "on";
}

/// Applies a switch position to the document right now and caches it for the
/// next launch. Only the off position marks the document — the tokens' hook
/// selects on it, so the on position needs no attribute at all.
function apply(mode: AnimationMode) {
  animation.mode = mode;
  // WHY the guard: server/test renders import this store through Dialog but
  // have no document — state still updates so logic stays testable, while
  // the attribute write waits for a real document (ADR-0028).
  if (typeof document === "undefined") return;
  if (mode === "off") {
    document.documentElement.dataset.animation = "off";
  } else {
    delete document.documentElement.dataset.animation;
  }
  try {
    localStorage.setItem(STORAGE_KEY, mode);
  } catch {
    // Storage unavailable — the backend still holds the persisted position.
  }
}

// Module scope runs before the layout renders, so a cached off switch is on
// the document before the first paint.
let cached: AnimationMode = "on";
try {
  cached = parseMode(localStorage.getItem(STORAGE_KEY));
} catch {
  // Storage unavailable — fall back to motion on.
}
apply(cached);

/// One-time wiring: reconciles with the backend so a fresh install or a
/// cleared cache still lands on the persisted switch position. The layout
/// calls this once.
export function startAnimation() {
  if (started) return;
  started = true;
  getSettings()
    .then((settings) => {
      const mode = parseMode(settings.animation);
      if (mode !== animation.mode) apply(mode);
    })
    .catch(() => {
      // Offline or locked DB: the cache still holds the last position.
    });
}

/// Applies a position read from the backend without writing it back — the
/// value already is the persisted one (used when a page loads settings
/// fresh, and by the dock window on every settings change).
export function restoreAnimation(mode: AnimationMode) {
  apply(mode);
}

/// The Settings screen's switch — applies instantly and persists on its own,
/// without saving the rest of the form. Rejects if the backend refused.
export async function selectAnimation(mode: AnimationMode): Promise<void> {
  apply(mode);
  await updateAnimation(mode);
}
