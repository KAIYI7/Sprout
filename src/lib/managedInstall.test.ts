import { beforeEach, describe, expect, it, vi } from "vitest";

const handlers: Record<string, (event: { payload: unknown }) => void> = {};

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((event: string, handler: (event: { payload: unknown }) => void) => {
    handlers[event] = handler;
    return Promise.resolve(() => {});
  }),
}));

vi.mock("./api", () => ({
  aiCancelManagedInstall: vi.fn().mockResolvedValue(false),
  aiInstallManaged: vi.fn(),
  aiRemoveManaged: vi.fn(),
}));

import { listen } from "@tauri-apps/api/event";
import { aiCancelManagedInstall, aiInstallManaged, aiRemoveManaged } from "./api";
import {
  cancelManagedInstall,
  dismissManagedInstall,
  formatManagedBytes,
  formatManagedRam,
  formatManagedUptime,
  isOccupyingRevisionError,
  MANAGED_INSTALL_PROGRESS_EVENT,
  managedActiveRowLabel,
  managedInstall,
  managedInstallPercent,
  managedModelParams,
  managedTierLabel,
  reconcileManagedInstall,
  removeAndRetryManagedInstall,
  startManagedInstall,
  watchManagedInstall,
} from "./managedInstall.svelte";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (cause: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

beforeEach(() => {
  vi.clearAllMocks();
  dismissManagedInstall();
});

describe("managed install store (tickets 189/193)", () => {
  it("starts one install and reports done with the backend message", async () => {
    vi.mocked(aiInstallManaged).mockResolvedValue({ model_id: "lite", installed: true, message: "Installed." });
    await startManagedInstall("lite");
    expect(aiInstallManaged).toHaveBeenCalledExactlyOnceWith("lite");
    expect(managedInstall.status).toBe("done");
    expect(managedInstall.notice).toBe("Installed.");
  });

  it("keeps a single flight across callers instead of a second install", async () => {
    const gate = deferred<{ model_id: string; installed: boolean; message: string }>();
    vi.mocked(aiInstallManaged).mockReturnValue(gate.promise);
    const first = startManagedInstall("lite");
    await Promise.resolve();
    expect(managedInstall.status).toBe("installing");
    await startManagedInstall("lite");
    expect(aiInstallManaged).toHaveBeenCalledTimes(1);
    gate.resolve({ model_id: "lite", installed: true, message: "Installed." });
    await first;
    expect(managedInstall.status).toBe("done");
  });

  it("records failure without exposing a partial install as done", async () => {
    vi.mocked(aiInstallManaged).mockRejectedValue(new Error("nope"));
    await startManagedInstall("lite");
    expect(managedInstall.status).toBe("error");
    expect(managedInstall.error).toContain("nope");
  });

  it("cancels from any surface through the one cancel path", async () => {
    const gate = deferred<{ model_id: string; installed: boolean; message: string }>();
    vi.mocked(aiInstallManaged).mockReturnValue(gate.promise);
    const flight = startManagedInstall("lite");
    cancelManagedInstall();
    expect(aiCancelManagedInstall).toHaveBeenCalledTimes(1);
    expect(managedInstall.notice).toContain("Cancelling");
    gate.resolve({ model_id: "lite", installed: true, message: "Installed." });
    await flight;
  });

  it("recovers the occupying-revision guard only through explicit remove-and-retry", async () => {
    vi.mocked(aiRemoveManaged).mockResolvedValue({ model_id: "lite", removed: true, remaining_installed: 0, message: "Removed." });
    vi.mocked(aiInstallManaged).mockResolvedValue({ model_id: "lite", installed: true, message: "Installed." });
    await removeAndRetryManagedInstall("lite");
    expect(aiRemoveManaged).toHaveBeenCalledExactlyOnceWith("lite");
    expect(aiInstallManaged).toHaveBeenCalledExactlyOnceWith("lite");
    expect(managedInstall.status).toBe("done");
  });

  it("reconciles honestly: installing here but idle there means interrupted", async () => {
    const gate = deferred<{ model_id: string; installed: boolean; message: string }>();
    vi.mocked(aiInstallManaged).mockReturnValue(gate.promise);
    const flight = startManagedInstall("lite");
    // A resolution still in flight is within grace — never flashes interrupted.
    reconcileManagedInstall(true);
    expect(managedInstall.status).toBe("installing");
    reconcileManagedInstall(false);
    expect(managedInstall.status).toBe("installing");
    // Past the grace window with an idle backend, the attempt died with the page.
    managedInstall.updatedAt = Date.now() - 10_000;
    reconcileManagedInstall(false);
    expect(managedInstall.status).toBe("interrupted");
    expect(managedInstall.error).toContain("Interrupted");
    gate.resolve({ model_id: "lite", installed: true, message: "Installed." });
    await flight;
    void flight;
  });

  it("feeds phase, bytes, and percent from backend progress events", async () => {
    watchManagedInstall();
    expect(listen).toHaveBeenCalledWith(MANAGED_INSTALL_PROGRESS_EVENT, expect.any(Function));
    const gate = deferred<{ model_id: string; installed: boolean; message: string }>();
    vi.mocked(aiInstallManaged).mockReturnValue(gate.promise);
    const flight = startManagedInstall("lite");
    handlers[MANAGED_INSTALL_PROGRESS_EVENT]({
      payload: { model_id: "lite", phase: "model", downloaded_bytes: 50, total_bytes: 200 },
    });
    expect(managedInstall.phase).toBe("model");
    expect(managedInstallPercent()).toBe(25);
    // Foreign flights never steer this store.
    handlers[MANAGED_INSTALL_PROGRESS_EVENT]({
      payload: { model_id: "heavy", phase: "model", downloaded_bytes: 200, total_bytes: 200 },
    });
    expect(managedInstall.downloadedBytes).toBe(50);
    gate.resolve({ model_id: "lite", installed: true, message: "Installed." });
    await flight;
    // A landed install ignores late events instead of resurrecting the bar.
    handlers[MANAGED_INSTALL_PROGRESS_EVENT]({
      payload: { model_id: "lite", phase: "model", downloaded_bytes: 200, total_bytes: 200 },
    });
    expect(managedInstall.status).toBe("done");
  });

  it("matches the guard predicate both backends copies share", () => {
    expect(isOccupyingRevisionError("An incomplete installation already occupies this model revision. Nothing was replaced.")).toBe(true);
    expect(isOccupyingRevisionError("An incompatible installation already occupies this model revision. Nothing was replaced.")).toBe(true);
    expect(isOccupyingRevisionError("A managed installation is already in progress.")).toBe(false);
    expect(managedInstallPercent()).toBeNull();
  });

  it("formats tiers, bytes, and uptime for the status lines", () => {
    expect(managedTierLabel("lightweight-candidate")).toBe("Lightweight");
    expect(managedTierLabel("stronger-candidate")).toBe("Stronger");
    expect(managedTierLabel("other")).toBe("other");
    expect(formatManagedBytes(2 * 1024 * 1024 * 1024)).toContain("GB");
    expect(formatManagedBytes(412 * 1024 * 1024)).toContain("MB");
    expect(formatManagedUptime(40)).toBe("up 40s");
    expect(formatManagedUptime(180)).toBe("up 3m");
    expect(formatManagedUptime(7500)).toBe("up 2h 5m");
  });

  it("labels the Active radio tier-first without touching the catalog schema (ticket 191)", () => {
    expect(managedModelParams("lightweight-candidate")).toBe("1.5B Q4");
    expect(managedModelParams("stronger-candidate")).toBe("7B Q4");
    // Unknown ids read back as themselves — never silently remapped.
    expect(managedModelParams("other")).toBe("other");
    expect(formatManagedRam(4096)).toBe("4 GB RAM");
    expect(formatManagedRam(512)).toBe("512 MB RAM");
    const row = managedActiveRowLabel({
      id: "lightweight-candidate",
      download_size_bytes: 1117320768,
      memory_needs_mb: 4096,
    });
    expect(row).toContain("Lightweight — 1.5B Q4");
    expect(row).toContain("GB");
    expect(row).toContain("4 GB RAM");
    // Missing evidence reads honestly instead of inventing numbers.
    const pending = managedActiveRowLabel({ id: "other", download_size_bytes: null, memory_needs_mb: null });
    expect(pending).toContain("other");
    expect(pending).toContain("size pending");
    expect(pending).toContain("RAM pending");
  });
});
