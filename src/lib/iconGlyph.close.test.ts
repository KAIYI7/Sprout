import { describe, expect, it } from "vitest";
import { mount, unmount } from "svelte";
import Icon from "./components/Icon.svelte";

function render(name: string): string {
  const host = document.createElement("div");
  document.body.appendChild(host);
  const instance = mount(Icon, { target: host, props: { name } });
  const html = host.innerHTML;
  unmount(instance);
  host.remove();
  return html;
}

describe("Icon glyphs", () => {
  it("renders a distinct export glyph (never the download glyph)", () => {
    const exported = render("export");
    const downloaded = render("download");
    expect(exported).toContain("<svg");
    expect(exported).toContain("<path");
    expect(exported).not.toBe(downloaded);
  });

  it("renders nothing for unknown names", () => {
    expect(render("no-such-icon")).not.toContain("<path");
  });
});
