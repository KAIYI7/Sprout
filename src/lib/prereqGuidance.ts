import type { PrerequisiteVerdict } from "./types";

/** Detectable prerequisite keys (ticket 200): the closed v1 catalog from
 *  ticket 199 that the dialog can name without guessing — two runtimes, the
 *  Playwright probe, and explicit `winget:` / `extension:` references.
 *  Anything else stays unverified prose in `assumptions`, never a verdict. */
export type PrereqKey = string;

const RUNTIME_ALIASES: Record<string, string> = {
  node: "node",
  nodejs: "node",
  python: "python",
  python3: "python",
  py: "python",
  playwright: "playwright",
};

const ID = "[A-Za-z0-9._-]+";

/** Names the detectable prerequisites inside free text (draft assumptions or
 *  the typed request): whole-word runtime matches plus explicit `winget:` /
 *  `extension:` keys, deduplicated in first-seen order. Returns [] when
 *  nothing in the closed catalog is named — the caller then makes no detect
 *  call at all, so unknown prose never becomes a fabricated verdict. */
export function extractPrereqKeys(texts: string[]): PrereqKey[] {
  const seen = new Set<string>();
  const keys: string[] = [];
  const push = (key: string) => {
    if (!seen.has(key)) {
      seen.add(key);
      keys.push(key);
    }
  };
  for (const text of texts) {
    // Explicit keys first: their spans are blanked before runtime matching
    // so an id like `ms-python.python` names the extension only — the bare
    // `python` runtime is claimed solely by its own word elsewhere.
    let rest = text;
    for (const match of text.matchAll(new RegExp(`\\bwinget:(${ID})`, "gi"))) {
      push(`winget:${match[1]}`);
      rest = rest.replace(match[0], " ".repeat(match[0].length));
    }
    for (const match of text.matchAll(new RegExp(`\\bextension:(${ID})`, "gi"))) {
      push(`extension:${match[1]}`);
      rest = rest.replace(match[0], " ".repeat(match[0].length));
    }
    const lowered = rest.toLowerCase();
    for (const [alias, key] of Object.entries(RUNTIME_ALIASES)) {
      const at = lowered.search(new RegExp(`(^|[^A-Za-z0-9])${alias}([^A-Za-z0-9]|$)`, "i"));
      if (at !== -1) push(key);
    }
  }
  return keys;
}

/** The dialog's display name for one key. `winget:` / `extension:` keys echo
 *  the author's own id — nothing is resolved or invented here. */
export function prereqLabel(key: PrereqKey): string {
  if (key === "node") return "Node.js";
  if (key === "python") return "Python 3";
  if (key === "playwright") return "Playwright";
  const winget = key.match(/^winget:(.+)$/);
  if (winget) return `The \`${winget[1]}\` package`;
  const extension = key.match(/^extension:(.+)$/);
  if (extension) return `The \`${extension[1]}\` editor extension`;
  return key;
}

function installGuidance(key: PrereqKey): string {
  if (key === "node")
    return "Install the LTS with `winget install OpenJS.NodeJS.LTS`, then generate again.";
  if (key === "python")
    return "Install it from python.org or the Microsoft Store, then generate again.";
  if (key === "playwright")
    return "Install it with `npm i -D playwright` and `npx playwright install`, then generate again.";
  const winget = key.match(/^winget:(.+)$/);
  if (winget) return `Install it with \`winget install ${winget[1]}\`, then generate again.`;
  const extension = key.match(/^extension:(.+)$/);
  if (extension)
    return "Install it from the editor's marketplace, then generate again.";
  return "Install it, then generate again.";
}

export interface PrereqLine {
  key: PrereqKey;
  status: PrerequisiteVerdict["status"];
  /** One or two concise sentences (research 0017 selective guidance):
   *  the constraint plus the install command — never a tutorial. */
  text: string;
  /** True while the draft still saves untouched beside the warning. */
  blocking: false;
}

/** Turns verified detect results into warn-never-block dialog lines: missing
 *  prerequisites carry install guidance, uncertain ones surface the backend's
 *  honest wording verbatim (never a fabricated version or path), and present
 *  ones collapse to a single quiet confirmation. */
export function prereqLines(verdicts: PrerequisiteVerdict[]): PrereqLine[] {
  return verdicts.map((verdict) => {
    const label = prereqLabel(verdict.name);
    if (verdict.status === "present") {
      const found = verdict.version ? `${label} ${verdict.version} found` : `${label} found`;
      return {
        key: verdict.name,
        status: verdict.status,
        text: `Prerequisites verified — ${found}.`,
        blocking: false,
      };
    }
    if (verdict.status === "not-found") {
      return {
        key: verdict.name,
        status: verdict.status,
        text: `${label} was not found. ${installGuidance(verdict.name)}`,
        blocking: false,
      };
    }
    return {
      key: verdict.name,
      status: verdict.status,
      text: `${verdict.detail} You can still save — generate again to re-check.`,
      blocking: false,
    };
  });
}
