/**
 * Download + cmd `start` lint helpers for Quick Action files (ticket 210).
 *
 * Pure functions only — no DOM, no Tauri invokes — so every rule below is
 * unit-testable. The dialog and the row menu own the Save-As dialog and the
 * file writes; this module owns the bytes math and the trap shape. Removing
 * file Download removes this module plus its dialog/page wiring.
 */

/** Decodes backend base64 file bytes to raw bytes for saving or zipping. */
export function base64ToBytes(b64: string): Uint8Array {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i += 1) {
    out[i] = bin.charCodeAt(i);
  }
  return out;
}

/** Encodes raw bytes to base64 (tests + any future upload path). */
export function bytesToBase64(bytes: Uint8Array): string {
  let bin = "";
  const CHUNK = 0x8000;
  for (let i = 0; i < bytes.length; i += CHUNK) {
    bin += String.fromCharCode(...bytes.subarray(i, i + CHUNK));
  }
  return btoa(bin);
}

const CRC_TABLE = (() => {
  const table = new Uint32Array(256);
  for (let n = 0; n < 256; n += 1) {
    let c = n;
    for (let k = 0; k < 8; k += 1) {
      c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    }
    table[n] = c >>> 0;
  }
  return table;
})();

function crc32(bytes: Uint8Array): number {
  let crc = 0xffffffff;
  for (let i = 0; i < bytes.length; i += 1) {
    crc = CRC_TABLE[(crc ^ bytes[i]) & 0xff] ^ (crc >>> 8);
  }
  return (crc ^ 0xffffffff) >>> 0;
}

export interface ZipEntry {
  filename: string;
  bytes: Uint8Array;
}

/**
 * Builds a minimal stored (uncompressed) zip bundle from persisted-file
 * bytes: local headers + central directory + end record, UTF-8 filenames, a
 * fixed zero DOS timestamp so the same inputs always produce the same bytes.
 * Stored means each file's bytes appear verbatim — a byte-identical round
 * trip without a compression dependency. Filenames arrive basename-only from
 * the backend contract, so no path can escape the bundle.
 */
export function buildStoredZip(entries: ZipEntry[]): Uint8Array {
  const encoder = new TextEncoder();
  const names = entries.map((entry) => {
    if (!entry.filename || entry.filename.includes("/") || entry.filename.includes("\\")) {
      throw new Error(`'${entry.filename}' is not a plain file name.`);
    }
    return encoder.encode(entry.filename);
  });
  const cdSize = names.reduce((n, name) => n + 46 + name.length, 0);
  const total =
    entries.reduce((n, entry, i) => n + 30 + names[i].length + entry.bytes.length, 0) +
    cdSize +
    22;
  const out = new Uint8Array(total);
  const view = new DataView(out.buffer);
  let at = 0;
  const offsets: number[] = [];
  entries.forEach((entry, i) => {
    const name = names[i];
    const crc = crc32(entry.bytes);
    offsets.push(at);
    view.setUint32(at, 0x04034b50, true);
    view.setUint16(at + 4, 20, true);
    view.setUint16(at + 6, 0x0800, true);
    view.setUint16(at + 8, 0, true);
    view.setUint16(at + 10, 0, true);
    view.setUint16(at + 12, 0, true);
    view.setUint32(at + 14, crc, true);
    view.setUint32(at + 18, entry.bytes.length, true);
    view.setUint32(at + 22, entry.bytes.length, true);
    view.setUint16(at + 26, name.length, true);
    view.setUint16(at + 28, 0, true);
    at += 30;
    out.set(name, at);
    at += name.length;
    out.set(entry.bytes, at);
    at += entry.bytes.length;
  });
  const cdStart = at;
  entries.forEach((entry, i) => {
    const name = names[i];
    const crc = crc32(entry.bytes);
    view.setUint32(at, 0x02014b50, true);
    view.setUint16(at + 4, 20, true);
    view.setUint16(at + 6, 20, true);
    view.setUint16(at + 8, 0x0800, true);
    view.setUint16(at + 10, 0, true);
    view.setUint16(at + 12, 0, true);
    view.setUint16(at + 14, 0, true);
    view.setUint32(at + 16, crc, true);
    view.setUint32(at + 20, entry.bytes.length, true);
    view.setUint32(at + 24, entry.bytes.length, true);
    view.setUint16(at + 28, name.length, true);
    view.setUint16(at + 30, 0, true);
    view.setUint16(at + 32, 0, true);
    view.setUint16(at + 34, 0, true);
    view.setUint16(at + 36, 0, true);
    view.setUint32(at + 38, 0, true);
    view.setUint32(at + 42, offsets[i], true);
    at += 46;
    out.set(name, at);
    at += name.length;
  });
  view.setUint32(at, 0x06054b50, true);
  view.setUint16(at + 4, 0, true);
  view.setUint16(at + 6, 0, true);
  view.setUint16(at + 8, entries.length, true);
  view.setUint16(at + 10, entries.length, true);
  view.setUint32(at + 12, cdSize, true);
  view.setUint32(at + 16, cdStart, true);
  view.setUint16(at + 20, 0, true);
  return out;
}

