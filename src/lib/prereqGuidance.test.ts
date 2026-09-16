import { describe, expect, it } from "vitest";
import { extractPrereqKeys, prereqLabel, prereqLines } from "./prereqGuidance";
import type { PrerequisiteVerdict } from "./types";

function verdict(
  name: string,
  status: PrerequisiteVerdict["status"],
  version: string | null = null,
  detail = "",
): PrerequisiteVerdict {
  return { name, status, version, detail };
}

describe("extractPrereqKeys", () => {
  it("names runtimes whole-word in either shell's prose", () => {
    expect(extractPrereqKeys(["Requires Node.js to run"])).toEqual(["node"]);
    expect(extractPrereqKeys(["Needs nodejs for the script"])).toEqual(["node"]);
    expect(extractPrereqKeys(["Uses python3 for parsing"])).toEqual(["python"]);
    expect(extractPrereqKeys(["Runs Playwright tests"])).toEqual(["playwright"]);
  });

  it("passes explicit winget and extension keys through", () => {
    expect(extractPrereqKeys(["Needs winget:Git.Git on PATH"])).toEqual([
      "winget:Git.Git",
    ]);
    expect(extractPrereqKeys(["Needs extension:ms-python.python"])).toEqual([
      "extension:ms-python.python",
    ]);
  });

  it("dedupes across assumptions and keeps first-seen order", () => {
    expect(
      extractPrereqKeys(["Requires Node.js", "node plus Playwright", "Node.js again"]),
    ).toEqual(["node", "playwright"]);
  });

  it("never invents keys from unknown prose", () => {
    expect(extractPrereqKeys(["Requires the XYZ-Cloud module"])).toEqual([]);
    expect(extractPrereqKeys(["Needs SomeVendor Thing 12"])).toEqual([]);
    expect(extractPrereqKeys([])).toEqual([]);
  });
});

describe("prereqLines", () => {
  it("warns with an install command for not-found, naming both shells' targets", () => {
    const [node, playwright, winget, extension] = prereqLines([
      verdict("node", "not-found", null, "Not found for node"),
      verdict("playwright", "not-found", null, "Not found for playwright"),
      verdict("winget:Git.Git", "not-found", null, "Not found for winget:Git.Git"),
      verdict("extension:ms-python.python", "not-found", null, "Not found"),
    ]);
    expect(node.text).toContain("Node.js was not found");
    expect(node.text).toContain("winget install OpenJS.NodeJS.LTS");
    expect(playwright.text).toContain("npm i -D playwright");
    expect(winget.text).toContain("winget install Git.Git");
    expect(extension.text).toContain("marketplace");
    for (const line of [node, playwright, winget, extension]) {
      expect(line.blocking).toBe(false);
    }
  });

  it("surfaces not-verifiable wording honestly, never a fabricated version", () => {
    const [line] = prereqLines([
      verdict("python", "not-verifiable", null, "Not verifiable — offline model, I cannot search online"),
    ]);
    expect(line.text).toContain("Not verifiable");
    expect(line.text).toContain("You can still save");
    expect(line.text).not.toContain("3.");
  });

  it("confirms present prerequisites quietly with the checked version", () => {
    const [withVersion, pathOnly] = prereqLines([
      verdict("node", "present", "22.1.0", "Node.js 22.1.0 found"),
      verdict("playwright", "present", null, "Found on PATH"),
    ]);
    expect(withVersion.text).toContain("22.1.0");
    expect(pathOnly.status).toBe("present");
  });

  it("labels every closed-catalog key", () => {
    expect(prereqLabel("node")).toBe("Node.js");
    expect(prereqLabel("python")).toBe("Python 3");
    expect(prereqLabel("playwright")).toBe("Playwright");
    expect(prereqLabel("winget:Git.Git")).toContain("Git.Git");
    expect(prereqLabel("extension:ms-python.python")).toContain("ms-python.python");
  });
});
