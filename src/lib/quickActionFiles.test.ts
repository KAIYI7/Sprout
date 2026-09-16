import { describe, expect, it } from "vitest";
import {
  base64ToBytes,
  buildStoredZip,
  bytesToBase64,
  detectCmdStartTitleTrap,
  downloadFilters,
} from "./quickActionFiles";

describe("file bytes base64 round-trip", () => {
  it("round-trips text and every byte value", () => {
    expect(base64ToBytes(bytesToBase64(new Uint8Array([])))).toEqual(new Uint8Array([]));
    const text = new TextEncoder().encode("hello files");
    expect(base64ToBytes(bytesToBase64(text))).toEqual(text);
    const all = new Uint8Array(256);
    for (let i = 0; i < 256; i += 1) all[i] = i;
    expect(base64ToBytes(bytesToBase64(all))).toEqual(all);
  });
});

describe("stored zip bundle", () => {
  function parseEntries(zip: Uint8Array): { name: string; bytes: Uint8Array }[] {
    const view = new DataView(zip.buffer, zip.byteOffset, zip.byteLength);
    expect(view.getUint32(0, true)).toBe(0x04034b50);
    const out: { name: string; bytes: Uint8Array }[] = [];
    let at = 0;
    while (view.getUint32(at, true) === 0x04034b50) {
      const method = view.getUint16(at + 8, true);
      expect(method).toBe(0);
      const size = view.getUint32(at + 18, true);
      const nameLen = view.getUint16(at + 26, true);
      const extraLen = view.getUint16(at + 28, true);
      const name = new TextDecoder().decode(zip.subarray(at + 30, at + 30 + nameLen));
      const start = at + 30 + nameLen + extraLen;
      out.push({ name, bytes: zip.subarray(start, start + size) });
      at = start + size;
    }
    return out;
  }

  it("keeps single-file bytes verbatim with its name", () => {
    const bytes = new Uint8Array([0, 1, 2, 255, 254]);
    const zip = buildStoredZip([{ filename: "clip.mp3", bytes }]);
    const entries = parseEntries(zip);
    expect(entries.length).toBe(1);
    expect(entries[0].name).toBe("clip.mp3");
    expect(entries[0].bytes).toEqual(bytes);
  });

  it("bundles several files in the given order, binary intact", () => {
    const pdf = new TextEncoder().encode("%PDF-1.7 payload");
    const mp3 = new Uint8Array([0x49, 0x44, 0x33, 0xff, 0x00]);
    const zip = buildStoredZip([
      { filename: "doc.pdf", bytes: pdf },
      { filename: "clip.mp3", bytes: mp3 },
    ]);
    const entries = parseEntries(zip);
    expect(entries.map((e) => e.name)).toEqual(["doc.pdf", "clip.mp3"]);
    expect(entries[0].bytes).toEqual(pdf);
    expect(entries[1].bytes).toEqual(mp3);
  });

  it("is deterministic and rejects non-plain names", () => {
    const bytes = new TextEncoder().encode("x");
    const first = buildStoredZip([{ filename: "a.txt", bytes }]);
    const second = buildStoredZip([{ filename: "a.txt", bytes }]);
    expect(first).toEqual(second);
    expect(() => buildStoredZip([{ filename: "sub/dir.txt", bytes }])).toThrow();
    expect(() => buildStoredZip([{ filename: "", bytes }])).toThrow();
  });

  it("points the end record at the real central directory", () => {
    // Regression: the end record once wrote the directory offset two bytes
    // early, overlapping the directory size — strict openers (Windows
    // Explorer, Python zipfile) rejected the bundle while the local-header
    // walk above stayed green.
    const zip = buildStoredZip([
      { filename: "doc.pdf", bytes: new TextEncoder().encode("%PDF-1.4 staged") },
      { filename: "clip.mp3", bytes: new Uint8Array([0x49, 0x44, 0x33]) },
    ]);
    const view = new DataView(zip.buffer, zip.byteOffset, zip.byteLength);
    const end = zip.length - 22;
    expect(view.getUint32(end, true)).toBe(0x06054b50);
    const count = view.getUint16(end + 10, true);
    const cdSize = view.getUint32(end + 12, true);
    const cdOffset = view.getUint32(end + 16, true);
    expect(count).toBe(2);
    expect(view.getUint32(cdOffset, true)).toBe(0x02014b50);
    expect(cdOffset + cdSize).toBe(end);
    let at = cdOffset;
    for (let i = 0; i < count; i += 1) {
      expect(view.getUint32(at, true)).toBe(0x02014b50);
      expect(view.getUint32(view.getUint32(at + 42, true), true)).toBe(0x04034b50);
      at += 46 + view.getUint16(at + 28, true);
    }
    expect(at).toBe(end);
  });
});

describe("download picker filters", () => {
  it("leads with the file's own extension", () => {
    expect(downloadFilters("report.pdf")[0].extensions).toEqual(["pdf"]);
    expect(downloadFilters("clip.mp3")[0].extensions).toEqual(["mp3"]);
    expect(downloadFilters("noext")[0].extensions).toEqual(["*"]);
  });
});

describe("cmd start title-trap lint", () => {
  it("warns on a quoted path or FilesDir with no empty title", () => {
    expect(detectCmdStartTitleTrap(`start "<FilesDir>\\a.txt"`, "cmd")).toBe(true);
    expect(detectCmdStartTitleTrap(`start <FilesDir>\\a.txt`, "cmd")).toBe(true);
    expect(detectCmdStartTitleTrap(`start "C:\\x\\file.pdf"`, "cmd")).toBe(true);
    expect(detectCmdStartTitleTrap(`start /wait <FilesDir>\\a.txt`, "cmd")).toBe(true);
  });

  it("stays quiet on the fixed and unrelated shapes", () => {
    expect(detectCmdStartTitleTrap(`start "" "<FilesDir>\\a.txt"`, "cmd")).toBe(false);
    expect(detectCmdStartTitleTrap(`start "" "C:\\x\\file.pdf"`, "cmd")).toBe(false);
    expect(detectCmdStartTitleTrap(`start /wait "" "<FilesDir>\\a.txt"`, "cmd")).toBe(false);
    expect(detectCmdStartTitleTrap(`start "My Title" notepad`, "cmd")).toBe(false);
    expect(detectCmdStartTitleTrap(`start notepad.exe`, "cmd")).toBe(false);
    expect(detectCmdStartTitleTrap(`start`, "cmd")).toBe(false);
    expect(detectCmdStartTitleTrap(`start /d "C:\\dir" notepad`, "cmd")).toBe(false);
    expect(detectCmdStartTitleTrap(`REM start <FilesDir>\\a.txt`, "cmd")).toBe(false);
    expect(detectCmdStartTitleTrap(`:: start <FilesDir>\\a.txt`, "cmd")).toBe(false);
    expect(detectCmdStartTitleTrap(`echo start`, "cmd")).toBe(false);
  });

  it("never fires outside the cmd shell", () => {
    expect(detectCmdStartTitleTrap(`start "<FilesDir>\\a.txt"`, "powershell")).toBe(false);
    expect(detectCmdStartTitleTrap(`start "<FilesDir>\\a.txt"`, "python3")).toBe(false);
    expect(detectCmdStartTitleTrap(`Start-Process <FilesDir>\\a.txt`, "powershell")).toBe(false);
    expect(detectCmdStartTitleTrap(`explorer <FilesDir>\\a.txt`, "cmd")).toBe(false);
  });
});
