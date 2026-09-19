import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const SETTINGS_SOURCE = readFileSync(
  new URL("../routes/settings/+page.svelte", import.meta.url),
  "utf8",
);
const TYPES_SOURCE = readFileSync(
  new URL("./types.ts", import.meta.url),
  "utf8",
);

describe("bezel settings UI (spec 214, ticket 218)", () => {
  it("offers bezel in the global dock mode options", () => {
    expect(SETTINGS_SOURCE).toContain('t("settings.opt.mode.bezel")');
    expect(SETTINGS_SOURCE).toContain('{ value: "bezel"');
  });

  it("offers bezel in every per-display mode list", () => {
    expect(SETTINGS_SOURCE).toContain(
      '{ value: "bezel", label: t("settings.opt.mode.bezel") },',
    );
  });

  it("binds the tab vertical-position control to bezel_y_ratio both ways", () => {
    // Reads the remembered ratio (the drag's mirror) and persists edits
    // through the same per-monitor seam as edge/mode/width.
    expect(SETTINGS_SOURCE).toContain("getDisplayBezelYRatio");
    expect(SETTINGS_SOURCE).toContain("setDisplayBezelYRatio");
    expect(SETTINGS_SOURCE).toContain("clampBezelYRatio");
    expect(SETTINGS_SOURCE).toContain("displayBezelY");
    // The single-display knob and the per-row sliders share the one label.
    expect(SETTINGS_SOURCE).toContain('t("quickwindow.bezelPosition")');
    expect(SETTINGS_SOURCE).toContain('t("settings.bezel-y.hint")');
  });

  it("disables the global width slider exactly when per-display widths rule", () => {
    expect(SETTINGS_SOURCE).toContain("disabled={displays.length > 1}");
    expect(SETTINGS_SOURCE).toContain('href="#per-monitor-title"');
    expect(SETTINGS_SOURCE).toContain('t("settings.dock-width.multiHint")');
    expect(SETTINGS_SOURCE).toContain('id="per-monitor-title"');
  });

  it("joins the dirty-bar and save flow like the neighboring knobs", () => {
    expect(SETTINGS_SOURCE).toContain("baseline.bezelYRatio");
    expect(SETTINGS_SOURCE).toContain("baselineDisplayBezelY");
  });

  it("caps bezel width like the auto-hide overlay, not the fixed strip", () => {
    expect(SETTINGS_SOURCE).toContain(
      'return mode === "auto-hide" || mode === "bezel" ? DOCK_WIDTH_MAX_PCT_AUTOHIDE : DOCK_WIDTH_MAX_PCT_FIXED;',
    );
  });

  it("carries bezel in the dock state's mode union", () => {
    expect(TYPES_SOURCE).toContain('mode: "auto-hide" | "fixed" | "bezel"');
  });
});
