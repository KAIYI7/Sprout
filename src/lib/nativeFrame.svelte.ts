/// The main window's frame store.
///
/// One source of truth for the Settings screen's Window frame knob and the
/// layout's choice between the unified header (frameless window) and no
/// header (native OS titlebar). The switch ("off" draws the unified header,
/// "on" restores the native frame) is persisted by the backend; a
/// localStorage cache applies it before first paint so a native-frame
/// fallback never flashes the custom header over the OS chrome.

import { getSettings, updateNativeFrame } from "$lib/api";
import { parseNativeFrame, type NativeFrameMode } from "$lib/windowChrome";

const STORAGE_KEY = "sprout.native_frame";

export const nativeFrame = $state<{
  /// The switch position the user picked (or the default "off").
  mode: NativeFrameMode;
}>({
  mode: "off",
});

let started = false;

function apply(mode: NativeFrameMode) {
  nativeFrame.mode = mode;
  try {
    localStorage.setItem(STORAGE_KEY, mode);
  } catch {
    // Storage unavailable — the backend still holds the persisted position.
  }
}

// Module scope runs before the layout renders, so a cached native-frame
// fallback hides the custom header before the first paint.
let cached: NativeFrameMode = "off";
try {
  cached = parseNativeFrame(localStorage.getItem(STORAGE_KEY));
} catch {
  // Storage unavailable — fall back to the unified header.
}
apply(cached);

/// One-time wiring: reconciles with the backend so a fresh install or a
/// cleared cache still lands on the persisted switch position. The layout
/// calls this once.
export function startNativeFrame() {
  if (started) return;
  started = true;
  getSettings()
    .then((settings) => {
      const mode = parseNativeFrame(settings.native_frame);
      if (mode !== nativeFrame.mode) apply(mode);
    })
    .catch(() => {
      // Offline or locked DB: the cache still holds the last position.
    });
}

/// Applies a position read from the backend without writing it back — the
/// value already is the persisted one (used when the Settings page loads
/// settings fresh).
export function restoreNativeFrame(mode: NativeFrameMode) {
  apply(mode);
}

/// The Settings screen's switch — applies instantly and persists on its own,
/// without saving the rest of the form. Rejects if the backend refused.
export async function selectNativeFrame(mode: NativeFrameMode): Promise<void> {
  apply(mode);
  await updateNativeFrame(mode);
}