/** Save-As picker filters for one downloaded file: its own extension leads. */
export function downloadFilters(filename: string): { name: string; extensions: string[] }[] {
  const dot = filename.lastIndexOf(".");
  const ext = dot > 0 ? filename.slice(dot + 1).toLowerCase() : "";
  if (!ext || /[^a-z0-9]/.test(ext)) {
    return [{ name: "All files", extensions: ["*"] }];
  }
  return [
    { name: `${ext.toUpperCase()} file`, extensions: [ext] },
    { name: "All files", extensions: ["*"] },
  ];
}

function splitStatements(line: string): string[] {
  const parts: string[] = [];
  let current = "";
  let inQuotes = false;
  for (let i = 0; i < line.length; i += 1) {
    const c = line[i];
    if (c === '"') {
      inQuotes = !inQuotes;
      current += c;
      continue;
    }
    if (!inQuotes && (c === "&" || c === "|")) {
      parts.push(current);
      current = "";
      if (line[i + 1] === c) i += 1;
      continue;
    }
    current += c;
  }
  parts.push(current);
  return parts;
}

function splitWords(statement: string): string[] {
  const words: string[] = [];
  let current = "";
  let inQuotes = false;
  const flush = () => {
    if (current) {
      words.push(current);
      current = "";
    }
  };
  for (let i = 0; i < statement.length; i += 1) {
    const c = statement[i];
    if (c === '"') {
      inQuotes = !inQuotes;
      current += c;
      continue;
    }
    if (!inQuotes && (c === " " || c === "\t" || c === "\r")) {
      flush();
      continue;
    }
    current += c;
  }
  flush();
  return words;
}

function isSwitchWithArg(word: string): boolean {
  const lower = word.toLowerCase();
  return lower === "/d" || lower === "/node" || lower === "/affinity";
}

/**
 * Whether the command holds a cmd `start` title-trap: a `start` line under
 * the cmd shell whose first quoted path (or `<FilesDir>` reference, which the
 * run quotes) arrives with no empty title, so cmd misreads the path as the
 * window title and opens nothing. Fires only on that shape — `start "" …`
 * (and intentional `start "Title" <command>`, bare `start`, unquoted runs,
 * comments, quoted mentions, and every non-cmd shell) stays quiet.
 */
export function detectCmdStartTitleTrap(command: string, shell: string): boolean {
  if (shell !== "cmd") return false;
  for (const rawLine of command.split("\n")) {
    let line = rawLine.replace(/\r$/, "");
    const stripped = line.replace(/^[\s(@(]+/, "");
    const lower = stripped.toLowerCase();
    if (lower.startsWith("rem") && (stripped.length === 3 || /[\s&|();,=]/.test(stripped[3]))) {
      continue;
    }
    if (stripped.startsWith("::")) continue;
    for (const statement of splitStatements(line)) {
      const cleaned = statement.replace(/^[\s@(]+/, "");
      if (!/^start(\s|$)/i.test(cleaned)) continue;
      const rest = cleaned.replace(/^start/i, "");
      const args = splitWords(rest);
      let i = 0;
      while (i < args.length && args[i].startsWith("/")) {
        if (isSwitchWithArg(args[i])) i += 2;
        else i += 1;
      }
      const first = args[i];
      if (first === undefined) continue;
      if (first === '""') continue;
      const quoted = first.startsWith('"');
      const filesDir = first.toLowerCase().includes("<filesdir>");
      if (!quoted && !filesDir) continue;
      if (i + 1 < args.length) continue;
      return true;
    }
  }
  return false;
}
