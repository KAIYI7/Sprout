import { afterEach, describe, expect, it, vi } from "vitest";
import { mount, tick, unmount } from "svelte";
import QuickActionFormDialog from "./components/QuickActionFormDialog.svelte";
import { aiGenerateDraft, detectPrerequisites } from "./api";

vi.mock("./api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./api")>();
  return { ...actual, aiGenerateDraft: vi.fn(), detectPrerequisites: vi.fn() };
});

const mounted: ReturnType<typeof mount>[] = [];

afterEach(async () => {
  for (const component of mounted.splice(0)) await unmount(component);
  document.body.replaceChildren();
  vi.clearAllMocks();
});

function mountDraftDialog() {
  const onsave = vi.fn();
  const oncancel = vi.fn();
  const host = document.createElement("div");
  document.body.append(host);
  mounted.push(
    mount(QuickActionFormDialog, {
      target: host,
      props: { open: true, action: null, aiReady: true, onsave, oncancel },
    }),
  );
  return { onsave, host };
}

function typeRequest(host: HTMLElement, text: string) {
  const area = host.querySelector("#qa-ai-request") as HTMLTextAreaElement | null;
  expect(area).not.toBeNull();
  area!.value = text;
  area!.dispatchEvent(new Event("input", { bubbles: true }));
}

async function clickButton(host: HTMLElement, label: string) {
  const button = [...host.querySelectorAll("button")].find(
    (item) => item.textContent?.trim() === label,
  );
  expect(button).not.toBeUndefined();
  button!.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
  await tick();
}

describe("Prerequisite warn-never-block (ticket 200)", () => {
  it("warns with install guidance beside a draft that assumes Playwright", async () => {
    vi.mocked(aiGenerateDraft).mockResolvedValue({
      kind: "draft",
      draft: {
        shell: "powershell",
        command: "npx playwright test",
        assumptions: ["Requires Playwright to run"],
        affected_targets: [],
        explanation: "Runs the suite.",
        executed: false,
      },
    });
    vi.mocked(detectPrerequisites).mockResolvedValue([
      { name: "playwright", status: "not-found", version: null, detail: "Not found for playwright" },
    ]);
    const { host } = mountDraftDialog();
    await tick();
    typeRequest(host, "Draft a script using Playwright to test the page");
    await clickButton(host, "Generate draft");

    expect(vi.mocked(detectPrerequisites)).toHaveBeenCalledWith(["playwright"]);
    await vi.waitFor(() =>
      expect(host.textContent).toContain("Playwright was not found"),
    );
    expect(host.textContent).toContain("npm i -D playwright");
    // Never blocked: the draft still applies and the dialog still saves.
    expect(
      [...host.querySelectorAll("button")].some((b) => b.textContent?.trim() === "Use this draft"),
    ).toBe(true);
  });

  it("confirms present prerequisites quietly and keeps saving open", async () => {
    vi.mocked(aiGenerateDraft).mockResolvedValue({
      kind: "draft",
      draft: {
        shell: "cmd",
        command: "node build.js",
        assumptions: ["Requires Node.js"],
        affected_targets: [],
        explanation: "Builds the project.",
        executed: false,
      },
    });
    vi.mocked(detectPrerequisites).mockResolvedValue([
      { name: "node", status: "present", version: "22.1.0", detail: "Node.js 22.1.0 found" },
    ]);
    const { host } = mountDraftDialog();
    await tick();
    typeRequest(host, "Draft a build script with Node.js");
    await clickButton(host, "Generate draft");

    await vi.waitFor(() =>
      expect(host.textContent).toContain("Prerequisites verified"),
    );
    expect(host.textContent).toContain("22.1.0");
    expect(host.textContent).not.toContain("was not found");
  });

  it("surfaces detect results under unknown-prerequisite choices without leaking a draft", async () => {
    const command = "npx playwright test";
    vi.mocked(aiGenerateDraft).mockResolvedValue({
      kind: "clarify",
      message: "That depends on a module whose presence is unknown.",
      choices: ["Draft with built-in commands only", "I'll name what's installed below"],
      aspect: "unknown-prerequisite",
    });
    vi.mocked(detectPrerequisites).mockResolvedValue([
      { name: "playwright", status: "not-verifiable", version: null, detail: "Not verifiable — the check timed out." },
    ]);
    const { host } = mountDraftDialog();
    await tick();
    typeRequest(host, "Draft a script using Playwright to test the page");
    await clickButton(host, "Generate draft");

    await vi.waitFor(() => expect(host.textContent).toContain("Not verifiable"));
    expect(host.textContent).toContain("You can still save");
    expect(host.textContent).not.toContain(command);
    // Answering still regenerates: the grill controls stay live.
    const labels = [...host.querySelectorAll("button")].map((b) => b.textContent?.trim());
    expect(labels).toContain("Continue");
    expect(labels).toContain("Draft anyway");
    expect(labels).toContain("Dismiss");
  });
});
