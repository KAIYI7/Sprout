import { readFileSync } from "node:fs";
import { describe, expect, it, vi } from "vitest";
import type { CompanionSite } from "./types";
import {
  companionDisplayName,
  companionDockPickerSites,
  companionPickerLabel,
  COMPANION_DOCK_PICKER_LIMIT,
  createCompanionSiteSwitchQueue,
} from "./companion";

const ROUTE_SOURCE = readFileSync(
  new URL("../routes/quick-launch-window/+page.svelte", import.meta.url),
  "utf8",
);
const SETTINGS_SOURCE = readFileSync(
  new URL("../routes/settings/+page.svelte", import.meta.url),
  "utf8",
);
const LAYOUT_SOURCE = readFileSync(
  new URL("../routes/+layout.svelte", import.meta.url),
  "utf8",
);
const API_SOURCE = readFileSync(
  new URL("./api.ts", import.meta.url),
  "utf8",
);

function site(url: string, name = "", ua: "mobile" | "desktop" = "mobile"): CompanionSite {
  return { url, name, ua };
}

function deferred() {
  let resolve!: () => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<void>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

async function settle() {
  await Promise.resolve();
  await Promise.resolve();
}

describe("Companion saved-site picker", () => {
  it("shows both identity and address for named sites", () => {
    expect(companionPickerLabel(site("https://example.com/work", "Work"))).toBe(
      "Work — https://example.com/work",
    );
    expect(companionPickerLabel(site("https://example.com"))).toBe(
      "https://example.com",
    );
  });

  it("shows the user-configured name only in picker rows", () => {
    // Names are unique at authoring (duplicates refused); blank names fall
    // back to the address via companionDisplayName. The trigger tooltip keeps
    // the full name + address for long/similar entries.
    const menuAt = ROUTE_SOURCE.indexOf("const companionSiteMenu");
    expect(menuAt).toBeGreaterThan(-1);
    expect(ROUTE_SOURCE.slice(menuAt, menuAt + 800)).toContain(
      "label: companionDisplayName(site)",
    );
  });

  it("keeps the single-site label plain and discloses multiple sites semantically", () => {
    expect(ROUTE_SOURCE).toContain("{#if companionHasSitePicker}");
    expect(ROUTE_SOURCE).toContain('aria-haspopup="menu"');
    expect(ROUTE_SOURCE).toContain("aria-expanded={companionSiteMenuOpen}");
    expect(ROUTE_SOURCE).toContain("<ContextMenu ctx={companionSiteMenu}");
    expect(ROUTE_SOURCE).toMatch(
      /\{:else\}\s*<span class="qlw__companion-url"[^>]*>/,
    );
  });

  it("keeps the external action separate and leaves the site visible under the picker", () => {
    const selector = ROUTE_SOURCE.indexOf('aria-haspopup="menu"');
    const external = ROUTE_SOURCE.indexOf('label={companionOpeningExternal');
    expect(selector).toBeGreaterThan(-1);
    expect(external).toBeGreaterThan(selector);
    // The details dialog still yields the native child, but the site menu
    // opens upward over web content so the page stays visible underneath.
    expect(ROUTE_SOURCE).toContain("const overlayOpen = detailsAction !== null;");
    expect(ROUTE_SOURCE).toContain('placement: "above"');
    expect(ROUTE_SOURCE).not.toContain(
      "detailsAction !== null || companionSiteMenuOpen",
    );
  });

  it("contains long labels and announces pending switches without claiming success", () => {
    // Rows show the name only; the trigger tooltip keeps the full
    // name + address for long/similar entries.
    expect(ROUTE_SOURCE).toContain("companionPickerLabel(activeCompanionSite)");
    expect(ROUTE_SOURCE).toContain("overflow-wrap: anywhere");
    expect(ROUTE_SOURCE).toContain('aria-live="polite"');
    expect(ROUTE_SOURCE).toContain('aria-busy={companionSwitchingTo !== null}');
    expect(ROUTE_SOURCE).toContain('{companionSwitchingTo ? "Switching…"');
  });

  it("serializes rapid requests and applies only the newest successful site", async () => {
    const first = deferred();
    const second = deferred();
    const persist = vi
      .fn<(url: string) => Promise<void>>()
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);
    const pending: Array<CompanionSite | null> = [];
    const applied: CompanionSite[] = [];
    const failed: Array<{ site: CompanionSite; error: unknown }> = [];
    const request = createCompanionSiteSwitchQueue({
      persist,
      onPending: (value) => pending.push(value),
      onApplied: (value) => applied.push(value),
      onFailure: (value, error) => failed.push({ site: value, error }),
    });
    const work = site("https://work.example", "Work", "desktop");
    const music = site("https://music.example", "Music");

    request(work);
    request(work);
    request(music);
    expect(persist).toHaveBeenCalledTimes(1);
    first.resolve();
    await settle();
    expect(persist).toHaveBeenNthCalledWith(2, music.url);
    expect(applied).toEqual([]);

    second.resolve();
    await settle();
    expect(applied).toEqual([music]);
    expect(failed).toEqual([]);
    expect(pending).toEqual([work, music, null]);
  });

  it("ignores an obsolete failure but reports a failure for the latest request", async () => {
    const first = deferred();
    const second = deferred();
    const persist = vi
      .fn<(url: string) => Promise<void>>()
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);
    const applied: CompanionSite[] = [];
    const failed: Array<{ site: CompanionSite; error: unknown }> = [];
    const request = createCompanionSiteSwitchQueue({
      persist,
      onPending: () => {},
      onApplied: (value) => applied.push(value),
      onFailure: (value, error) => failed.push({ site: value, error }),
    });
    const firstSite = site("https://first.example");
    const lastSite = site("https://last.example");

    request(firstSite);
    request(lastSite);
    first.reject(new Error("obsolete"));
    await settle();
    expect(failed).toEqual([]);

    const finalError = new Error("save refused");
    second.reject(finalError);
    await settle();
    expect(applied).toEqual([]);
    expect(failed).toEqual([{ site: lastSite, error: finalError }]);
  });
});

