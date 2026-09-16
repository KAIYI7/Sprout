import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import {
  parseNativeFrame,
  SECTION_LABEL_BY_ROUTE,
  sectionLabelForRoute,
} from "./windowChrome";

// Contract tests for the main window's unified header (research 0025): one
// continuous strip carrying the mark + section breadcrumb and the window
// buttons — never a primary button or search (research 0005 rule 2). The bar
// itself is the Tauri drag region; the button cluster opts out so presses
// land on the buttons.
const HEADER_SOURCE = readFileSync(
  new URL("./components/UnifiedHeader.svelte", import.meta.url),
  "utf8",
);
const LAYOUT_SOURCE = readFileSync(
  new URL("../routes/+layout.svelte", import.meta.url),
  "utf8",
);
const SETTINGS_SOURCE = readFileSync(
  new URL("../routes/settings/+page.svelte", import.meta.url),
  "utf8",
);
const RAIL_SOURCE = readFileSync(
  new URL("./components/NavRail.svelte", import.meta.url),
  "utf8",
);

describe("sectionLabelForRoute", () => {
  it("labels the home route as launching", () => {
    expect(sectionLabelForRoute("/")).toBe("Launching apps");
  });

  it("labels every mapped route from the one shared map", () => {
    for (const [route, label] of Object.entries(SECTION_LABEL_BY_ROUTE)) {
      expect(sectionLabelForRoute(route)).toBe(label);
    }
  });

  it("reads unknown and missing routes as the default composing state, never blank", () => {
    expect(sectionLabelForRoute("/nope")).toBe("Composing presets");
    expect(sectionLabelForRoute(null)).toBe("Launching apps");
    expect(sectionLabelForRoute(undefined)).toBe("Launching apps");
  });
});

describe("parseNativeFrame", () => {
  it("keeps only the offered native position", () => {
    expect(parseNativeFrame("on")).toBe("on");
  });

  it("reads everything else as the unified header", () => {
    expect(parseNativeFrame("off")).toBe("off");
    expect(parseNativeFrame("sometimes")).toBe("off");
    expect(parseNativeFrame("")).toBe("off");
    expect(parseNativeFrame(undefined)).toBe("off");
  });
});

describe("UnifiedHeader drag + button contract", () => {
  it("makes the bar the drag region", () => {
    expect(HEADER_SOURCE).toMatch(/data-tauri-drag-region="deep"/);
  });

  it("opts the button cluster out of dragging", () => {
    expect(HEADER_SOURCE).toMatch(
      /win-header__controls" data-tauri-drag-region="false"/,
    );
  });

  it("labels all three window buttons and swaps the maximize glyph", () => {
    expect(HEADER_SOURCE).toContain('label="Minimize"');
    expect(HEADER_SOURCE).toContain('label="Close"');
    expect(HEADER_SOURCE).toMatch(
      /label=\{maximized \? "Restore" : "Maximize"\}/,
    );
    expect(HEADER_SOURCE).toMatch(/icon=\{maximized \? "restore" : "square"\}/);
  });

  it("double-clicks the bar to toggle maximize, never from a button press", () => {
    expect(HEADER_SOURCE).toMatch(/ondblclick=\{handleDblClick\}/);
    expect(HEADER_SOURCE).toMatch(/closest\("button"\)/);
  });

  it("holds no primary button or search", () => {
    expect(HEADER_SOURCE).not.toMatch(/<Button /);
    expect(HEADER_SOURCE).not.toMatch(/SearchInput/);
  });

  it("shares one hairline-free surface with the rail, distinct from the page", () => {
    expect(HEADER_SOURCE).toMatch(/background: var\(--bg-surface\)/);
    expect(HEADER_SOURCE).not.toMatch(/border-bottom/);
    expect(RAIL_SOURCE).toMatch(/background: var\(--bg-surface\)/);
    expect(RAIL_SOURCE).not.toMatch(/border-right/);
  });

  it("starts the mark where the rail pills start", () => {
    expect(HEADER_SOURCE).toMatch(/padding: 0 var\(--space-2\) 0 var\(--space-3\)/);
  });

  it("keeps the only Sprout mark — the rail brand is gone", () => {
    expect(RAIL_SOURCE).not.toContain("rail__brand");
    expect(RAIL_SOURCE).not.toContain("SproutMark");
    expect(HEADER_SOURCE).toContain("SproutMark");
  });
});

describe("layout composition", () => {
  it("renders the header above the shell, hidden under the native frame", () => {
    expect(LAYOUT_SOURCE).toMatch(/<UnifiedHeader/);
    expect(LAYOUT_SOURCE).toMatch(/nativeFrame\.mode === "off"/);
    expect(LAYOUT_SOURCE).toMatch(
      /section=\{sectionLabelForRoute\(page\.route\.id\)\}/,
    );
  });

  it("panels the content with rounded top corners, square when maximized", () => {
    expect(LAYOUT_SOURCE).toMatch(/class:app--framed=\{nativeFrame\.mode === "off"\}/);
    expect(LAYOUT_SOURCE).toMatch(/\.app--framed \.stage/);
    expect(LAYOUT_SOURCE).toMatch(/border-top-left-radius: var\(--radius-lg\)/);
    expect(LAYOUT_SOURCE).toMatch(/\.app--maximized \.stage/);
  });

  it("reports presence from the same shared map the header breadcrumbs", () => {
    expect(LAYOUT_SOURCE).toMatch(/SECTION_LABEL_BY_ROUTE\[page\.route\.id/);
    expect(LAYOUT_SOURCE).not.toContain("PRESENCE_STATE_BY_ROUTE");
  });
});

describe("settings fallback knob", () => {
  it("carries the frame switch through load, pick, and bulk save", () => {
    expect(SETTINGS_SOURCE).toMatch(/restoreNativeFrame\(parseNativeFrame\(loaded\.native_frame\)\)/);
    expect(SETTINGS_SOURCE).toMatch(/knobVisible\("native-frame"\)/);
    expect(SETTINGS_SOURCE).toMatch(/native_frame: nativeFrame\.mode/);
  });
});
