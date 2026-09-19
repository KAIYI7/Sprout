import { readFileSync } from "node:fs";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { saveMock, writeFileMock, getFileMock, listFilesMock } = vi.hoisted(() => ({
  saveMock: vi.fn(),
  writeFileMock: vi.fn(),
  getFileMock: vi.fn(),
  listFilesMock: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({ save: saveMock }));
vi.mock("@tauri-apps/plugin-fs", () => ({ writeFile: writeFileMock }));
vi.mock("./api", () => ({
  getQuickActionFile: getFileMock,
  listQuickActionFiles: listFilesMock,
}));

import { downloadFilesZip, downloadSingleFile } from "./quickActionDownload";
import { bytesToBase64 } from "./quickActionFiles";

const DIALOG_SOURCE = readFileSync(
  new URL("./components/QuickActionFormDialog.svelte", import.meta.url),
  "utf8",
);
const PAGE_SOURCE = readFileSync(
  new URL("../routes/quick-actions/+page.svelte", import.meta.url),
  "utf8",
);

beforeEach(() => {
  saveMock.mockReset();
  writeFileMock.mockReset();
  getFileMock.mockReset();
  listFilesMock.mockReset();
});

describe("single-file Download flow", () => {
  it("stays silent on Save-As cancel and never touches the disk", async () => {
    saveMock.mockResolvedValue(null);
    const result = await downloadSingleFile(7, "clip.mp3");
    expect(result).toBe("cancelled");
    expect(getFileMock).not.toHaveBeenCalled();
    expect(writeFileMock).not.toHaveBeenCalled();
  });

  it("writes the exact bytes to the chosen path", async () => {
    saveMock.mockResolvedValue("/tmp/clip.mp3");
    const bytes = new Uint8Array([0x49, 0x44, 0x33, 0x00, 0xff]);
    getFileMock.mockResolvedValue({ filename: "clip.mp3", bytes_base64: bytesToBase64(bytes) });
    const result = await downloadSingleFile(7, "clip.mp3");
    expect(result).toBe("saved");
    expect(writeFileMock).toHaveBeenCalledTimes(1);
    expect(writeFileMock.mock.calls[0][0]).toBe("/tmp/clip.mp3");
    expect(writeFileMock.mock.calls[0][1]).toEqual(bytes);
  });
});

describe("Download-all-zip flow", () => {
  it("never opens the dialog while fewer than two files persist", async () => {
    listFilesMock.mockResolvedValue([{ id: 1, filename: "a.txt" }]);
    const result = await downloadFilesZip(3, "demo");
    expect(result).toBe("empty");
    expect(saveMock).not.toHaveBeenCalled();
    expect(writeFileMock).not.toHaveBeenCalled();
  });

  it("stays silent on Save-As cancel", async () => {
    listFilesMock.mockResolvedValue([
      { id: 1, filename: "a.txt" },
      { id: 2, filename: "b.txt" },
    ]);
    saveMock.mockResolvedValue(null);
    const result = await downloadFilesZip(3, "demo");
    expect(result).toBe("cancelled");
    expect(getFileMock).not.toHaveBeenCalled();
    expect(writeFileMock).not.toHaveBeenCalled();
  });

  it("zips every persisted file in list order", async () => {
    listFilesMock.mockResolvedValue([
      { id: 1, filename: "a.txt" },
      { id: 2, filename: "b.txt" },
    ]);
    saveMock.mockResolvedValue("/tmp/demo-files.zip");
    getFileMock.mockImplementation(async (id: number) =>
      id === 1
        ? { filename: "a.txt", bytes_base64: bytesToBase64(new TextEncoder().encode("aaa")) }
        : { filename: "b.txt", bytes_base64: bytesToBase64(new TextEncoder().encode("b")) },
    );
    const result = await downloadFilesZip(3, "demo");
    expect(result).toBe("saved");
    const written: Uint8Array = writeFileMock.mock.calls[0][1];
    expect(written[0]).toBe(0x50);
    expect(written[1]).toBe(0x4b);
    const raw = new TextDecoder().decode(written);
    expect(raw).toContain("a.txt");
    expect(raw).toContain("b.txt");
    expect(raw).toContain("aaa");
  });
});

describe("Download + lint wiring pins", () => {
  it("keeps per-file Download beside Remove plus the zip threshold", () => {
    expect(DIALOG_SOURCE).toContain('t("menu.download")');
    expect(DIALOG_SOURCE).toContain('t("menu.downloadAll")');
    expect(DIALOG_SOURCE).toContain("persistedFileCount >= 2");
    expect(DIALOG_SOURCE).toContain("editing && row.id !== null");
  });

  it("keeps the row-menu Download flyout with the zip threshold", () => {
    expect(PAGE_SOURCE).toContain('t("menu.downloadAll")');
    expect(PAGE_SOURCE).toContain("downloadRowFile");
    expect(PAGE_SOURCE).toContain("files.length >= 2");
  });

  it("keeps the cmd start title-trap warning with its fixed forms", () => {
    expect(DIALOG_SOURCE).toContain('t("actionform.startTrap")');
    expect(DIALOG_SOURCE).toContain('t("actionform.startTrap")');
    expect(DIALOG_SOURCE).toContain('t("actionform.startTrap")');
  });
});
