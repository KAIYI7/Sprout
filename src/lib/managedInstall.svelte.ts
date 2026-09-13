/// The one home for managed install progress (tickets 189/193, shared
/// contract via 197).
///
/// The Settings managed rows and the review dialog render the same install,
/// so its state lives here instead of in per-surface copies that reset on
/// navigation: one event handler, one single-flight mirror, one Cancel path.
/// Byte progress arrives from the backend worker's events; completion and
/// failure arrive through the install call itself. Reload honesty comes from
/// the backend's in-memory single-flight flag: installing here but idle
/// there means the attempt died with the last page, and the store says so
/// plainly instead of spinning forever (research 0004 rule 5 — silence
/// reads as breakage).

import { listen } from "@tauri-apps/api/event";
import {
  aiCancelManagedInstall,
  aiInstallManaged,
  aiRemoveManaged,
} from "$lib/api";
import type { ManagedInstallProgress } from "$lib/types";

/** Keep in step with `MANAGED_INSTALL_PROGRESS_EVENT` in
 *  `src-tauri/src/ai_managed.rs`. */
export const MANAGED_INSTALL_PROGRESS_EVENT = "managed-install-progress";

export type ManagedInstallPhase = "runtime" | "model";
/** The backend's current step: `downloading` rides the byte events, then one
 *  silent step at a time (`verifying-runtime`, `verifying-model`,
 *  `extracting`, `activating`) so a long hash reads as named work, never a
 *  stuck bar. Unknown values fall back to `downloading`. */
export type ManagedInstallStage =
  | "downloading"
  | "verifying-runtime"
  | "verifying-model"
  | "extracting"
  | "activating";
export type ManagedInstallStatus =
  | "idle"
  | "installing"
  | "interrupted"
  | "done"
  | "error";

export const managedInstall = $state({
  status: "idle" as ManagedInstallStatus,
  modelId: null as string | null,
  phase: "runtime" as ManagedInstallPhase,
  stage: "downloading" as ManagedInstallStage,
  downloadedBytes: 0,
  totalBytes: 0,
  error: null as string | null,
  notice: null as string | null,
  /** `Date.now()` of the last progress event or state change — the
   *  reconcile grace clock, so a just-finished install never flashes
   *  "interrupted" while its resolution is in flight. */
  updatedAt: 0,
});

/** Narrows an event stage to the known steps; anything else (including
 *  events from an older backend without the field) reads as downloading. */
export function asManagedInstallStage(value: unknown): ManagedInstallStage {
  return value === "verifying-runtime" ||
    value === "verifying-model" ||
    value === "extracting" ||
    value === "activating"
    ? value
    : "downloading";
}

let listening = false;

/** Installs the store's progress listener once per webview (idempotent like
 *  `syncQuickActionRuns` — a hot reload may re-run callers). Events for an
 *  install this page did not start are ignored: one webview, one flight. */
export function watchManagedInstall(): void {
  if (listening) return;
  listening = true;
  listen<ManagedInstallProgress>(MANAGED_INSTALL_PROGRESS_EVENT, (event) => {
    if (
      managedInstall.status !== "installing" ||
      managedInstall.modelId === null ||
      event.payload.model_id !== managedInstall.modelId
    ) {
      return;
    }
    managedInstall.phase = event.payload.phase;
    managedInstall.stage = asManagedInstallStage(event.payload.stage);
    managedInstall.downloadedBytes = event.payload.downloaded_bytes;
    managedInstall.totalBytes = event.payload.total_bytes;
    managedInstall.updatedAt = Date.now();
  }).catch(() => {});
}

/** Whole percent for the current phase, or null before the first event. */
export function managedInstallPercent(): number | null {
  if (managedInstall.totalBytes <= 0) return null;
  return Math.min(
    100,
    Math.round(
      (managedInstall.downloadedBytes / managedInstall.totalBytes) * 100,
    ),
  );
}

/** The occupying-revision guard (ticket 193): a previous attempt left its
 *  revision directory behind with no readable manifest, so only explicit
 *  remove-and-retry recovers. Matched by backend copy — kept as the one
 *  predicate beside the Rust message. */
export function isOccupyingRevisionError(message: string): boolean {
  return message.includes("already occupies this model revision");
}

/** Starts one managed install, mirroring the backend single-flight flag in
 *  this store so every surface reads the same flight. A second start while
 *  one runs is a no-op — the backend would refuse it, and the store never
 *  invents a second flight. */
export async function startManagedInstall(modelId: string): Promise<void> {
  if (managedInstall.status === "installing") return;
  managedInstall.status = "installing";
  managedInstall.modelId = modelId;
  managedInstall.phase = "runtime";
  managedInstall.stage = "downloading";
  managedInstall.downloadedBytes = 0;
  managedInstall.totalBytes = 0;
  managedInstall.error = null;
  managedInstall.notice = null;
  managedInstall.updatedAt = Date.now();
  try {
    const result = await aiInstallManaged(modelId);
    managedInstall.status = "done";
    managedInstall.notice = result.message;
    managedInstall.updatedAt = Date.now();
  } catch (cause) {
    managedInstall.status = "error";
    managedInstall.error = String(cause);
    managedInstall.updatedAt = Date.now();
  }
}

/** Cancels the in-flight install from any surface (ticket 193): row and
 *  dialog call this same function, so Cancel exists everywhere the bar
 *  does. The failure surfaces through the install call itself; staged
 *  files are never activated (backend guarantee). */
