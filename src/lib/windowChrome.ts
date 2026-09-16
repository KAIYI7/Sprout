/** Shared window-chrome vocabulary for the main window's unified header.
 *
 *  `SECTION_LABEL_BY_ROUTE` is the single map from route to the context label
 *  shown in the header breadcrumb — the layout's Discord presence report reads
 *  the same map, so the header and the presence state can never disagree
 *  (research 0005 rule 5: same-kind content shares one treatment).
 */

export const SECTION_LABEL_BY_ROUTE: Record<string, string> = {
  "/": "Launching apps",
  "/products": "Browsing products",
  "/presets": "Composing presets",
  "/plan": "Reviewing a plan",
  "/history": "Reviewing history",
  "/logs": "Reading logs",
  "/settings": "Tuning settings",
  "/clips": "Managing clips",
  "/quick-actions": "Editing quick actions",
};

/** The context label for a route id — unknown routes read as the default
 *  composing state, never blank. */
export function sectionLabelForRoute(routeId: string | null | undefined): string {
  return SECTION_LABEL_BY_ROUTE[routeId ?? "/"] ?? "Composing presets";
}

export type NativeFrameMode = "on" | "off";

/** A stored frame switch the menu does not offer reads back as custom — the
 *  same fallback the backend applies, so a broken value never freezes the UI
 *  on a chrome it cannot render. */
export function parseNativeFrame(value: unknown): NativeFrameMode {
  return value === "on" ? "on" : "off";
}
