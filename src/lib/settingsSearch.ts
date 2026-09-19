/// Settings-local filter index: search before hierarchy (research 0014 rule 6).
///
/// The Settings page stays one route with four Disclosure groups; when the
/// page outgrows scanning, a local filter narrows groups to matches instead of
/// adding navigation depth. Entries are data — a future knob joins by adding
/// one entry here, never by special-casing the matcher.
///
/// Display labels and descriptions resolve through the copy dictionary so the
/// filter surface follows the active locale live; synonyms and value texts
/// stay matching data and are never rendered alone.

import { t, tCount } from "./copy";

export type SettingsGroupKey = "general" | "dock" | "companion" | "backup" | "ai";

export const SETTINGS_GROUPS: { key: SettingsGroupKey; label: string }[] = [
  { key: "general", label: "General" },
  { key: "dock", label: "Dock" },
  { key: "companion", label: "Companion" },
  { key: "backup", label: "Backup & housekeeping" },
  { key: "ai", label: "AI assistance" },
];

/// Knob ids per group, backing the section count badges and the resolver.
export const SETTINGS_GROUP_KNOBS: Record<SettingsGroupKey, string[]> = {
  general: ["theme", "language", "animation", "native-frame", "install-dir", "autostart", "default-timeout", "log-retention", "launch-concurrency"],
  dock: ["dock-state", "dock-mode", "dock-edge", "bezel-y", "dock-width", "dock-density", "per-monitor", "reveal-dwell", "reveal-sensitivity"],
  companion: ["companion-active", "companion-height", "companion-sites"],
  backup: ["backup", "updates"],
  ai: ["ai-provider", "ai-endpoint", "ai-model"],
};

export interface SettingsSearchEntry {
  group: SettingsGroupKey;
  /** Knob id, or `group:<key>` for a whole-group match (e.g. mute, which the
   *  dock pane toolbar owns — the filter surfaces the owning group). */
  id: string;
  label: string;
  synonyms: string[];
  /** Current values as searchable text, so `dark` or `18%` land. */
  values: string[];
  description: string;
}

/// The live values the index reads. Plain data so tests can build an index
/// without rendering the page.
export interface SettingsSearchSnapshot {
  themeMode: string;
  themeLabel: string;
  language: string;
  languageLabel: string;
  animation: string;
  animationLabel: string;
  nativeFrame: string;
  nativeFrameLabel: string;
  installDir: string;
  autostart: string;
  timeoutMinutes: number;
  retentionDays: number;
  launchConcurrency: number;
  dockMode: string;
  dockEdge: string;
  dockState: string;
  /** The bezel tab's parking height as a whole percent of its travel (0–100)
   *  — the single-display knob's value, and the searchable value for the
   *  per-display sliders. Plain data so tests can build an index. */
  bezelYRatioPct: number;
  dockWidthPct: number;
  dockDensity: string;
  revealDwellMs: number;
  revealSensitivityPx: number;
  companionActiveName: string | null;
  companionRatioPct: number;
  companionSiteCount: number;
  companionSiteNames: string[];
  companionMuted: boolean;
  updateSummary: string;
  aiProvider: string;
  aiProviderLabel: string;
  aiModel: string;
}