export function cancelManagedInstall(): void {
  void aiCancelManagedInstall().catch(() => {});
  if (managedInstall.status === "installing") {
    managedInstall.notice =
      "Cancelling installation; staged files will not be activated.";
    managedInstall.updatedAt = Date.now();
  }
}

/** The guard-hit recovery (ticket 193): explicitly removes the occupying
 *  app-owned revision, then retries the same install. Never auto-deletes —
 *  this runs only from the "Remove that revision and retry" action. */
export async function removeAndRetryManagedInstall(
  modelId: string,
): Promise<void> {
  await aiRemoveManaged(modelId);
  await startManagedInstall(modelId);
}

/** Reconciles the store against the backend's in-memory flag after a
 *  (re)mount (ticket 193): installing here but idle there means the attempt
 *  died with the last page — report `Interrupted — safe to retry` instead
 *  of a stuck bar. The grace window covers a resolution still in flight. */
export function reconcileManagedInstall(serverActive: boolean): void {
  if (managedInstall.status !== "installing") return;
  if (serverActive) return;
  if (Date.now() - managedInstall.updatedAt < 3000) return;
  managedInstall.status = "interrupted";
  managedInstall.error = "Interrupted — safe to retry.";
  managedInstall.updatedAt = Date.now();
}

/** Clears a finished, failed, or interrupted install back to idle — the
 *  explicit dismiss behind Keep-files and Retry affordances. */
export function dismissManagedInstall(): void {
  managedInstall.status = "idle";
  managedInstall.modelId = null;
  managedInstall.stage = "downloading";
  managedInstall.downloadedBytes = 0;
  managedInstall.totalBytes = 0;
  managedInstall.error = null;
  managedInstall.notice = null;
  managedInstall.updatedAt = 0;
}

/** Short tier label for the Running/Stopped status line (ticket 189).
 *  Ticket 191 settles the tier-first row copy; the status line reuses this
 *  one helper so the two cannot disagree. */
export function managedTierLabel(modelId: string): string {
  if (modelId === "lightweight-candidate") return "Lightweight";
  if (modelId === "stronger-candidate") return "Stronger";
  return modelId;
}

/** Compact parameter shorthand per managed tier (ticket 191): the frontend
 *  id→tier map, so no catalog schema change is needed. Unknown ids read back
 *  as themselves — an installed model missing from the recommendations stays
 *  identifiable instead of silently mapping (ticket 152). */
export function managedModelParams(modelId: string): string {
  if (modelId === "lightweight-candidate") return "1.5B Q4";
  if (modelId === "stronger-candidate") return "7B Q4";
  return modelId;
}

/** Compact RAM shorthand for the Active-model row (ticket 191):
 *  `4096` reads as `4 GB RAM`, anything under a gig stays in MB. */
export function formatManagedRam(memoryNeedsMb: number | null): string {
  if (memoryNeedsMb === null || !Number.isFinite(memoryNeedsMb) || memoryNeedsMb < 0) {
    return "RAM pending";
  }
  if (memoryNeedsMb >= 1024) {
    const gib = memoryNeedsMb / 1024;
    const label = Number.isInteger(gib) ? String(gib) : String(Math.round(gib * 10) / 10);
    return `${label} GB RAM`;
  }
  return `${memoryNeedsMb} MB RAM`;
}

/** Tier-first Active-model row (ticket 191):
 *  `{Tier} — {params} · {size} · {RAM}`. The full artifact/source/revision/
 *  hash/license identity stays one level down in the Details dialog — this
 *  row chooses by cost, the dialog proves identity. */
export function managedActiveRowLabel(model: {
  id: string;
  download_size_bytes: number | null;
  memory_needs_mb: number | null;
}): string {
  const size =
    model.download_size_bytes === null ? "size pending" : formatManagedBytes(model.download_size_bytes);
  return `${managedTierLabel(model.id)} — ${managedModelParams(model.id)} · ${size} · ${formatManagedRam(model.memory_needs_mb)}`;
}

/** Adaptive byte count for progress lines (`38% · 412 MB of 1.04 GB`):
 *  the phase totals span megabytes (runtime) to gigabytes (model), so one
 *  fixed unit would lie by precision in one phase or noise in the other. */
export function formatManagedBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "0 B";
  const gib = bytes / 1024 / 1024 / 1024;
  if (gib >= 1) {
    return `${new Intl.NumberFormat(undefined, { maximumFractionDigits: 2 }).format(gib)} GB`;
  }
  const mib = bytes / 1024 / 1024;
  if (mib >= 1) {
    return `${new Intl.NumberFormat(undefined, { maximumFractionDigits: 0 }).format(mib)} MB`;
  }
  const kib = bytes / 1024;
  if (kib >= 1) {
    return `${new Intl.NumberFormat(undefined, { maximumFractionDigits: 0 }).format(kib)} KB`;
  }
  return `${bytes} B`;
}

/** `up 40s` / `up 3m` / `up 2h 5m` for the Running status line. */
export function formatManagedUptime(uptimeSecs: number | null): string {
  if (uptimeSecs === null || !Number.isFinite(uptimeSecs) || uptimeSecs < 0) {
    return "up just now";
  }
  const total = Math.floor(uptimeSecs);
  if (total < 60) return `up ${total}s`;
  const minutes = Math.floor(total / 60);
  if (minutes < 60) return `up ${minutes}m`;
  const hours = Math.floor(minutes / 60);
  return `up ${hours}h ${minutes % 60}m`;
}
