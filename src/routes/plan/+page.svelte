<script lang="ts">
  import type {
    Composition,
    PlannedAction,
    PlanEntry,
    PresetRecord,
    Requirement,
    RequirementOutcome,
    RunProgress,
    RunRecord,
    RunStatus,
  } from "$lib/types";
  import { actionLabelText, policyLabelText, runStatusLabelText } from "$lib/types";
  import { t, tCount } from "$lib/copy";
  import { cancelRun, computePlan, createPreset, getActiveRun, getRun, getSettings, listPresets, quickInstallPlan, readRunProgress, startRun } from "$lib/api";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { onDestroy } from "svelte";
  import Button from "$lib/components/Button.svelte";
  import ConfirmDialog from "$lib/components/ConfirmDialog.svelte";
  import EmptyState from "$lib/components/EmptyState.svelte";
  import PresetFormDialog from "$lib/components/PresetFormDialog.svelte";
  import Badge from "$lib/components/Badge.svelte";
  import Notice from "$lib/components/Notice.svelte";
  import PageHeader from "$lib/components/PageHeader.svelte";

  /** One live row of the run's check-then-act story (ticket 20): seeded from
   * the composed selection as "Checking…", then resolved by the worker's
   * progress events — acting for installs/upgrades, done with the outcome
   * for everything else. */
  type LiveRow = {
    product_id: string;
    product_name: string;
    state: "checking" | "acting" | "done";
    action?: string;
    index?: number;
    total?: number;
    status?: RunStatus;
    detail?: string;
    reboot_required?: boolean;
    log_path?: string;
  };

  let presets = $state<PresetRecord[]>([]);
  let selected = $state<string[]>([]);
  /** Quick install (ticket 21): this visit plans one Library Product as a
   * single synthetic requirement — set from ?quick=<product id> on arrival,
   * never from a preset selection. */
  let quickProductId = $state<string | null>(null);
  let loading = $state(true);
  let loadFailed = $state(false);
  let planning = $state(false);
  let composition: Composition | null = $state(null);
  let checkedAt = $state<number | null>(null);
  /** The machine-local default install directory (ticket 34, ADR-0009),
   * read from Settings at bootstrap. It is a machine fact, never part of
   * the Plan: shown next to the plan, honored by the run. */
  let installDir = $state("");
  let error = $state("");
  let notice = $state("");
  let saveOpen = $state(false);
  let saveError = $state("");
  let running = $state(false);
  let runResult: RunRecord | null = $state(null);
  let runId = $state<string | null>(null);
  let progress = $state<RunProgress[]>([]);
  let progressOffset = $state(0);
  let cancelRequested = $state(false);
  let cancelOpen = $state(false);
  let liveRows = $state<Record<string, LiveRow>>({});
  let liveSeeded = $state(false);
  let pollHandle: ReturnType<typeof setInterval> | null = null;
  let included = $state<Record<string, boolean>>({});
  let choice = $state<Record<string, number | "exclude" | undefined>>({});
  let booted = $state(false);
  let validateTimer: ReturnType<typeof setTimeout> | null = null;
  let validateSeq = 0;

  $effect(() => {
    if (!booted) bootstrap();
  });

  // A run started before this page (re)mounted is still this page's run:
  // adopt it from the run-active query (ticket 18), so returning to Plan
  // mid-run keeps the live section and the cancel button — and a finished
  // run keeps its summary. The layout banner is the always-on surface; this
  // is the page's own state catching up with it.
  $effect(() => {
    adoptActiveRun();
  });

  // The live list is seeded once per run from the composed selection: every
  // requirement starts as "Checking…", then progress events resolve it. The
  // flag keeps a mid-run "Check again" from resetting rows that are already
  // past checking.
  $effect(() => {
    if (running && composition && !liveSeeded) {
      liveSeeded = true;
      seedLiveRows();
      applyEvents(progress);
    }
  });

  // Selection is ephemeral per visit: a plain /plan arrival starts empty, and
  // a ?presets=… link (open-in-plan) prefills by name, then validates once.
  // A ?quick=… link (install now) plans one Product as a single synthetic
  // requirement, validated the same way. Nothing re-validates from
  // navigation — the URL is written, never read, after this first pass.
  async function bootstrap() {
    try {
      presets = await listPresets();
      loadFailed = false;
      try {
        installDir = (await getSettings()).install_dir;
      } catch {
        // Best-effort: the install directory only shapes copy — an unreadable
        // database must not block planning.
      }
      const quickId = page.url.searchParams.get("quick");
      if (quickId) {
        quickProductId = quickId;
        await runValidate();
        return;
      }
      const names = (page.url.searchParams.get("presets") ?? "")
        .split(",")
        .map((n) => decodeURIComponent(n.trim()))
        .filter(Boolean);
      if (names.length > 0) {
        const found = presets.filter((p) => names.includes(p.name));
        const missing = names.filter((n) => !presets.some((p) => p.name === n));
        if (missing.length > 0) {
          notice =
            missing.length === 1
              ? t("plan.linkMissingOne")
                  .replace("{name}", missing[0])
                  .replace("{rest}", found.length ? t("plan.linkRestSome") : t("plan.linkRestNone"))
              : t("plan.linkMissingMany")
                  .replace("{count}", String(missing.length))
                  .replace("{rest}", found.length ? t("plan.linkRestSome") : t("plan.linkRestNone"));
        }
        selected = found.map((p) => p.id);
        if (found.length > 0) {
          await runValidate();
        } else {
          composition = null;
        }
      }
    } catch (e) {
      loadFailed = true;
      error = String(e);
    } finally {
      loading = false;
      booted = true;
    }
  }

  function togglePreset(id: string) {
    selected = selected.includes(id)
      ? selected.filter((s) => s !== id)
      : [...selected, id];
    if (selected.length === 0) {
      composition = null;
      checkedAt = null;
    }
    syncUrl();
    if (selected.length > 0) validate();
  }

  function validate() {
    if (validateTimer) clearTimeout(validateTimer);
    validateTimer = setTimeout(runValidate, 250);
  }

  async function runValidate() {
    if (validateTimer) {
      clearTimeout(validateTimer);
      validateTimer = null;
    }
    const planFor = quickProductId
      ? () => quickInstallPlan(quickProductId!)
      : selected.length > 0
        ? () => computePlan(selected)
        : null;
    if (!planFor) return;
    const seq = ++validateSeq;
    planning = true;
    error = "";
    try {
      const plan = await planFor();
      if (seq !== validateSeq) return;
      applyComposition(plan);
    } catch (e) {
      if (seq !== validateSeq) return;
      composition = null;
      checkedAt = null;
      error = String(e);
      if (quickProductId) {
        // A failed quick install is not a plan of nothing: the product is
        // gone or uninstallable — drop out of quick mode and let the page
        // fall back to the normal preset pick.
        quickProductId = null;
      }
      syncUrl();
    } finally {
      if (seq === validateSeq) planning = false;
    }
  }

  /** Adopts a fresh composition: the plan, the check time, every entry
   * included, conflicts undecided, and the URL mirroring it. */
  function applyComposition(plan: Composition) {
    composition = plan;
    checkedAt = Date.now();
    const fresh: Record<string, boolean> = {};
    for (const entry of plan.entries) {
      fresh[entry.product_id] = true;
    }
    included = fresh;
    choice = {};
    syncUrl();
  }

  function flash(message: string) {
    notice = message;
    setTimeout(() => (notice = ""), 3600);
  }

  function currentStage(): "pick" | "plan" | "run" {
    return running || runResult ? "run" : composition ? "plan" : "pick";
  }

  const stage = $derived(currentStage());

  /** The product's display name for quick-install copy — the URL carries only
   * the id; the name comes from the composed plan. */
  const quickProductName = $derived.by(() => {
    if (!quickProductId || !composition) return null;
    return (
      composition.entries.find((e) => e.candidates.length > 0)?.product_name ??
      null
    );
  });

  // The URL mirrors the selection and the stage (?stage=…&presets=…, or
  // ?stage=…&quick=<id> for an install-now visit), so the page is
  // deep-linkable and open-in-plan works; replace keeps toggling out of
  // history. Names travel encoded — a comma inside a name survives.
  function syncUrl() {
    const params = new URLSearchParams();
    if (quickProductId) {
      params.set("quick", quickProductId);
    } else if (selected.length > 0) {
      const names = selected
        .map((id) => presets.find((p) => p.id === id)?.name)
        .filter((n): n is string => !!n);
      if (names.length > 0) {
        params.set("presets", names.map(encodeURIComponent).join(","));
      }
    }
    const current = currentStage();
    if (current !== "pick") params.set("stage", current);
    const qs = params.toString();
    const target = qs ? `/plan?${qs}` : "/plan";
    if (target !== page.url.pathname + page.url.search) {
      goto(target, { replaceState: true, noScroll: true });
    }
  }

  function formatCheckedAt(t: number): string {
    return new Date(t).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  }

  const count = $derived.by(() => {
    if (!composition) return null;
    const by: Record<string, number> = {};
    let skipped = 0;
    let removed = 0;
    let unmanaged = 0;
    for (const entry of composition.entries) {
      if (entry.unresolved && entry.candidates.length === 0) {
        removed += 1;
        continue;
      }
      const entryChoice = choice[entry.product_id];
      if (entry.conflict) {
        if (entryChoice === undefined) continue;
        if (entryChoice === "exclude") {
          skipped += 1;
          continue;
        }
      } else if (!included[entry.product_id]) {
        skipped += 1;
        continue;
      }
      const candidate = entry.conflict
        ? entry.candidates[entryChoice as number]
        : entry.candidates[0];
      by[candidate.action.kind] = (by[candidate.action.kind] ?? 0) + 1;
      if (candidate.action.kind === "unmanaged_skip") {
        unmanaged += 1;
      }
    }
    return { by, skipped, removed, unmanaged };
  });

  const includedCount = $derived.by(() => {
    if (!composition) return 0;
    return composition.entries.filter((entry) => {
      if (entry.unresolved && entry.candidates.length === 0) return false;
      if (entry.conflict) {
        const c = choice[entry.product_id];
        return c !== undefined && c !== "exclude";
      }
      return included[entry.product_id];
    }).length;
  });

  const undecidedCount = $derived.by(() => {
    if (!composition) return 0;
    return composition.entries.filter(
      (e) => e.conflict && choice[e.product_id] === undefined
    ).length;
  });

  const canSave = $derived(
    composition !== null && includedCount > 0 && undecidedCount === 0
  );

  const canRun = $derived(
    composition !== null && includedCount > 0 && undecidedCount === 0
  );

  type PlanGroupId = "ready" | "already" | "decide" | "attention";

  const planGroups = $derived.by(() => {
    const groups: Record<PlanGroupId, PlanEntry[]> = {
      ready: [],
      already: [],
      decide: [],
      attention: [],
    };
    if (!composition) return groups;
    for (const entry of composition.entries) {
      if (entry.unresolved && entry.candidates.length === 0) {
        groups.attention.push(entry);
        continue;
      }
      if (entry.conflict) {
        groups.decide.push(entry);
        continue;
      }
      const action = entry.candidates[0]?.action;
      if (!action) {
        groups.attention.push(entry);
      } else if (action.kind === "install" || action.kind === "upgrade") {
        groups.ready.push(entry);
      } else if (action.kind === "already_ok" || action.kind === "satisfied_by_newer") {
        groups.already.push(entry);
      } else {
        groups.attention.push(entry);
      }
    }
    return groups;
  });

  const groupOrder: { id: PlanGroupId; title: string }[] = $derived([
    { id: "ready", title: t("plan.groupReady") },
    { id: "already", title: t("plan.groupAlready") },
    { id: "decide", title: t("plan.groupDecide") },
    { id: "attention", title: t("plan.groupAttention") },
  ]);

  const runGroups = $derived.by(() => {
    if (!runResult) return null;
    const groups: Partial<Record<RunStatus, RequirementOutcome[]>> = {};
    for (const result of runResult.results) {
      (groups[result.status] ??= []).push(result);
    }
    return groups;
  });

  const runFailed = $derived.by(() => {
    if (!runResult) return false;
    return runResult.results.some(
      (r) => r.status === "failed" || r.status === "timed_out"
    );
  });

  const runCancelled = $derived.by(
    () => runResult !== null && runResult.outcome === "cancelled"
  );

  const runHasNotes = $derived.by(() => {
    if (!runResult) return false;
    return runResult.results.some((r) => r.status === "skipped_unmanaged");
  });

  const runNotesCount = $derived.by(() => {
    if (!runResult) return 0;
    return runResult.results.filter((r) => r.status === "skipped_unmanaged")
      .length;
  });

  const statusOrder: RunStatus[] = [
    "installed",
    "upgraded",
    "already_ok",
    "satisfied_by_newer",
    "skipped_unmanaged",
    "failed",
    "timed_out",
  ];

  type Tone = "accent" | "warm" | "muted" | "info" | "faint" | "danger" | "warn";

  function runStatusTone(status: RunStatus): Tone {
    switch (status) {
      case "installed":
        return "accent";
      case "upgraded":
        return "warm";
      case "already_ok":
        return "muted";
      case "satisfied_by_newer":
        return "info";
      case "skipped_unmanaged":
        return "warn";
      case "failed":
      case "timed_out":
        return "danger";
    }
  }

  async function adoptActiveRun() {
    try {
      const active = await getActiveRun();
      if (!active) return;
      if (active.done) {
        runId = active.run_id;
        const record = await getRun(active.run_id);
        if (record) {
          runResult = record;
          syncUrl();
        }
        return;
      }
      runId = active.run_id;
      running = true;
      progress = [];
      progressOffset = 0;
      cancelRequested = false;
      liveSeeded = false;
      syncUrl();
      const pollStarted = Date.now();
      pollHandle = setInterval(
        () => pollProgress(Number.MAX_SAFE_INTEGER, pollStarted),
        600
      );
    } catch {
      // Nothing to adopt — the page starts clean.
    }
  }

  async function runPlan() {
    if (!composition) return;
    const record = composedRecord();
    if (!record) return;
    running = true;
    error = "";
    runResult = null;
    runId = null;
    progress = [];
    progressOffset = 0;
    cancelRequested = false;
    liveSeeded = false;
    try {
      const started = await startRun(composition.preset_names, record.requirements);
      runId = started.run_id;
      syncUrl();
      const pollStarted = Date.now();
      const maxWait = maxRunWaitMs(record.requirements);
      pollHandle = setInterval(() => pollProgress(maxWait, pollStarted), 600);
    } catch (e) {
      running = false;
      error = String(e);
      syncUrl();
    }
  }

  /** How long the worker may legitimately take: the sum of every timebox,
   * plus margin for detection and persistence. */
  function maxRunWaitMs(requirements: Requirement[]): number {
    const minutes = requirements.reduce(
      (sum, req) => sum + (req.timeout_minutes || 0),
      0
    );
    return minutes * 60_000 + 180_000;
  }

  async function pollProgress(maxWait: number, pollStarted: number) {
    if (!runId) return;
    try {
      const chunk = await readRunProgress(runId, progressOffset);
      progress.push(...chunk.events);
      progressOffset = chunk.offset;
      applyEvents(chunk.events);
      if (chunk.done) {
        stopPolling();
        if (chunk.done.error) {
          running = false;
          error = chunk.done.error;
          syncUrl();
          return;
        }
        const record = await getRun(runId);
        running = false;
        runResult = record;
        syncUrl();
        if (!record) {
          error = t("run.noResults");
        }
        } else {
          const active = await getActiveRun();
          if (active?.run_id !== runId) {
            stopPolling();
            running = false;
            error = t("plan.diedSilent");
            syncUrl();
          } else if (Date.now() - pollStarted > maxWait) {
            stopPolling();
            running = false;
            error = t("plan.timeoutPrompt");
            syncUrl();
          }
        }
    } catch (e) {
      stopPolling();
      running = false;
      error = String(e);
      syncUrl();
    }
  }

  function stopPolling() {
    if (pollHandle !== null) {
      clearInterval(pollHandle);
      pollHandle = null;
    }
  }

  // Rows mirror exactly what will run: same filter as composedRecord() — a
  // conflict only counts once decided and included, toggled-off and
  // removed-from-library entries never get a row.
  function seedLiveRows() {
    if (!composition) return;
    const rows: Record<string, LiveRow> = {};
    for (const entry of composition.entries) {
      if (entry.candidates.length === 0) continue;
      if (entry.conflict) {
        const c = choice[entry.product_id];
        if (c === undefined || c === "exclude") continue;
      } else if (!included[entry.product_id]) {
        continue;
      }
      rows[entry.product_id] = {
        product_id: entry.product_id,
        product_name: entry.product_name,
        state: "checking",
      };
    }
    liveRows = rows;
  }

  // Each event advances the matching row: started → acting (or its resolved
  // verdict when the check itself was the answer), finished → done.
  function applyEvents(events: RunProgress[]) {
    for (const ev of events) {
      if (ev.type === "requirement_started") {
        const row = liveRows[ev.product_id];
        if (row) {
          row.state = "acting";
          row.action = ev.action;
          row.index = ev.index;
          row.total = ev.total;
        }
      } else if (ev.type === "requirement_finished") {
        const row = liveRows[ev.product_id];
        if (row) {
          row.state = "done";
          row.status = ev.status;
          row.detail = ev.detail;
          row.reboot_required = ev.reboot_required;
          row.log_path = ev.log_path;
        }
      }
    }
  }

  // Composition order keeps the list stable and matching the plan's grouping;
  // only the rows that are actually in the run appear.
  const liveRowList = $derived.by(() => {
    if (!composition) return [];
    return composition.entries
      .filter((entry) => entry.candidates.length > 0)
      .map((entry) => liveRows[entry.product_id])
      .filter((row): row is LiveRow => !!row);
  });

  /** The check-then-act live vocabulary (ticket 20): a row visibly goes
   * "Checking…" then "Installing…" / "Upgrading…", or straight to its
   * verdict when the check itself was the answer. Action words come from the
   * backend events; display text resolves per call so a language switch
   * lands live. */
  function liveActingLabel(action: string): string {
    switch (action) {
      case "install":
        return t("plan.actingInstall");
      case "upgrade":
        return t("plan.actingUpgrade");
      case "already ok":
        return t("plan.actingOk");
      case "satisfied by newer":
        return t("plan.actingNewer");
      case "skip":
        return t("plan.actingSkip");
      default:
        return t("common.busy.checking");
    }
  }

  function liveStatusLabel(status: RunStatus): string {
    switch (status) {
      case "already_ok":
        return t("plan.actingOk");
      case "satisfied_by_newer":
        return t("plan.actingNewer");
      case "skipped_unmanaged":
        return t("plan.actingSkip");
      default:
        return runStatusLabelText(status);
    }
  }

  /** The worker flags an installer that ignored the requested directory with
   * a phrase in the outcome detail (ticket 34, ADR-0009). */
  function hasMismatchNote(detail: string): boolean {
    return detail.includes("ignored the requested directory");
  }

  function liveActionTone(action: string): Tone {
    switch (action) {
      case "upgrade":
        return "warm";
      case "already ok":
        return "muted";
      case "satisfied by newer":
        return "info";
      case "skip":
        return "warn";
      default:
        return "accent";
    }
  }

  onDestroy(stopPolling);

  async function cancelPlan() {
    if (!runId) return;
    cancelRequested = true;
    try {
      await cancelRun(runId);
    } catch (e) {
      error = String(e);
    }
  }

  function requestCancel() {
    cancelOpen = false;
    cancelPlan();
  }

  function requirementFor(entry: Composition["entries"][number]): Requirement {
    if (entry.conflict) {
      const c = choice[entry.product_id];
      return entry.candidates[c as number].requirement;
    }
    return entry.merged;
  }

  function openSave() {
    if (!composition) return;
    saveError = "";
    saveOpen = true;
  }

  function composedRecord(): PresetRecord | null {
    if (!composition) return null;
    const requirements = composition.entries
      .filter((entry) => {
        if (entry.candidates.length === 0) return false;
        if (entry.conflict) {
          const c = choice[entry.product_id];
          return c !== undefined && c !== "exclude";
        }
        return included[entry.product_id];
      })
      .map(requirementFor);
    return {
      id: "",
      imported: false,
      schema_version: 1,
      platform: "windows",
      name: "",
      description: "",
      author: "",
      version: "1",
      requirements,
    };
  }

  async function saveComposed(record: PresetRecord) {
    try {
      await createPreset(record);
      saveOpen = false;
      flash(t("plan.composedFlash").replace("{name}", record.name));
      await load();
    } catch (e) {
      saveError = String(e);
    }
  }

  async function load() {
    try {
      presets = await listPresets();
      loadFailed = false;
    } catch (e) {
      loadFailed = true;
      error = String(e);
    } finally {
      loading = false;
    }
  }

  function actionBadge(action: PlannedAction): string {
    return actionLabelText(action.kind);
  }

  function actionTone(action: PlannedAction): Tone {
    switch (action.kind) {
      case "install":
        return "accent";
      case "upgrade":
        return "warm";
      case "already_ok":
        return "muted";
      case "satisfied_by_newer":
        return "info";
      case "unmanaged_skip":
        return "warn";
    }
  }