/// Builds the full knob + group index for one snapshot of current values.
/// Labels and descriptions resolve through the dictionary at build time, so
/// rebuilding the index (the page derives it) follows a language switch.
export function buildSettingsSearchIndex(snap: SettingsSearchSnapshot): SettingsSearchEntry[] {
  const installValue = snap.installDir.trim() || t("settings.wingetDefault");
  const companionValue = snap.companionActiveName ?? t("common.off");
  return [
    {
      group: "general",
      id: "group:general",
      label: t("settings.group.general"),
      synonyms: ["settings", "defaults"],
      values: [],
      description: t("search.group.general.desc"),
    },
    {
      group: "general",
      id: "language",
      label: t("settings.language.label"),
      synonyms: ["language", "locale", "chinese", "english", "translation", "简体中文", "中文"],
      values: [snap.language, snap.languageLabel],
      description: t("search.language.desc"),
    },
    {
      group: "general",
      id: "theme",
      label: t("settings.theme.label"),
      synonyms: ["appearance", "look", "mode", "system", "light", "dark"],
      values: [snap.themeMode, snap.themeLabel],
      description: t("search.theme.desc"),
    },
    {
      group: "general",
      id: "animation",
      label: t("settings.animation.label"),
      synonyms: ["motion", "movement", "transitions", "effects", "fade", "pulses", "still"],
      values: [snap.animation, snap.animationLabel],
      description: t("search.animation.desc"),
    },
    {
      group: "general",
      id: "native-frame",
      label: t("settings.native-frame.label"),
      synonyms: ["titlebar", "title bar", "frame", "frameless", "native", "unified header", "window buttons", "minimize", "maximize", "close"],
      values: [snap.nativeFrame, snap.nativeFrameLabel],
      description: t("search.native-frame.desc"),
    },
    {
      group: "general",
      id: "install-dir",
      label: t("settings.install-dir.label"),
      synonyms: ["location", "folder", "path", "directory", "winget default"],
      values: [installValue],
      description: t("search.install-dir.desc"),
    },
    {
      group: "general",
      id: "autostart",
      label: t("search.autostart.label"),
      synonyms: ["autostart", "login", "boot", "tray", "startup", "on", "off"],
      values: [snap.autostart],
      description: t("search.autostart.desc"),
    },
    {
      group: "general",
      id: "default-timeout",
      label: t("settings.default-timeout.label"),
      synonyms: ["minutes", "kill", "long", "min"],
      values: [`${snap.timeoutMinutes} min`, `${snap.timeoutMinutes} minutes`],
      description: t("search.default-timeout.desc"),
    },
    {
      group: "general",
      id: "log-retention",
      label: t("settings.log-retention.label"),
      synonyms: ["logs", "prune", "days", "archive", "history"],
      values: [`${snap.retentionDays} days`],
      description: t("search.log-retention.desc"),
    },
    {
      group: "general",
      id: "launch-concurrency",
      label: t("settings.launch-concurrency.label"),
      synonyms: ["parallel", "queue", "apps", "at once", "gentle", "snappy"],
      values: [`${snap.launchConcurrency} apps`],
      description: t("search.launch-concurrency.desc"),
    },
    {
      group: "dock",
      id: "group:dock",
      label: t("settings.group.dock"),
      synonyms: ["quick launch", "window", "bar", "strip", "palette"],
      values: [],
      description: t("search.group.dock.desc"),
    },
    {
      group: "dock",
      id: "dock-state",
      label: t("settings.dock-state.label"),
      synonyms: ["floating", "docked", "dock", "palette", "bar"],
      values: [snap.dockState],
      description: t("search.dock-state.desc"),
    },
    {
      group: "dock",
      id: "dock-mode",
      label: t("settings.dock-mode.label"),
      synonyms: ["auto-hide", "autohide", "fixed", "pinned", "taskbar", "hide", "reveal", "bezel", "tab"],
      values: [snap.dockMode],
      description: t("search.dock-mode.desc"),
    },
    {
      group: "dock",
      id: "bezel-y",
      label: t("quickwindow.bezelPosition"),
      synonyms: ["bezel", "tab", "position", "height", "vertical", "drag", "park"],
      values: [`${snap.bezelYRatioPct}%`],
      description: t("search.bezel-y.desc"),
    },
    {
      group: "dock",
      id: "dock-edge",
      label: t("search.dock-edge.label"),
      synonyms: ["left", "right", "side", "screen edge"],
      values: [snap.dockEdge],
      description: t("search.dock-edge.desc"),
    },
    {
      group: "dock",
      id: "dock-width",
      label: t("settings.dock-width.label"),
      synonyms: ["wide", "narrow", "size", "percent", "pixels", "px", "slider"],
      values: [`${snap.dockWidthPct}%`],
      description: t("search.dock-width.desc"),
    },
    {
      group: "dock",
      id: "dock-density",
      label: t("settings.dock-density.label"),
      synonyms: ["compact", "default", "large", "text size", "rows", "readable"],
      values: [snap.dockDensity],
      description: t("search.dock-density.desc"),
    },
    {
      group: "dock",
      id: "per-monitor",
      label: t("settings.per-monitor.label"),
      synonyms: ["monitor", "monitors", "display", "displays", "screen", "screens", "multi", "bezel", "tab position"],
      values: [],
      description: t("search.per-monitor.desc"),
    },
    {
      group: "dock",
      id: "reveal-dwell",
      label: t("settings.reveal-dwell.label"),
      synonyms: ["dwell", "hold", "hover", "slide out", "milliseconds", "ms", "sensitivity", "snappy", "graze"],
      values: [`${snap.revealDwellMs} ms`],
      description: t("search.reveal-dwell.desc"),
    },
    {
      group: "dock",
      id: "reveal-sensitivity",
      label: t("settings.reveal-sensitivity.label"),
      synonyms: ["push", "nudge", "pixels", "px", "brushes"],
      values: [`${snap.revealSensitivityPx} px`],
      description: t("search.reveal-sensitivity.desc"),
    },
    {
      group: "companion",
      id: "group:companion",
      label: t("nav.companion"),
      synonyms: ["sound", "audio", "mute", "muted", "unmute", "volume", "music", "browser", "web", "site", "pane"],
      values: [companionValue],
      description: t("search.group.companion.desc"),
    },
    {
      group: "companion",
      id: "companion-active",
      label: t("settings.companion-active.label"),
      synonyms: ["url", "off", "pane", "website", "page"],
      values: [companionValue],
      description: t("search.companion-active.desc"),
    },
    {
      group: "companion",
      id: "companion-height",
      label: t("settings.companion-height.label"),
      synonyms: ["tall", "short", "ratio", "divider", "drag", "splitter", "percent"],
      values: [`${snap.companionRatioPct}%`],
      description: t("search.companion-height.desc"),
    },
    {
      group: "companion",
      id: "companion-sites",
      label: t("settings.companion-sites.label"),
      synonyms: ["manage", "add", "rename", "delete", "names", "urls", "list"],
      values: snap.companionSiteNames,
      description:
        snap.companionSiteCount === 1
          ? tCount("search.companion-sites.descOne", snap.companionSiteCount)
          : tCount("search.companion-sites.descMany", snap.companionSiteCount),
    },
    {
      group: "backup",
      id: "group:backup",
      label: t("settings.group.backup"),
      synonyms: ["export", "restore", "update", "updates"],
      values: [],
      description: t("search.group.backup.desc"),
    },
    {
      group: "backup",
      id: "backup",
      label: t("settings.backup.label"),
      synonyms: ["export", "restore", "json", "collections", "file"],
      values: [],
      description: t("search.backup.desc"),
    },
    {
      group: "backup",
      id: "updates",
      label: t("settings.updates.label"),
      synonyms: ["update", "upgrade", "version", "release", "github", "new build", "install"],
      values: [snap.updateSummary],
      description: t("search.updates.desc"),
    },
    {
      group: "ai",
      id: "group:ai",
      label: t("settings.group.ai"),
      synonyms: ["ai", "model", "draft", "generate", "assistant", "llm", "ollama"],
      values: [],
      description: t("search.group.ai.desc"),
    },
    {
      group: "ai",
      id: "ai-provider",
      label: t("settings.ai-provider.label"),
      synonyms: ["off", "existing local", "managed", "cloud", "service", "setup", "enable"],
      values: [snap.aiProvider, snap.aiProviderLabel],
      description: t("search.ai-provider.desc"),
    },
    {
      group: "ai",
      id: "ai-endpoint",
      label: t("settings.ai-endpoint.label"),
      synonyms: ["endpoint", "url", "address", "localhost", "port", "connection", "connect"],
      values: [],
      description: t("search.ai-endpoint.desc"),
    },
    {
      group: "ai",
      id: "ai-model",
      label: t("settings.ai-model.label"),
      synonyms: ["model name", "exposes", "pick"],
      values: snap.aiModel ? [snap.aiModel] : [],
      description: t("search.ai-model.desc"),
    },
  ];
}

