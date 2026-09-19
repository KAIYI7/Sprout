import { describe, expect, it, vi } from "vitest";
import { readFileSync } from "node:fs";
import { render } from "svelte/server";
import QuickActionFormDialog from "./components/QuickActionFormDialog.svelte";

const DIALOG_SOURCE = readFileSync(
  new URL("./components/QuickActionFormDialog.svelte", import.meta.url),
  "utf8",
);

/// The AI view lives between its panel marker and the manual panel marker.
function aiSection(): string {
  const start = DIALOG_SOURCE.indexOf('id="panel-ai"');
  const end = DIALOG_SOURCE.indexOf('id={aiReady ? "panel-manual"');
  expect(start).toBeGreaterThan(-1);
  expect(end).toBeGreaterThan(start);
  return DIALOG_SOURCE.slice(start, end);
}

describe("QuickActionFormDialog two-view dialog", () => {
  it("renders Add+ready in the AI view with tabs and the single hero", () => {
    const { body } = render(QuickActionFormDialog, {
      props: {
        open: true,
        action: null,
        aiReady: true,
        onsave: vi.fn(),
        oncancel: vi.fn(),
      },
    });
    expect(body).toContain("Add a quick action");
    expect(body).toContain("AI draft");
    expect(body).toContain("Manual");
    expect(body).toContain("qa-ai-request");
    expect(body).toContain("Generate draft");
  });

  it("renders zero AI nodes when unready — the manual form stands alone", () => {
    const { body } = render(QuickActionFormDialog, {
      props: {
        open: true,
        action: null,
        aiReady: false,
        onsave: vi.fn(),
        oncancel: vi.fn(),
      },
    });
    expect(body).toContain("Add a quick action");
    expect(body).not.toContain("AI draft");
    expect(body).not.toContain("qa-ai-request");
    expect(body).not.toContain("Generate draft");
    expect(body).toContain("qa-name");
  });

  it("points the unready managed route at Settings with one plain-text line (ticket 192)", () => {
    const { body } = render(QuickActionFormDialog, {
      props: {
        open: true,
        action: null,
        aiReady: false,
        aiSetupKind: "managed",
        onsave: vi.fn(),
        oncancel: vi.fn(),
        onsetupai: vi.fn(),
      },
    });
    expect(body).toContain("No managed model is on right now");
    expect(body).toContain("Enable it");
    // Zero chrome holds: no tabs, no hero, no duplicated config.
    expect(body).not.toContain("AI draft");
    expect(body).not.toContain("qa-ai-request");
    expect(body).not.toContain("Generate draft");
    // At most one pointer line.
    expect(body.match(/ai-setup__link/g)?.length).toBe(1);
  });

  it("points any other unready route at generic setup (ticket 192)", () => {
    const { body } = render(QuickActionFormDialog, {
      props: {
        open: true,
        action: null,
        aiReady: false,
        aiSetupKind: "generic",
        onsave: vi.fn(),
        oncancel: vi.fn(),
        onsetupai: vi.fn(),
      },
    });
    expect(body).toContain("Set up AI assistance in Settings");
    expect(body).not.toContain("No managed model is on right now");
    expect(body).not.toContain("AI draft");
    expect(body).not.toContain("qa-ai-request");
  });

  it("keeps the ready state free of the enable line (ticket 192)", () => {
    const { body } = render(QuickActionFormDialog, {
      props: {
        open: true,
        action: null,
        aiReady: true,
        onsave: vi.fn(),
        oncancel: vi.fn(),
      },
    });
    expect(body).toContain("AI draft");
    expect(body).toContain("qa-ai-request");
    expect(body).toContain("Generate draft");
    expect(body).not.toContain("Enable it");
    expect(body).not.toContain("Set up AI assistance in Settings");
  });

  it("shows ready with no pointer and no restart hint (ticket 192)", () => {
    const { body } = render(QuickActionFormDialog, {
      props: {
        open: true,
        action: null,
        aiReady: true,
        onsave: vi.fn(),
        oncancel: vi.fn(),
      },
    });
    expect(body).toContain("AI draft");
    expect(body).toContain("qa-ai-request");
    expect(body).not.toContain("Enable it");
    expect(body).not.toContain("Set up AI assistance in Settings");
  });

  it("adds no motion or scroll machinery to the pointer (ticket 192)", () => {
    const pointerStart = DIALOG_SOURCE.indexOf("ai-setup");
    expect(pointerStart).toBeGreaterThan(-1);
    const pointer = DIALOG_SOURCE.slice(pointerStart, pointerStart + 2000);
    expect(pointer).not.toContain("smooth");
    expect(pointer).not.toContain("pulse");
    expect(pointer).not.toContain("scrollIntoView");
    expect(pointer).not.toContain("transition");
    expect(DIALOG_SOURCE).not.toMatch(/\/settings#/);
  });

  it("keeps the shared close wiring — the dialog still closes via oncancel", () => {
    expect(DIALOG_SOURCE).toContain("onclose={oncancel}");
    expect(DIALOG_SOURCE).toContain("onclick={oncancel}");
  });

  it("gives every AI button type button so none can submit the form", () => {
    const section = aiSection();
    const buttons = section.match(/<Button\b/g) ?? [];
    expect(buttons.length).toBeGreaterThan(0);
    expect(section).not.toMatch(/<Button(?![^>]*type="button")/);
  });

  it("leaves Escape alone — the AI key handler only reroutes Ctrl/Cmd+Enter", () => {
    const handlerStart = DIALOG_SOURCE.indexOf("function aiKeydown");
    const handlerEnd = DIALOG_SOURCE.indexOf("async function submit");
    expect(handlerStart).toBeGreaterThan(-1);
    expect(handlerEnd).toBeGreaterThan(handlerStart);
    const handler = DIALOG_SOURCE.slice(handlerStart, handlerEnd);
    expect(handler).toContain("Enter");
    expect(handler).not.toContain("Escape");
    expect(DIALOG_SOURCE).not.toContain("stopPropagation");
  });

  it("adds no overlay mechanics that could swallow close clicks", () => {
    const section = aiSection();
    expect(section).not.toContain("pointer-events");
    expect(section).not.toMatch(/position:\s*(fixed|absolute)/);
  });

  it("announces the applied draft for manual review", () => {
    expect(DIALOG_SOURCE).toContain('t("actionform.appliedManual")');
    expect(DIALOG_SOURCE).toContain('role="status"');
  });

  it("chains clarification answers with aspect keys and offers drafting anyway", () => {
    expect(DIALOG_SOURCE).toContain('t("actionform.continue")');
    expect(DIALOG_SOURCE).toContain("clarified choice [");
    expect(DIALOG_SOURCE).toContain("clarified choice:");
    expect(DIALOG_SOURCE).toContain("draft-anyway:");
    expect(DIALOG_SOURCE).toContain('t("actionform.draftAnyway")');
    expect(DIALOG_SOURCE).toContain('t("actionform.clarifyRound")');
    expect(DIALOG_SOURCE).not.toContain("Regenerate with selection");
  });
});
