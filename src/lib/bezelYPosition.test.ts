import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const API_SOURCE = readFileSync(new URL("./api.ts", import.meta.url), "utf8");
const PAGE_SOURCE = readFileSync(
  new URL("../routes/quick-launch-window/+page.svelte", import.meta.url),
  "utf8",
);

describe("bezel Y position contract (spec 214)", () => {
  it("exposes per-display bezel-Y wrappers over the identity-keyed commands", () => {
    expect(API_SOURCE).toContain("get_display_bezel_y_ratio");
    expect(API_SOURCE).toContain("set_display_bezel_y_ratio");
    expect(API_SOURCE).toContain("getDisplayBezelYRatio");
    expect(API_SOURCE).toContain("setDisplayBezelYRatio");
    // Per-display key shape: display id in, validated ratio out.
    expect(API_SOURCE).toContain("{ display }");
    expect(API_SOURCE).toContain("{ display, ratio }");
  });

  it("parks the tab with a draggable, keyboard-nudgable grip on the bezel tab", () => {
    // Slider role with a live value so screen readers hear the position.
    expect(PAGE_SOURCE).toContain('role="slider"');
    expect(PAGE_SOURCE).toContain("aria-valuenow");
    // Pointer drag moves live, release persists; arrows nudge as the
    // accessible alternative so drag is never the only path.
    expect(PAGE_SOURCE).toContain("onBezelGripPointerDown");
    expect(PAGE_SOURCE).toContain("onBezelGripKeyDown");
    expect(PAGE_SOURCE).toContain("setDisplayBezelYRatio");
    expect(PAGE_SOURCE).toContain("placeBezelTab");
  });

  it("clamps the ratio and centers on disconnect instead of losing the tab", () => {
    expect(PAGE_SOURCE).toContain("clampBezelYRatio");
    expect(PAGE_SOURCE).toContain("Math.min(1, Math.max(0, f))");
    // The remembered ratio re-resolves after dock and arrangement changes;
    // a gone display reads centered without persisting.
    expect(PAGE_SOURCE).toContain("refreshBezelY");
    expect(PAGE_SOURCE).toContain('listen("displays-changed"');
  });

  it("derives every pixel from the backend, never JS geometry", () => {
    expect(PAGE_SOURCE).toContain("getBezelTabY");
    expect(PAGE_SOURCE).toContain("getBezelTabHeight");
    expect(PAGE_SOURCE).not.toContain("BEZEL_COLLAPSED_WIDTH_PX");
    expect(PAGE_SOURCE).not.toContain("BEZEL_PEEK_WIDTH_PX");
    expect(API_SOURCE).not.toContain("BEZEL_COLLAPSED_WIDTH_PX");
  });
});
