<script lang="ts">
  import { onMount } from "svelte";
  import type {
    RequirementOutcome,
    RunRecord,
    RunStatus,
    RunSummary,
  } from "$lib/types";
  import { runOutcomeLabelText, runStatusLabelText } from "$lib/types";
  import { formatDuration, formatTimestampShort } from "$lib/format";
  import { getRun, listRuns } from "$lib/api";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import Button from "$lib/components/Button.svelte";
  import EmptyState from "$lib/components/EmptyState.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import PageHeader from "$lib/components/PageHeader.svelte";
  import Badge from "$lib/components/Badge.svelte";
  import Notice from "$lib/components/Notice.svelte";
  import { t } from "$lib/copy";

  let runs = $state<RunSummary[]>([]);
  let loading = $state(true);
  let loadFailed = $state(false);
  let error = $state("");
  let openId = $state<string | null>(null);
  let detail: RunRecord | null = $state(null);
  let detailLoading = $state(false);

  onMount(() => {
    load();
  });

  async function load() {
    loading = true;
    loadFailed = false;
    error = "";
    try {
      runs = await listRuns();
      // ?run=<id> deep-links from the Plan page's run summary: open that run
      // once the list is here. Unknown ids are ignored — the page is just
      // the plain list.
      const wanted = page.url.searchParams.get("run");
      if (wanted && openId === null && runs.some((r) => r.id === wanted)) {
        await openRun(wanted);
      }
    } catch (e) {
      loadFailed = true;
      error = String(e);
    } finally {
      loading = false;
    }
  }

  /** The four run outcome tiers (ticket 16) — the Notion status colors, not
   * generic badge tones; "with notes" never reads as success. */
  type OutcomeTone =
    | "status-applied"
    | "status-notes"
    | "status-cancelled"
    | "status-failed";

  function outcomeTone(outcome: RunSummary["outcome"]): OutcomeTone {
    switch (outcome) {
      case "ok":
        return "status-applied";
      case "with_notes":
        return "status-notes";
      case "cancelled":
        return "status-cancelled";
      case "failed":
        return "status-failed";
    }
  }

  function outcomeLabel(outcome: RunSummary["outcome"]): string {
    return runOutcomeLabelText(outcome);
  }

  /** "Open in Plan" carries the run's stored preset names to the Plan page,
   * which matches them by name and says plainly when one has been removed
   * or renamed — never a broken page. */
  function openInPlan(run: RunSummary) {
    const names = run.preset_names.map((n) => encodeURIComponent(n)).join(",");
    goto(names ? `/plan?presets=${names}` : "/plan");
  }

  async function openRun(id: string) {
    if (openId === id) {
      openId = null;
      detail = null;
      return;
    }
    openId = id;
    detail = null;
    detailLoading = true;
    error = "";
    try {
      const record = await getRun(id);
      if (record) {
        detail = record;
      } else {
        error = t("run.noResults");
      }
    } catch (e) {
      error = String(e);
    } finally {
      detailLoading = false;
    }
  }

  const statusOrder: RunStatus[] = [
    "installed",
    "upgraded",
    "already_ok",
    "satisfied_by_newer",
    "skipped_unmanaged",
    "failed",
    "timed_out",
  ];

  const detailGroups = $derived.by(() => {
    if (!detail) return null;
    const groups: Partial<Record<RunStatus, RequirementOutcome[]>> = {};
    for (const result of detail.results) {
      (groups[result.status] ??= []).push(result);
    }
    return groups;
  });

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
</script>