/// Multi-keyword match: every whitespace-separated token must appear somewhere
/// in the entry's label, synonyms, values, or description. Returns matched ids.
export function matchSettingsSearch(index: SettingsSearchEntry[], query: string): Set<string> {
  const tokens = query.toLowerCase().split(/\s+/).filter(Boolean);
  const matched = new Set<string>();
  if (tokens.length === 0) return matched;
  for (const entry of index) {
    const haystack = [entry.label, entry.description, ...entry.synonyms, ...entry.values]
      .join("\n")
      .toLowerCase();
    if (tokens.every((token) => haystack.includes(token))) matched.add(entry.id);
  }
  return matched;
}

export interface SettingsFilterResolution {
  visibleKnobIds: Set<string>;
  wholeGroups: Set<SettingsGroupKey>;
}

/// Resolves a query to visible knobs. Knob matches win: when any knob
/// matched, only those knobs show. A group match surfaces its whole group
/// only when no knob matched — for bare group-name queries and for concepts
/// no knob owns (mute lives in the dock pane toolbar, so it lands on the
/// whole Companion group instead of a knob that does not exist).
export function resolveSettingsFilter(
  index: SettingsSearchEntry[],
  query: string,
): SettingsFilterResolution {
  const matched = matchSettingsSearch(index, query);
  const visibleKnobIds = new Set(
    index
      .filter((entry) => !entry.id.startsWith("group:") && matched.has(entry.id))
      .map((entry) => entry.id),
  );
  const wholeGroups = new Set<SettingsGroupKey>();
  if (visibleKnobIds.size === 0) {
    for (const entry of index) {
      if (entry.id.startsWith("group:") && matched.has(entry.id)) {
        wholeGroups.add(entry.group);
        for (const id of SETTINGS_GROUP_KNOBS[entry.group]) visibleKnobIds.add(id);
      }
    }
  }
  return { visibleKnobIds, wholeGroups };
}
