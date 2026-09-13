import { afterEach, describe, expect, it, vi } from "vitest";
import { mount, tick } from "svelte";
import QuickActionFormDialog from "./components/QuickActionFormDialog.svelte";

const hosts: HTMLElement[] = [];

afterEach(() => {
  for (const host of hosts.splice(0)) host.remove();
});

function mountDialog(props: Record<string, unknown>) {
  const host = document.createElement("div");
  document.body.appendChild(host);
  hosts.push(host);
  mount(QuickActionFormDialog, {
    target: host,
    props: {
      open: true,
      action: null,
      onsave: vi.fn(),
      oncancel: vi.fn(),
      ...props,
    },
  });
  return host;
}

async function click(el: Element) {
  el.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
  await tick();
}

describe("Quick Action setup pointer (ticket 192)", () => {
  it("routes the managed pointer through onsetupai with zero AI tabs", async () => {
    const onsetupai = vi.fn();
    const host = mountDialog({ aiReady: false, aiSetupKind: "managed", onsetupai });
    await tick();
    expect(host.textContent).toContain("No managed model is on right now");
    expect([...host.querySelectorAll('[role="tab"]')].length).toBe(0);
    const enable = [...host.querySelectorAll("button")].find(
      (item) => item.textContent?.trim() === "Enable it",
    );
    expect(enable).not.toBeUndefined();
    // A real button: keyboard and screen-reader operable, never a submit.
    expect(enable).toBeInstanceOf(HTMLButtonElement);
    expect(enable!.getAttribute("type")).toBe("button");
    await click(enable!);
    expect(onsetupai).toHaveBeenCalledTimes(1);
  });

  it("routes the generic pointer through onsetupai", async () => {
    const onsetupai = vi.fn();
    const host = mountDialog({ aiReady: false, aiSetupKind: "generic", onsetupai });
    await tick();
    const setup = [...host.querySelectorAll("button")].find(
      (item) => item.textContent?.trim() === "Set up AI assistance in Settings",
    );
    expect(setup).not.toBeUndefined();
    await click(setup!);
    expect(onsetupai).toHaveBeenCalledTimes(1);
  });

  it("shows no pointer once ready — stopped keeps the tab with a restart hint", async () => {
    const onsetupai = vi.fn();
    const host = mountDialog({ aiReady: true, aiRuntimeStopped: true, onsetupai });
    await tick();
    expect(host.textContent).toContain("AI draft");
    expect(host.textContent).toContain("Stopped — Generate will restart.");
    expect(host.textContent).not.toContain("Enable it");
    expect(host.textContent).not.toContain("Set up AI assistance in Settings");
    expect(onsetupai).not.toHaveBeenCalled();
  });
});
