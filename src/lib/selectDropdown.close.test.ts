import { afterEach, describe, expect, it, vi } from "vitest";
import { mount, tick, unmount } from "svelte";
import Select from "./components/Select.svelte";

const cleanups: Array<() => void> = [];

afterEach(() => {
  for (const fn of cleanups.splice(0)) fn();
});

function nextFrame() {
  return new Promise<void>((resolve) => {
    requestAnimationFrame(() => resolve());
  });
}

const OPTIONS = [
  { value: "a", label: "Alpha" },
  { value: "b", label: "Beta", title: "Second choice" },
  { value: "c", label: "Gamma", disabled: true, title: "Unavailable here" },
];

function mountSelect(value = "a", onchange: (v: string) => void = () => {}) {
  const host = document.createElement("div");
  document.body.appendChild(host);
  const instance = mount(Select, {
    target: host,
    props: {
      id: "demo-select",
      value,
      options: OPTIONS,
      onchange,
      "aria-label": "Demo choice",
    },
  });
  cleanups.push(() => {
    unmount(instance);
    host.remove();
  });
  return host;
}

function trigger(host: HTMLElement) {
  return host.querySelector("#demo-select") as HTMLButtonElement;
}

function menuItems() {
  return [...document.querySelectorAll(".ctx-menu .ctx-item")] as HTMLButtonElement[];
}

describe("Select trigger", () => {
  it("renders the current option label with menu semantics and a value readout", async () => {
    const host = mountSelect("b");
    await tick();
    const btn = trigger(host);
    expect(btn.tagName).toBe("BUTTON");
    expect(btn.textContent).toContain("Beta");
    expect(btn.dataset.value).toBe("b");
    expect(btn.getAttribute("aria-haspopup")).toBe("menu");
    expect(btn.getAttribute("aria-expanded")).toBe("false");
    expect(document.querySelector(".ctx-menu")).toBeNull();
  });

  it("keeps the external label association on the trigger", async () => {
    const host = mountSelect();
    await tick();
    expect(trigger(host).id).toBe("demo-select");
  });
});

describe("Select popout", () => {
  it("opens the themed popout on click with the current choice checked", async () => {
    const host = mountSelect();
    await tick();
    trigger(host).click();
    await tick();
    await nextFrame();
    await tick();

    const items = menuItems();
    expect(items.map((item) => item.textContent?.trim())).toEqual([
      "Alpha",
      "Beta",
      "Gamma",
    ]);
    const checked = items.filter(
      (item) => item.getAttribute("aria-checked") === "true",
    );
    expect(checked.map((item) => item.textContent?.trim())).toEqual(["Alpha"]);
    expect(items[0].getAttribute("role")).toBe("menuitemradio");
  });

  it("picks an option, reports its value, and closes", async () => {
    const onchange = vi.fn();
    const host = mountSelect("a", onchange);
    await tick();
    trigger(host).click();
    await tick();
    await nextFrame();
    await tick();

    menuItems()[1].click();
    await tick();
    expect(onchange).toHaveBeenCalledExactlyOnceWith("b");
    expect(document.querySelector(".ctx-menu")).toBeNull();
  });

  it("never picks a disabled row", async () => {
    const onchange = vi.fn();
    const host = mountSelect("a", onchange);
    await tick();
    trigger(host).click();
    await tick();
    await nextFrame();
    await tick();

    const items = menuItems();
    expect(items[2].disabled).toBe(true);
    items[2].click();
    await tick();
    expect(onchange).not.toHaveBeenCalled();
    expect(document.querySelector(".ctx-menu")).not.toBeNull();
  });

  it("carries option title tooltips into the popout rows", async () => {
    const host = mountSelect();
    await tick();
    trigger(host).click();
    await tick();
    await nextFrame();
    await tick();

    const items = menuItems();
    expect(items[1].getAttribute("title")).toBe("Second choice");
    expect(items[2].getAttribute("title")).toBe("Unavailable here");
  });

  it("moves focus into the menu on keyboard open and restores it on Escape", async () => {
    const host = mountSelect();
    await tick();
    const btn = trigger(host);
    btn.focus();
    // HTMLElement.click() carries detail 0 — the same path a real
    // Enter/Space keypress takes through the trigger.
    btn.click();
    await tick();
    await nextFrame();
    await tick();

    const items = menuItems();
    expect(document.activeElement).toBe(items[0]);
    document.dispatchEvent(
      new KeyboardEvent("keydown", { key: "Escape", bubbles: true }),
    );
    await tick();
    expect(document.querySelector(".ctx-menu")).toBeNull();
    expect(document.activeElement).toBe(btn);
  });
});