</script>

{#snippet row(entry: PlanEntry)}
  {@const entryChoice = choice[entry.product_id]}
  {@const removedOnly = !!entry.unresolved && entry.candidates.length === 0}
  {@const conflictIn = entry.conflict && entryChoice !== undefined && entryChoice !== "exclude"}
  {@const plainIn = !entry.conflict && included[entry.product_id]}
  {@const inRun = entry.conflict ? conflictIn : plainIn}
  {@const candidate = entry.conflict
    ? (entryChoice !== undefined && entryChoice !== "exclude" ? entry.candidates[entryChoice as number] : entry.candidates[0])
    : entry.candidates[0]}
  <li class="plan__row" class:conflict={entry.conflict} class:removed={removedOnly} class:out={!removedOnly && !inRun}>
    <div class="plan__toggle">
      {#if removedOnly}
        <span class="plan__decision plan__decision--removed">{t("plan.removedBadge")}</span>
      {:else if entry.conflict}
        <span class="plan__decision" class:undecided={entryChoice === undefined}>
          {entryChoice === undefined ? t("plan.needsDecision") : entryChoice === "exclude" ? t("plan.excluded") : t("plan.included")}
        </span>
      {:else}
        <label class="plan__check">
          <input
            type="checkbox"
            checked={included[entry.product_id]}
            onchange={() => (included[entry.product_id] = !included[entry.product_id])}
          />
          <span class="plan__check-label">{t("plan.inRun")}</span>
        </label>
      {/if}
    </div>

    <div class="plan__body">
      {#if removedOnly}
        <div class="plan__headline">
          <span class="plan__name">{entry.product_name}</span>
          <Badge tone="warn">{t("plan.removedFromLibrary")}</Badge>
        </div>
        <p class="plan__detail">
          {t("plan.removedDetail")}
        </p>
        <p class="plan__sources">
          {t("plan.fromSources").replace("{items}", entry.sources.map((s) => `“${s}”`).join(" + "))}
        </p>
      {:else}
        <div class="plan__headline">
          <span class="plan__name">{entry.product_name}</span>
          <span class="tag tag--policy">{policyLabelText(candidate.requirement.version_policy.kind)}</span>
          <Badge tone={actionTone(candidate.action)}>{actionBadge(candidate.action)}</Badge>
        </div>
        <p class="plan__detail">{candidate.detail}</p>
        {#if candidate.requirement.product.install_dir ?? installDir}
          <p class="plan__target">
            {t("plan.installTo").replace("{dir}", candidate.requirement.product.install_dir ?? installDir)}
          </p>
        {/if}
        <p class="plan__sources">
          {t("plan.fromSources").replace("{items}", entry.sources.map((s) => `“${s}”`).join(" + "))}
        </p>

        {#if entry.unresolved}
          <p class="plan__note">
            {t("plan.unresolvedNote")}
          </p>
        {/if}

        {#if entry.conflict}
          <fieldset class="conflict">
            <legend class="conflict__legend">{t("plan.conflictLegend")}</legend>
            {#each entry.candidates as cand, i (i)}
              <label class="conflict__option">
                <input
                  type="radio"
                  name={`conflict-${entry.product_id}`}
                  value={i}
                  checked={entryChoice === i}
                  onchange={() => (choice[entry.product_id] = i)}
                />
                <span class="conflict__body">
                  <span class="conflict__head">
                    <Badge tone={actionTone(cand.action)}>{actionBadge(cand.action)}</Badge>
                    <span class="conflict__preset">{t("plan.fromPreset").replace("{name}", cand.preset)}</span>
                  </span>
                  <span class="conflict__detail">{cand.detail}</span>
                </span>
              </label>
            {/each}
            <label class="conflict__option">
              <input
                type="radio"
                name={`conflict-${entry.product_id}`}
                value="exclude"
                checked={entryChoice === "exclude"}
                onchange={() => (choice[entry.product_id] = "exclude")}
              />
              <span class="conflict__body">
                <span class="conflict__head">
                  <Badge tone="faint">{t("plan.excluded")}</Badge>
                  <span class="conflict__preset">{t("plan.excludeIt")}</span>
                </span>
                <span class="conflict__detail">{t("plan.excludeDetail")}</span>
              </span>
            </label>
          </fieldset>
        {/if}
      {/if}
    </div>
  </li>
{/snippet}

<section class="plan" aria-labelledby="plan-title">
  <PageHeader titleId="plan-title" title={t("nav.plan")}>
    {#snippet subtitle()}
      {#if quickProductId}
        {t("plan.subQuick").replace("{name}", quickProductName ?? t("plan.thisProduct"))}
      {:else}
        {t("plan.subPick")}
      {/if}
    {/snippet}
  </PageHeader>

  <ol class="steps" aria-label={t("plan.stagesLabel")}>
    <li class="steps__item" class:active={stage === "pick"} aria-current={stage === "pick" ? "step" : undefined}>
      {t("plan.stepPick")}
    </li>
    <li class="steps__item" class:active={stage === "plan"} aria-current={stage === "plan" ? "step" : undefined}>
      {t("plan.stepReview")}
    </li>
    <li class="steps__item" class:active={stage === "run"} aria-current={stage === "run" ? "step" : undefined}>
      {t("plan.stepRun")}
    </li>
  </ol>

  {#if notice}
    <Notice tone="ok">{notice}</Notice>
  {/if}
  {#if error}
    <Notice tone="error">{error}</Notice>
  {/if}

  {#if loading}
    <p class="sifting">Loading…</p>
  {:else if loadFailed}
    <EmptyState icon="x" title={t("common.libraryReadFail")}>
      <p>{t("plan.dbBody")} <span class="mono">%LOCALAPPDATA%\Sprout\sprout.db</span>{t("launch.filterNone")}</p>
      <p class="error-detail">{error}</p>
      <div class="empty-cta">
        <Button variant="secondary" onclick={load}>{t("common.retry")}</Button>
      </div>
    </EmptyState>
  {:else if presets.length === 0 && !quickProductId}
    <EmptyState title={t("plan.noPresets")}>
      <p>{t("plan.noPresetsBody")}</p>
      <div class="empty-cta">
        <Button onclick={() => goto("/presets")}>{t("plan.composeFirst")}</Button>
      </div>
    </EmptyState>
  {:else}
    {#if !quickProductId}
      <div class="pick">
        <p class="pick__label">{t("plan.stepPick")}</p>
        <ul class="pick__list">
          {#each presets as record (record.id)}
            {@const checked = selected.includes(record.id)}
            <li class="pick__cell">
              <label class="pick__card" class:checked>
                <input
                  type="checkbox"
                  class="pick__check"
                  checked={checked}
                  onchange={() => togglePreset(record.id)}
                />
                <span class="pick__body">
                  <span class="pick__name-row">
                    <span class="pick__name">{record.name}</span>
                    <span class="pick__version">v{record.version}</span>
                  </span>
                  <span class="pick__desc">{record.description}</span>
                  <span class="pick__meta">
                    {record.requirements.length === 1 ? tCount("packet.reqOne", record.requirements.length) : tCount("packet.reqMany", record.requirements.length)}
                    {#if record.imported}
                      <span class="pick__imported">{t("packet.imported")}</span>
                    {/if}
                  </span>
                </span>
              </label>
            </li>
          {/each}
        </ul>
      </div>
    {/if}

    {#if planning && !composition}
      <p class="sifting">Loading…</p>
    {/if}

    {#if composition}
      {@const summary = count}
      <div class="plan__result">
        <p class="pick__label">{t("plan.stepReview")}</p>

        <div class="staleness" role="status" aria-live="polite">
          <span class="staleness__note">
            {checkedAt
              ? t("plan.checkedAt").replace("{time}", formatCheckedAt(checkedAt))
              : t("plan.checkedNow")}
            {t("plan.stalePrompt")}
          </span>
          <button
            class="staleness__btn"
            onclick={runValidate}
            disabled={planning || (!quickProductId && selected.length === 0)}
          >
            {planning ? t("common.busy.checking") : t("plan.checkAgain")}
          </button>
        </div>

        <div class="plan__summary" role="status" aria-live="polite">
          {#if includedCount > 0}
            <span class="summary-line">
              {(includedCount === 1 ? t("plan.reqInRunOne") : t("plan.reqInRunMany"))
                .replace("{count}", String(includedCount))
                .replace("{actions}", summary ? Object.entries(summary.by).map(([kind, n]) => `${n} ${actionLabelText(kind as PlannedAction["kind"])}`).join(", ") : "")}
            </span>
          {:else}
            <span class="summary-line">{t("plan.nothingSelected")}</span>
          {/if}
          {#if summary?.skipped}
            <span class="summary-line summary-line--muted">{t("plan.toggledOff").replace("{count}", String(summary.skipped))}</span>
          {/if}
          {#if summary?.unmanaged}
            <span class="summary-line summary-line--attention">
              {(summary.unmanaged === 1 ? t("plan.unmanagedOne") : t("plan.unmanagedMany")).replace("{count}", String(summary.unmanaged))}
            </span>
          {/if}
          {#if undecidedCount > 0}
            <span class="summary-line summary-line--warn">
              {(undecidedCount === 1 ? t("plan.conflictsOne") : t("plan.conflictsMany")).replace("{count}", String(undecidedCount))}
            </span>
          {/if}
          {#if summary?.removed}
            <span class="summary-line summary-line--warn">
              {(summary.removed === 1 ? t("plan.removedOne") : t("plan.removedMany")).replace("{count}", String(summary.removed))}
            </span>
          {/if}
          {#if installDir}
            <span class="summary-line">
              {t("plan.installsGoTo").replace("{dir}", installDir)}
            </span>
          {/if}
        </div>

        {#each groupOrder as group (group.id)}
          {@const entries = planGroups[group.id]}
          {#if entries.length > 0}
            <section class="group group--{group.id}" aria-labelledby="group-{group.id}-title">
              <header class="group__head">
                <h2 id="group-{group.id}-title" class="group__title">
                  {group.title}
                  <span class="group__count">
                    ({group.id === "decide" ? undecidedCount : entries.length})
                  </span>
                </h2>
                <p class="group__sub">
                  {#if group.id === "ready"}
                    {installDir
                      ? t("plan.subReadyDir").replace("{dir}", installDir)
                      : t("plan.subReady")}
                  {:else if group.id === "already"}
                    {t("plan.subAlready")}
                  {:else if group.id === "decide"}
                    {undecidedCount > 0
                      ? t("plan.subDecideOpen")
                      : t("plan.subDecideSettled")}
                  {:else}
                    {t("plan.subAttention")}
                  {/if}
                </p>
              </header>
              <ul class="group__list">
                {#each entries as entry (entry.product_id)}
                  {@render row(entry)}
                {/each}
              </ul>
            </section>
          {/if}
        {/each}

        <div class="plan__actions">
          <div class="plan__run-cta">
            {#if runResult}
              <Button variant="secondary" onclick={runPlan} disabled={!canRun || running}>
                {t("run.again")}
              </Button>
            {/if}
            {#if !quickProductId}
              <Button variant="secondary" onclick={openSave} disabled={!canSave}>
                {t("plan.saveAsNew")}
              </Button>
            {/if}
            <Button onclick={runPlan} disabled={!canRun || running}>
              {running ? t("plan.applying") : runResult ? t("plan.runThis") : t("plan.runThisCount").replace("{count}", String(includedCount))}
            </Button>
          </div>
          <p class="plan__actions-hint">
            {canRun
              ? t("plan.hintCanRun")
              : undecidedCount > 0
                ? t("plan.hintConflict")
                : t("plan.hintToggle")}
          </p>
        </div>

        {#if running}
          <section class="live" aria-labelledby="live-title">
            <header class="live__head">
              <div>
                <p class="eyebrow">{t("action.run")}</p>
                <h2 id="live-title" class="live__title">{t("plan.liveTitle")}</h2>
                <p class="live__sub">
                  {#if cancelRequested}
                    {t("plan.liveCancelNote")}
                  {:else}
                    {t("plan.liveHow")}
                  {/if}
                </p>
              </div>
              <div class="live__cta">
                <Button variant="danger" onclick={() => (cancelOpen = true)} disabled={cancelRequested}>
                  {cancelRequested ? t("run.cancelling") : t("run.cancel")}
                </Button>
              </div>
            </header>
            <ul class="live__list">
              {#if liveRowList.length === 0}
                <li class="live__phase" role="status" aria-live="polite">
                  {t("plan.checkingMachine")}
                </li>
              {:else}
                {#each liveRowList as row (row.product_id)}
                  {@const executing =
                    row.state === "acting" &&
                    (row.action === "install" || row.action === "upgrade")}
                  {@const mark = row.status
                    ? runStatusTone(row.status)
                    : executing
                      ? "spin"
                      : row.state === "acting"
                        ? liveActionTone(row.action ?? "")
                        : "faint"}
                  <li class="live__row" class:live__row--active={executing} role="status" aria-live="polite">
                    <span class="live__mark live__mark--{mark}" aria-hidden="true"></span>
                    <span class="live__name">{row.product_name}</span>
                    {#if executing}
                      <span class="live__meta">
                        {t("plan.stepOf").replace("{index}", String(row.index !== undefined ? row.index + 1 : "?")).replace("{total}", String(row.total ?? "?"))}
                      </span>
                      <Badge tone={liveActionTone(row.action ?? "")}>
                        {liveActingLabel(row.action ?? "")}
                      </Badge>
                    {:else if row.status}
                      <Badge tone={runStatusTone(row.status)}>{liveStatusLabel(row.status)}</Badge>
                    {:else if row.state === "acting"}
                      <Badge tone={liveActionTone(row.action ?? "")}>
                        {liveActingLabel(row.action ?? "")}
                      </Badge>
                    {:else}
                      <Badge tone="faint">{t("common.busy.checking")}</Badge>
                    {/if}
                    {#if (row.status === "failed" || row.status === "timed_out") && row.detail}
                      <span class="live__detail">{row.detail}</span>
                    {:else if (row.status === "installed" || row.status === "upgraded") && row.detail && hasMismatchNote(row.detail)}
                      <span class="live__detail">{row.detail}</span>
                    {/if}
                    {#if row.reboot_required}
                      <span class="live__note">{t("run.rebootRequired")}</span>
                    {/if}
                  </li>
                {/each}
              {/if}
            </ul>
          </section>
        {/if}

        {#if runResult && runGroups}
          <section class="run" aria-labelledby="run-title">
            <header class="run__head">
              <div>
                <p class="eyebrow">{t("action.run")}</p>
                <h2 id="run-title" class="run__title">
                  {runCancelled
                    ? t("run.cancelledTitle")
                    : runFailed
                      ? t("run.failedTitle")
                      : runHasNotes
                        ? t("run.notesTitle")
                        : t("run.okTitle")}
                </h2>
                <p class="run__sub">
                  {t("run.fromRun").replace("{items}", runResult.preset_names.length === 1 ? `“${runResult.preset_names[0]}”` : runResult.preset_names.map((n) => `“${n}”`).join(" + "))}
                  {#if runCancelled}
                    {t("run.rerunRest")}
                  {:else if runFailed}
                    {t("run.rerunFailed")}
                  {:else if runHasNotes}
                    {(runNotesCount === 1 ? t("run.notesDetailOne") : t("run.notesDetailMany")).replace("{count}", String(runNotesCount))}
                  {/if}
                </p>
              </div>
              <div class="run__head-side">
                <Button
                  variant="secondary"
                  onclick={() => goto(`/history?run=${runResult!.id}`)}
                >
                  {t("plan.seeHistory")}
                </Button>
                <span class="run__id">{runResult.id}</span>
              </div>
            </header>

            <ul class="run__groups">
              {#each statusOrder as status}
                {@const items = runGroups[status] ?? []}
                {#if items.length > 0}
                  <li class="run__group">
                    <p class="run__group-head">
                      <Badge tone={runStatusTone(status)}>{runStatusLabelText(status)}</Badge>
                      <span class="run__group-count">{items.length}</span>
                    </p>
                    <ul class="run__group-list">
                      {#each items as item (item.product_id)}
                        <li class="run__item">
                          <span class="run__item-name">{item.product_name}</span>
                          {#if item.reboot_required}
                            <span class="run__item-note">{t("run.rebootRequired")}</span>
                          {/if}
                          {#if status === "failed" || status === "timed_out"}
                            <span class="run__item-detail">{item.detail}</span>
                            {#if item.log_path}
                              <span class="run__item-log">{item.log_path}</span>
                            {/if}
                          {:else if (status === "installed" || status === "upgraded") && hasMismatchNote(item.detail)}
                            <span class="run__item-detail">{item.detail}</span>
                          {:else if status === "skipped_unmanaged"}
                            <span class="run__item-notes">{item.detail}</span>
                          {/if}
                        </li>
                      {/each}
                    </ul>
                  </li>
                {/if}
              {/each}
            </ul>
          </section>
        {/if}
      </div>
    {/if}
  {/if}
</section>

<PresetFormDialog
  open={saveOpen}
  preset={composedRecord()}
  error={saveError}
  onsave={saveComposed}
  oncancel={() => (saveOpen = false)}
  onerror={(message) => (saveError = message)}
/>

<ConfirmDialog
  open={cancelOpen}
  title={t("dialog.stopRun.title")}
  confirmLabel={t("dialog.stopRun.confirm")}
  danger
  onconfirm={requestCancel}
  oncancel={() => (cancelOpen = false)}
>
  <p>{t("dialog.stopRun.body1")}</p>
  <p>{t("dialog.stopRun.body2")}</p>
</ConfirmDialog>

<style>
  .plan {
    max-width: 920px;
    margin: 0 auto;
  }

  .eyebrow {
    margin: 0 0 var(--space-2);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    font-weight: 500;
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    color: var(--warm-text);
  }

  .steps {
    list-style: none;
    margin: 0 0 var(--space-5);
    padding: 0;
    display: flex;
    gap: var(--space-2);
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .steps__item {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    padding: 3px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--bg-sunken);
  }

  .steps__item.active {
    color: var(--accent);
    border-color: var(--accent);
    background: var(--accent-tint);
  }

  .sifting {
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    letter-spacing: var(--tracking-mono);
    color: var(--text-muted);
  }

  .mono {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
  }

  .error-detail {
    margin-top: var(--space-2) !important;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--danger-text);
    overflow-wrap: anywhere;
  }

  .empty-cta {
    margin-top: var(--space-4);
  }

  .pick {
    margin-bottom: var(--space-6);
  }

  .pick__label,
  .plan__result > .pick__label {
    margin: 0 0 var(--space-3);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    font-weight: 500;
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    color: var(--accent);
  }

  .pick__list {
    list-style: none;
    margin: 0 0 var(--space-3);
    padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: var(--space-3);
  }

  .pick__card {
    display: flex;
    gap: var(--space-3);
    height: 100%;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: var(--space-3);
    background: var(--bg-surface);
    cursor: pointer;
    transition: border-color var(--dur) var(--ease-out),
      box-shadow var(--dur) var(--ease-out);
  }

  .pick__card:hover {
    border-color: var(--border-strong);
  }

  .pick__card.checked {
    border-color: var(--accent);
    box-shadow: var(--ring-glow);
  }

  .pick__check {
    margin-top: 3px;
    accent-color: var(--accent);
  }

  .pick__body {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    min-width: 0;
  }

  .pick__name-row {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .pick__name {
    font-family: var(--font-display);
    font-size: var(--text-base);
    font-weight: 600;
    color: var(--text);
    overflow-wrap: anywhere;
  }

  .pick__version {
    flex-shrink: 0;
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
    padding: 1px 7px;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-lg);
    color: var(--text-muted);
    background: var(--bg-sunken);
  }

  .pick__desc {
    margin: 0;
    font-size: var(--text-xs);
    color: var(--text-muted);
    overflow-wrap: anywhere;
  }

  .pick__meta {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
    color: var(--warm-text);
  }

  .pick__imported {
    margin-left: 6px;
    border: 1px dashed var(--border-strong);
    border-radius: var(--radius-lg);
    padding: 0 6px;
    color: var(--text-muted);
  }

  .plan__result {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .staleness {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  .staleness__btn {
    border: none;
    padding: 0;
    background: none;
    font-family: var(--font-body);
    font-size: var(--text-xs);
    color: var(--accent);
    text-decoration: underline;
    text-underline-offset: 2px;
    cursor: pointer;
  }

  .staleness__btn:hover {
    color: var(--text);
  }

  .staleness__btn:disabled {
    color: var(--text-muted);
    cursor: default;
  }

  .plan__summary {
    font-size: var(--text-sm);
    color: var(--text);
    background: var(--bg-sunken);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: var(--space-3) var(--space-4);
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2) var(--space-4);
  }

  .summary-line {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
  }

  .summary-line--muted {
    color: var(--text-muted);
  }

  .summary-line--warn {
    color: var(--warm-text);
  }

  .summary-line--attention {
    color: var(--status-notes-text);
  }

  .group {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    border-left: 2px solid var(--border);
    padding-left: var(--space-4);
  }

  .group--ready {
    border-left-color: var(--accent);
  }

  .group--already {
    border-left-color: var(--gray);
  }

  .group--decide {
    border-left-color: var(--warm);
  }

  .group--attention {
    border-left-color: var(--status-notes);
  }

  .group__head {
    display: flex;
    align-items: baseline;
    flex-wrap: wrap;
    gap: var(--space-2);
  }

  .group__title {
    margin: 0;
    font-family: var(--font-display);
    font-size: var(--text-base);
    font-weight: 600;
    color: var(--text);
  }

  .group__count {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
    color: var(--text-muted);
  }

  .group__sub {
    margin: 0;
    font-size: var(--text-xs);
    font-style: italic;
    color: var(--text-muted);
  }

  .group__list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .plan__row {
    display: flex;
    gap: var(--space-3);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: var(--space-3) var(--space-4);
    background: var(--bg-surface);
  }

  .plan__row.conflict {
    border-color: var(--warm-tint-border);
    background: var(--warm-tint);
  }

  .plan__row.removed {
    border-color: var(--warm-tint-border);
    background: var(--warm-tint);
  }

  .plan__row.out {
    opacity: 0.55;
  }

  .plan__toggle {
    flex-shrink: 0;
    padding-top: 2px;
  }

  .plan__check {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    color: var(--text-muted);
    cursor: pointer;
  }

  .plan__check input {
    accent-color: var(--accent);
  }

  .plan__decision {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .plan__decision.undecided {
    color: var(--danger-text);
  }

  .plan__decision--removed {
    color: var(--warm-text);
  }

  .plan__body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .plan__headline {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: var(--space-2);
  }

  .plan__name {
    font-family: var(--font-display);
    font-size: var(--text-base);
    font-weight: 600;
    color: var(--text);
    overflow-wrap: anywhere;
  }

  .tag {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: 0.04em;
    padding: 2px 7px;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-lg);
    color: var(--text-muted);
    background: transparent;
  }

  .plan__detail {
    margin: 0;
    font-size: var(--text-xs);
    color: var(--text);
  }

  .plan__target {
    margin: 0;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  .plan__sources {
    margin: 0;
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
    color: var(--text-muted);
  }

  .plan__note {
    margin: 0;
    font-size: var(--text-xs);
    font-style: italic;
    color: var(--warm-text);
  }

  .conflict {
    margin-top: var(--space-2);
    border: none;
    border-top: 1px dashed var(--warm-tint-border);
    padding: var(--space-2) 0 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .conflict__legend {
    font-size: var(--text-xs);
    font-style: italic;
    color: var(--warm-text);
    margin-bottom: var(--space-1);
  }

  .conflict__option {
    display: flex;
    align-items: flex-start;
    gap: var(--space-2);
    cursor: pointer;
    padding: var(--space-1) 0;
  }

  .conflict__option input {
    margin-top: 3px;
    accent-color: var(--accent);
  }

  .conflict__body {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .conflict__head {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .conflict__preset {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
    color: var(--text-muted);
  }

  .conflict__detail {
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  .plan__actions {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: var(--space-2);
    border-top: 1px dashed var(--border-strong);
    padding-top: var(--space-3);
  }

  .plan__actions-hint {
    margin: 0;
    font-size: var(--text-xs);
    color: var(--text-muted);
    font-style: italic;
  }

  .plan__run-cta {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    flex-shrink: 0;
  }

  .live {
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-surface);
    padding: var(--space-4);
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .live__head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--space-4);
  }

  .live__title {
    margin: 0;
    font-family: var(--font-display);
    font-size: var(--text-xl);
    font-weight: 600;
    color: var(--text);
  }

  .live__sub {
    margin: var(--space-1) 0 0;
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  .live__cta {
    flex-shrink: 0;
  }

  .live__list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .live__phase {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    color: var(--text-muted);
    padding: 2px 0;
  }

  .live__row {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    font-size: var(--text-xs);
    color: var(--text);
    padding: 2px 0;
  }

  .live__row--active {
    color: var(--text);
  }

  .live__name {
    font-weight: 600;
    overflow-wrap: anywhere;
  }

  .live__meta {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
    color: var(--text-muted);
  }

  .live__detail {
    color: var(--danger-text);
    overflow-wrap: anywhere;
  }

  .live__note {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
    color: var(--warm-text);
  }

  .live__mark {
    flex-shrink: 0;
    width: 9px;
    height: 9px;
    border-radius: 50%;
  }

  .live__mark--accent {
    background: var(--accent);
  }

  .live__mark--warm {
    background: var(--warm);
  }

  .live__mark--muted {
    background: var(--gray);
  }

  .live__mark--info {
    background: var(--info);
  }

  .live__mark--warn {
    background: var(--status-notes);
  }

  .live__mark--faint {
    background: transparent;
    border: 1px solid var(--border-strong);
  }

  .live__mark--danger {
    background: var(--danger);
  }

  .live__mark--spin {
    background: var(--warm);
    animation: live-pulse 1s ease-in-out infinite;
  }

  @keyframes live-pulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.35;
    }
  }

  .run {
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-surface);
    padding: var(--space-4);
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .run__head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--space-4);
  }

  .run__title {
    margin: 0;
    font-family: var(--font-display);
    font-size: var(--text-xl);
    font-weight: 600;
    color: var(--text);
  }

  .run__sub {
    margin: var(--space-1) 0 0;
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  .run__id {
    flex-shrink: 0;
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
    padding: 1px 7px;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-lg);
    color: var(--text-muted);
    background: var(--bg-sunken);
  }

  .run__head-side {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .run__groups {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .run__group-head {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin: 0 0 var(--space-1);
  }

  .run__group-count {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
    color: var(--text-muted);
  }

  .run__group-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .run__item {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
    font-size: var(--text-xs);
    color: var(--text);
  }

  .run__item-name {
    font-weight: 600;
  }

  .run__item-note {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
    color: var(--warm-text);
  }

  .run__item-detail {
    color: var(--danger-text);
    overflow-wrap: anywhere;
  }

  .run__item-notes {
    color: var(--status-notes-text);
    overflow-wrap: anywhere;
  }

  .run__item-log {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    color: var(--text-muted);
    overflow-wrap: anywhere;
  }
</style>