describe("Companion dock picker cap (first five + manage row)", () => {
  function numbered(count: number): CompanionSite[] {
    return Array.from({ length: count }, (_, i) => ({
      url: `https://site${i + 1}.example`,
      name: `Site ${i + 1}`,
      ua: "mobile" as const,
    }));
  }

  it("caps the dock set at five sites", () => {
    expect(COMPANION_DOCK_PICKER_LIMIT).toBe(5);
  });

  it("renders an empty menu model for no sites", () => {
    expect(companionDockPickerSites([])).toEqual([]);
  });

  it("keeps a single site whole (the plain-label case stays untouched)", () => {
    const one = numbered(1);
    expect(companionDockPickerSites(one)).toEqual(one);
  });

  it("passes through exactly five sites unchanged", () => {
    const five = numbered(5);
    expect(companionDockPickerSites(five)).toEqual(five);
  });

  it("shows the first five in user order when six or more are saved", () => {
    expect(companionDockPickerSites(numbered(6)).map((s) => s.url)).toEqual([
      "https://site1.example",
      "https://site2.example",
      "https://site3.example",
      "https://site4.example",
      "https://site5.example",
    ]);
    expect(companionDockPickerSites(numbered(8))).toHaveLength(5);
  });

  it("keeps unnamed sites renderable (blank names still fall back to the address)", () => {
    const mixed: CompanionSite[] = [
      { url: "https://noname.example", name: "", ua: "mobile" },
      ...numbered(6).slice(1),
    ];
    const shown = companionDockPickerSites(mixed);
    expect(shown).toHaveLength(5);
    expect(companionDisplayName(shown[0]!)).toBe("https://noname.example");
  });

  it("never mutates the saved list", () => {
    const saved = numbered(7);
    const snapshot = [...saved];
    companionDockPickerSites(saved);
    expect(saved).toEqual(snapshot);
  });

  it("builds the dock menu from the capped set with the active site marked", () => {
    const menuAt = ROUTE_SOURCE.indexOf("const companionSiteMenu");
    expect(menuAt).toBeGreaterThan(-1);
    const menuSrc = ROUTE_SOURCE.slice(menuAt, menuAt + 1400);
    expect(menuSrc).toContain("companionDockPickerSites(companionUrlList)");
    expect(menuSrc).toContain("checked:");
    expect(menuSrc).not.toContain("companionUrlList.map((site)");
  });

  it("ends the dock menu with a management row into the full manager", () => {
    expect(ROUTE_SOURCE).toContain("Manage in Sprout…");
    expect(ROUTE_SOURCE).toContain("manageCompanionSites");
    expect(ROUTE_SOURCE).toContain("openCompanionManager");
    const menuAt = ROUTE_SOURCE.indexOf("const companionSiteMenu");
    const menuSrc = ROUTE_SOURCE.slice(menuAt, menuAt + 1400);
    expect(menuSrc).toContain("separator: true");
    // The manage row is a plain navigation row — no search box or lazy
    // loading joins the dock surface.
    expect(ROUTE_SOURCE).not.toMatch(/companionSiteMenu[\s\S]{0,400}Search/);
  });

  it("keeps the single-site plain label and the absent-pane behavior", () => {
    expect(ROUTE_SOURCE).toContain("const companionHasSitePicker = $derived(companionUrlList.length > 1);");
    expect(ROUTE_SOURCE).toMatch(
      /\{:else\}\s*<span class="qlw__companion-url"[^>]*>/,
    );
  });

  it("leaves Settings and the manager uncapped (the cap deletes nothing)", () => {
    // The Settings Active-site control still maps the whole saved list.
    expect(SETTINGS_SOURCE).toContain("...companionUrlList.map((site) => ({");
    expect(SETTINGS_SOURCE).not.toContain("companionDockPickerSites");
    expect(SETTINGS_SOURCE).not.toContain("COMPANION_DOCK_PICKER_LIMIT");
  });

  it("routes the management row to the full manager", () => {
    expect(API_SOURCE).toContain('invoke<void>("open_companion_manager")');
    // An already-open main window navigates on the event; a recreated one
    // consumes the recorded route on load (the pending-import shape).
    expect(LAYOUT_SOURCE).toContain('listen("open-companion-manager"');
    expect(LAYOUT_SOURCE).toContain('goto("/companion")');
    expect(LAYOUT_SOURCE).toContain("takePendingRoute()");
  });
});
