/** Shared window-chrome vocabulary for the main window's unified header.
 *
 *  `SECTION_KEY_BY_ROUTE` is the single map from route to dictionary key —
 *  the header breadcrumb resolves it through the active locale while the
 *  layout's Discord presence report reads the same keys from the English map,
 *  so header and presence can never disagree on the section (research 0005
 *  rule 5: same-kind content shares one treatment).
 *  Values come from the copy dictionary (spec 214) so wording stays in one
 *  file; the nine routes themselves are unchanged (ADR-0033 wording-only).
 */

import { englishCopy, t } from "./copy";

// Route → dictionary key. Keys are the contract; values resolve per call so
// the header breadcrumb follows the active locale live.
const SECTION_KEY_BY_ROUTE: Record<string, string> = {
  "/": "presence.state.launching",
  "/products": "presence.state.products",
  "/presets": "presence.state.presets",
  "/plan": "presence.state.plan",
  "/history": "presence.state.history",
  "/logs": "presence.state.logs",
  "/settings": "presence.state.settings",
  "/clips": "presence.state.clips",
  "/quick-actions": "presence.state.quick-actions",
};

const FALLBACK_SECTION_KEY = "presence.state.presets";

export const SECTION_LABEL_BY_ROUTE: Record<string, string> = {
  "/": englishCopy["presence.state.launching"],
  "/products": englishCopy["presence.state.products"],
  "/presets": englishCopy["presence.state.presets"],
  "/plan": englishCopy["presence.state.plan"],
  "/history": englishCopy["presence.state.history"],
  "/logs": englishCopy["presence.state.logs"],
  "/settings": englishCopy["presence.state.settings"],
  "/clips": englishCopy["presence.state.clips"],
  "/quick-actions": englishCopy["presence.state.quick-actions"],
};

/** The context label for a route id — unknown routes read as the default
 *  composing state, never blank. Resolves through the active locale, so the
 *  header translates live; the Discord pipe keeps the English map above
 *  (ADR-0033 wording lock). */
export function sectionLabelForRoute(routeId: string | null | undefined): string {
  return t(SECTION_KEY_BY_ROUTE[routeId ?? "/"] ?? FALLBACK_SECTION_KEY);
}

export type NativeFrameMode = "on" | "off";

/** A stored frame switch the menu does not offer reads back as custom — the
 *  same fallback the backend applies, so a broken value never freezes the UI
 *  on a chrome it cannot render. */
export function parseNativeFrame(value: unknown): NativeFrameMode {
  return value === "on" ? "on" : "off";
}