<section class="history" aria-labelledby="history-title">
  <PageHeader titleId="history-title" title={t("nav.history")}>
    {#snippet actions()}
      <Button variant="secondary" onclick={load} disabled={loading}>
        <Icon name="refresh" size={13} /> {t("common.refresh")}
      </Button>
    {/snippet}
    {#snippet subtitle()}
      {t("history.subtitle")}
    {/snippet}
  </PageHeader>

  {#if error}
    <Notice tone="error">{error}</Notice>
  {/if}

  {#if loading}
    <p class="sifting" aria-live="polite">Loading…</p>
  {:else if loadFailed}
    <EmptyState icon="x" title={t("history.dbTitle")}>
      <p>{t("history.dbBody")}
        <span class="mono">%LOCALAPPDATA%\Sprout\sprout.db</span>.</p>
      <p>{t("common.dbLockedHint")}</p>
      <p class="error-detail">{error}</p>
      <div class="empty-cta">
        <Button variant="secondary" onclick={load}>{t("common.retry")}</Button>
      </div>
    </EmptyState>
  {:else if runs.length === 0}
    <EmptyState title={t("history.noRuns")}>
      <p>{t("history.noRunsBody")}</p>
      <div class="empty-cta">
        <Button onclick={() => goto("/plan")}>{t("history.planFirst")}</Button>
      </div>
    </EmptyState>
  {:else}
    <ul class="runs">
      {#each runs as run (run.id)}
        {@const open = openId === run.id}
        <li class="runs__row" class:open>
          <button
            type="button"
            class="runs__toggle"
            aria-expanded={open}
            aria-controls={open ? `run-detail-${run.id}` : undefined}
            onclick={() => openRun(run.id)}
          >
            <span class="runs__date">{formatTimestampShort(run.started_at)}</span>
            <span class="runs__presets">
              {run.preset_names.length === 1
                ? run.preset_names[0]
                : run.preset_names.map((n) => `“${n}”`).join(" + ")}
            </span>
            <span class="runs__meta">
              <Badge tone={outcomeTone(run.outcome)}>{outcomeLabel(run.outcome)}</Badge>
              <span class="runs__duration">{formatDuration(run.finished_at - run.started_at)}</span>
              <span class="runs__id">{run.id}</span>
            </span>
          </button>

          <button type="button" class="runs__open-plan" onclick={() => openInPlan(run)}>
            <Icon name="play" size={12} /> {t("history.openInPlan")}
          </button>

          {#if open}
            <div class="runs__detail" id="run-detail-{run.id}" aria-live="polite">
              {#if detailLoading}
                <p class="sifting">{t("history.opening")}</p>
              {:else if detail}
                <header class="detail__head">
                  <p class="detail__sub">
                    {t("history.startedFor").replace("{start}", formatTimestampShort(detail.started_at)).replace("{dur}", formatDuration(detail.finished_at - detail.started_at))}
                  </p>
                  {#if detail.outcome === "with_notes"}
                    <p class="detail__note detail__note--notes">
                      {(detailGroups?.["skipped_unmanaged"]?.length ?? 0) === 1
                        ? t("history.notesDetailOne").replace("{count}", String(detailGroups?.["skipped_unmanaged"]?.length ?? 0))
                        : t("history.notesDetailMany").replace("{count}", String(detailGroups?.["skipped_unmanaged"]?.length ?? 0))}
                    </p>
                  {:else if detail.outcome === "failed"}
                    <p class="detail__note detail__note--failed">
                      {t("history.noteFailed")}
                    </p>
                  {:else if detail.outcome === "cancelled"}
                    <p class="detail__note detail__note--cancelled">
                      {t("history.noteCancelled")}
                    </p>
                  {/if}
                </header>

                {#if detail.results.length === 0}
                  <p class="detail__none">{t("history.nothingRan")}</p>
                {:else}
                  <ul class="detail__groups">
                    {#each statusOrder as status}
                      {@const items = detailGroups?.[status] ?? []}
                      {#if items.length > 0}
                        <li class="detail__group">
                          <p class="detail__group-head">
                            <Badge tone={runStatusTone(status)}>{runStatusLabelText(status)}</Badge>
                            <span class="detail__group-count">{items.length}</span>
                          </p>
                          <ul class="detail__group-list">
                            {#each items as item (item.product_id)}
                              <li class="detail__item">
                                <span class="detail__item-name">{item.product_name}</span>
                                {#if item.reboot_required}
                                  <span class="detail__item-note">{t("run.rebootRequired")}</span>
                                {/if}
                                {#if status === "failed" || status === "timed_out"}
                                  <span class="detail__item-detail">{item.detail}</span>
                                {:else if (status === "installed" || status === "upgraded") && item.detail.includes("ignored the requested directory")}
                                  <span class="detail__item-detail">{item.detail}</span>
                                {:else if status === "skipped_unmanaged"}
                                  <span class="detail__item-notes">{item.detail}</span>
                                {/if}
                                {#if item.log_path}
                                  <span class="detail__item-log">{item.log_path}</span>
                                {/if}
                              </li>
                            {/each}
                          </ul>
                        </li>
                      {/if}
                    {/each}
                  </ul>
                {/if}
                {:else}
                  <p class="sifting">{t("history.nothingToShow")}</p>
                {/if}
            </div>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .history {
    max-width: 920px;
    margin: 0 auto;
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

  .runs {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .runs__row {
    display: grid;
    grid-template-columns: 1fr auto;
    align-items: stretch;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-surface);
  }

  .runs__row.open {
    border-color: var(--border-strong);
  }

  .runs__toggle {
    display: grid;
    grid-template-columns: 150px 1fr auto;
    align-items: center;
    gap: var(--space-3);
    width: 100%;
    padding: var(--space-3) var(--space-4);
    border: none;
    border-radius: inherit;
    background: transparent;
    text-align: left;
    cursor: pointer;
    font: inherit;
    color: inherit;
  }

  .runs__toggle:hover {
    background: var(--bg-hover);
  }

  .runs__open-plan {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-2);
    align-self: center;
    margin-right: var(--space-3);
    padding: 5px 10px;
    border: none;
    border-radius: var(--radius);
    background: transparent;
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    font-weight: 500;
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    color: var(--text-muted);
    cursor: pointer;
    white-space: nowrap;
    transition: color var(--dur-fast) var(--ease-out),
      background var(--dur-fast) var(--ease-out);
  }

  .runs__open-plan:hover {
    color: var(--accent);
    background: var(--bg-hover);
  }

  .runs__date {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: var(--tracking-mono);
    color: var(--text);
  }

  .runs__presets {
    font-family: var(--font-display);
    font-size: var(--text-base);
    font-weight: 600;
    color: var(--text);
    overflow-wrap: anywhere;
  }

  .runs__meta {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    justify-content: flex-end;
  }

  .runs__duration {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
    color: var(--text-muted);
  }

  .runs__id {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
    padding: 1px 7px;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-lg);
    color: var(--text-muted);
    background: var(--bg-sunken);
  }

  .runs__detail {
    grid-column: 1 / -1;
    border-top: 1px dashed var(--border-strong);
    padding: var(--space-4);
  }

  .detail__head {
    margin-bottom: var(--space-3);
  }

  .detail__sub {
    margin: 0;
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  .detail__note {
    margin: var(--space-2) 0 0;
    font-size: var(--text-xs);
  }

  .detail__note--notes {
    color: var(--status-notes-text);
  }

  .detail__note--failed {
    color: var(--status-failed-text);
  }

  .detail__note--cancelled {
    color: var(--status-cancelled-text);
  }

  .detail__none {
    margin: 0;
    font-size: var(--text-xs);
    font-style: italic;
    color: var(--text-muted);
  }

  .detail__groups {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .detail__group-head {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin: 0 0 var(--space-1);
  }

  .detail__group-count {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
    color: var(--text-muted);
  }

  .detail__group-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .detail__item {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: var(--text-xs);
    color: var(--text);
  }

  .detail__item-name {
    font-weight: 600;
  }

  .detail__item-note {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
    color: var(--warm-text);
  }

  .detail__item-detail {
    color: var(--danger-text);
    overflow-wrap: anywhere;
  }

  .detail__item-notes {
    color: var(--status-notes-text);
    overflow-wrap: anywhere;
  }

  .detail__item-log {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    color: var(--text-muted);
    overflow-wrap: anywhere;
  }
</style>
