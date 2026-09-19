<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { getSettings, setCompanionUrl, setCompanionUrlList } from "$lib/api";
  import type { CompanionSite } from "$lib/types";
  import { companionDisplayName, companionUrlKey, normalizeCompanionSites } from "$lib/companion";
  import PageHeader from "$lib/components/PageHeader.svelte";
  import Button from "$lib/components/Button.svelte";
  import Notice from "$lib/components/Notice.svelte";
  import EmptyState from "$lib/components/EmptyState.svelte";
  import IconButton from "$lib/components/IconButton.svelte";
  import Dialog from "$lib/components/Dialog.svelte";
  import Badge from "$lib/components/Badge.svelte";
  import ConfirmDialog from "$lib/components/ConfirmDialog.svelte";
  import Select from "$lib/components/Select.svelte";
  import { t } from "$lib/copy";

  // Companion manager — machine-local only, never in Preset exports (the settings
  // row itself never travels in backups either). Reuses PageHeader / Dialog /
  // IconButton (research 0006 visibility-on-surface vs configuration-elsewhere,
  // 0008 page-features never, 0004 content-gated). Saved sites carry a display
  // name each: add/rename/remove, reorder via ordered_list discipline
  // (position-preserving), duplicates refused on URL and on name.

  let sites = $state<CompanionSite[]>([]);
  let activeUrl: string | null = $state(null);
  let editIndex: number | null = $state(null);
  let siteDraft = $state("");
  let nameDraft = $state("");
  let uaDraft = $state<"mobile" | "desktop">("mobile");
  let formOpen = $state(false);
  let formError = $state("");
  let removeIndex: number | null = $state(null);
  let loading = $state(true);
  let error = $state("");
  let notice = $state("");
  let noticeSiteUrl: string | null = $state(null);
  let noticeTimer: ReturnType<typeof setTimeout> | undefined;

  function normalizeList(list: CompanionSite[]): CompanionSite[] {
    return normalizeCompanionSites(list);
  }
  function validateInput(url: string): string | null {
    const trimmed = url.trim();
    if (!trimmed) return t("companion.urlEmpty");
    if (!trimmed.toLowerCase().startsWith("https://")) return t("companion.urlScheme");
    if (trimmed.includes(" ")) return t("companion.urlValid");
    return null;
  }
  function duplicateUrl(url: string, except: number | null): string | null {
    const key = companionUrlKey(url);
    const hit = sites.find((s, i) => i !== except && companionUrlKey(s.url) === key);
    return hit ? t("companion.dupeUrl").replace("{url}", url.trim()) : null;
  }
  function duplicateName(name: string, except: number | null): string | null {
    const trimmed = name.trim();
    if (!trimmed) return null;
    const hit = sites.find((s, i) => i !== except && s.name.trim().toLowerCase() === trimmed.toLowerCase());
    return hit ? t("companion.dupeName").replace("{name}", trimmed) : null;
  }
  function flash(msg: string) {
    notice = msg;
    noticeSiteUrl = null;
    clearTimeout(noticeTimer);
    noticeTimer = setTimeout(() => {
      notice = "";
      noticeSiteUrl = null;
    }, 3200);
  }
  function flashAdded(site: CompanionSite) {
    notice = t("companion.addedFlash").replace("{name}", companionDisplayName(site));
    noticeSiteUrl = site.url;
    clearTimeout(noticeTimer);
    noticeTimer = setTimeout(() => {
      notice = "";
      noticeSiteUrl = null;
    }, 3200);
  }
  async function enableNoticedSite() {
    const url = noticeSiteUrl;
    if (!url) return;
    try {
      await setCompanionUrl(url);
      activeUrl = url;
      error = "";
      const saved = sites.find((s) => s.url.toLowerCase() === url.toLowerCase());
      flash(t("companion.enabledFlash").replace("{name}", companionDisplayName(saved ?? { url, name: "", ua: "mobile" })));
    } catch (e) {
      error = String(e);
    }
  }

  onMount(() => {
    void load();
    return () => clearTimeout(noticeTimer);
  });

  async function load() {
    loading = true;
    try {
      const s = await getSettings();
      sites = normalizeList(s.companion_url_list ?? []);
      activeUrl = s.companion_url ?? null;
      error = "";
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function persistList(next: CompanionSite[]) {
    const normalized = normalizeList(next);
    await setCompanionUrlList(normalized);
    sites = normalized;
    // If active disappeared, clear it
    if (activeUrl && !normalized.some((s) => s.url.toLowerCase() === activeUrl!.toLowerCase())) {
      activeUrl = null;
      await setCompanionUrl(null);
    }
  }

  async function addUrl() {
    const err = validateInput(siteDraft);
    if (err) { formError = err; return; }
    const trimmed = siteDraft.trim();
    const dupe = duplicateUrl(trimmed, null) ?? duplicateName(nameDraft, null);
    if (dupe) { formError = dupe; return; }
    const next = [...sites, { url: trimmed, name: nameDraft.trim(), ua: uaDraft }];
    try {
      await persistList(next);
      siteDraft = "";
      nameDraft = "";
      uaDraft = "mobile";
      formOpen = false;
      formError = "";
      error = "";
      flashAdded(sites[sites.length - 1]);
    } catch (e) { formError = String(e); }
  }
  function startEdit(idx: number) {
    editIndex = idx;
    siteDraft = sites[idx]?.url ?? "";
    nameDraft = sites[idx]?.name ?? "";
    uaDraft = sites[idx]?.ua === "desktop" ? "desktop" : "mobile";
    formError = "";
    formOpen = true;
  }
  function cancelEdit() {
    formOpen = false;
    editIndex = null;
    siteDraft = "";
    nameDraft = "";
    uaDraft = "mobile";
    formError = "";
  }
  function startAdd() {
    editIndex = null;
    siteDraft = "";
    nameDraft = "";
    uaDraft = "mobile";
    formError = "";
    formOpen = true;
  }
  async function saveEdit() {
    if (editIndex === null) return;
    const err = validateInput(siteDraft);
    if (err) { formError = err; return; }
    const trimmed = siteDraft.trim();
    const dupe = duplicateUrl(trimmed, editIndex) ?? duplicateName(nameDraft, editIndex);
    if (dupe) { formError = dupe; return; }
    const wasActive = activeUrl && sites[editIndex!].url.toLowerCase() === activeUrl!.toLowerCase();
    const next = [...sites];
    next[editIndex!] = {
      ...sites[editIndex!]!,
      url: trimmed,
      name: nameDraft.trim(),
      ua: uaDraft,
    };
    try {
      await persistList(next);
      if (wasActive) {
        await setCompanionUrl(trimmed);
        activeUrl = trimmed;
      }
      editIndex = null;
      siteDraft = "";
      nameDraft = "";
      uaDraft = "mobile";
      formOpen = false;
      formError = "";
      error = "";
      flash(t("companion.savedFlash"));
    } catch (e) { formError = String(e); }
  }
  async function removeUrl(idx: number) {
    const removed = sites[idx];
    const next = sites.filter((_, i) => i !== idx);
    try {
      await persistList(next);
      if (activeUrl && removed.url.toLowerCase() === activeUrl.toLowerCase()) {
        activeUrl = null;
      }
      flash(t("companion.removedFlash").replace("{name}", companionDisplayName(removed)));
    } catch (e) { error = String(e); }
  }
  async function moveUrl(idx: number, dir: -1 | 1) {
    const target = idx + dir;
    if (target < 0 || target >= sites.length) return;
    const next = [...sites];
    const tmp = next[idx];
    next[idx] = next[target];
    next[target] = tmp;
    try {
      await persistList(next);
    } catch (e) { error = String(e); }
  }
</script>

<svelte:head>
  <title>{t("nav.companion")} — Sprout</title>
</svelte:head>

<section class="companion" aria-labelledby="companion-title">
  <PageHeader titleId="companion-title" title={t("nav.companion")}>
    {#snippet actions()}
      <Button variant="secondary" onclick={() => goto("/settings")}>{t("companion.backToSettings")}</Button>
      <Button onclick={startAdd}>{t("companion.addSite")}</Button>
    {/snippet}
    {#snippet subtitle()}
      {t("companion.subtitle")}
    {/snippet}
  </PageHeader>

  {#if error}
    <Notice tone="error">{error}</Notice>
  {/if}
  {#if notice}
    {#if noticeSiteUrl}
      <Notice tone="ok">
        {notice}
        {#snippet action()}
          <Button onclick={() => void enableNoticedSite()}>{t("companion.enableNow")}</Button>
        {/snippet}
      </Notice>
    {:else}
      <Notice tone="ok">{notice}</Notice>
    {/if}
  {/if}

  {#if loading}
    <p class="sifting" aria-live="polite">Loading…</p>
  {:else}
    <section class="saved-sites" aria-labelledby="saved-sites-title">
      <div class="saved-sites__header">
        <div>
          <h2 id="saved-sites-title" class="saved-sites__title">{t("settings.companion-sites.label")}</h2>
        </div>
        <span class="saved-sites__count">{sites.length}</span>
      </div>
      {#if sites.length > 0}
        <ul class="saved-list">
          {#each sites as site, idx (site.url)}
            <li class="saved-row">
              <span class="saved-row__text">
                <span class="saved-row__name" title={site.url}>{companionDisplayName(site)}</span>
                {#if site.name.trim()}
                  <span class="saved-row__url" title={site.url}>{site.url}</span>
                {/if}
                {#if site.ua === "desktop"}
                  <span class="saved-row__url">{t("companion.desktopBadge")}</span>
                {/if}
              </span>
              {#if activeUrl && activeUrl.toLowerCase() === site.url.toLowerCase()}
                <Badge tone="accent">{t("companion.activeBadge")}</Badge>
              {/if}
              <Button variant="ghost" onclick={() => startEdit(idx)}>{t("common.edit")}</Button>
              <IconButton icon="chevron-up" label={t("menu.moveUp")} quiet onclick={() => moveUrl(idx, -1)} disabled={idx===0} />
              <IconButton icon="chevron-down" label={t("menu.moveDown")} quiet onclick={() => moveUrl(idx, 1)} disabled={idx===sites.length-1} />
              <Button variant="ghost" onclick={() => (removeIndex = idx)}>{t("common.remove")}</Button>
            </li>
          {/each}
        </ul>
      {:else}
        <EmptyState icon="monitor" title={t("companion.noSites")}>
          <p>{t("companion.noSitesBody")}</p>
        </EmptyState>
      {/if}
    </section>
  {/if}
</section>

<Dialog open={formOpen} title={editIndex === null ? t("companion.addTitle") : t("companion.editTitle")} onclose={cancelEdit}>
  <form
    class="site-form"
    onsubmit={(event) => {
      event.preventDefault();
      if (editIndex === null) void addUrl();
      else void saveEdit();
    }}
  >
    <label class="site-form__label" for="companion-site-name">{t("companion.nameLabel")}</label>
    <input
      id="companion-site-name"
      name="companion-site-name"
      class="site-form__input"
      type="text"
      autocomplete="off"
      spellcheck="false"
      placeholder={t("companion.namePh")}
      value={nameDraft}
      oninput={(event) => (nameDraft = (event.target as HTMLInputElement).value)}
      aria-describedby="companion-site-name-hint"
    />
    <p id="companion-site-name-hint" class="site-form__hint">{t("companion.nameHint")}</p>
    <label class="site-form__label" for="companion-site-url">{t("companion.urlLabel")}</label>
    <input
      id="companion-site-url"
      name="companion-site-url"
      class="site-form__input"
      type="text"
      inputmode="url"
      autocomplete="off"
      spellcheck="false"
      placeholder={t("companion.urlPh")}
      value={siteDraft}
      oninput={(event) => (siteDraft = (event.target as HTMLInputElement).value)}
      aria-describedby={formError ? "companion-site-error" : "companion-site-hint"}
    />
    {#if formError}
      <p id="companion-site-error" class="site-form__error" role="alert">{formError}</p>
    {:else}
      <p id="companion-site-hint" class="site-form__hint">{t("companion.urlHint")}</p>
    {/if}
    <label class="site-form__label" for="companion-site-ua">{t("companion.uaLabel")}</label>
    <Select
      id="companion-site-ua"
      variant="small"
      value={uaDraft}
      onchange={(v) => (uaDraft = v === "desktop" ? "desktop" : "mobile")}
      options={[
        { value: "mobile", label: t("companion.uaMobile") },
        { value: "desktop", label: t("companion.uaDesktop") },
      ]}
    />
    <p class="site-form__hint">{t("companion.uaHint")}</p>
    <div class="site-form__actions">
      <Button variant="ghost" type="button" onclick={cancelEdit}>{t("common.cancel")}</Button>
      <Button kind="submit">{editIndex === null ? t("companion.addSite") : t("common.saveChanges")}</Button>
    </div>
  </form>
</Dialog>

<ConfirmDialog
  open={removeIndex !== null}
  title={t("companion.removeTitle")}
  confirmLabel={t("companion.removeConfirm")}
  danger
  oncancel={() => (removeIndex = null)}
  onconfirm={() => {
    const index = removeIndex;
    removeIndex = null;
    if (index !== null) void removeUrl(index);
  }}
>
  <p>{t("companion.removeBody")}</p>
</ConfirmDialog>

<style>
  .companion {
    max-width: 1080px;
    margin: 0 auto;
  }
  .sifting {
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    letter-spacing: var(--tracking-mono);
    color: var(--text-muted);
  }
  .site-form__label {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    font-weight: 500;
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    color: var(--accent);
  }
  .site-form__hint {
    margin: 0;
    font-size: var(--text-xs);
    line-height: var(--leading-tight);
    color: var(--text-muted);
  }
  .site-form__input {
    width: 100%;
    font-family: var(--font-mono);
    font-size: var(--text-base);
    color: var(--text);
    /* The shared filled frame (research 0021, ticket 204). */
    background: var(--bg-sunken);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    padding: var(--space-2) var(--space-3);
    text-align: left;
    font-variant-ligatures: none;
    /* WHY three properties: the shared progressive input frame — quiet rest,
       hover wash, glowing focus (research 0020 enhancement). */
    transition: border-color var(--dur-fast) var(--ease-out),
      background-color var(--dur-fast) var(--ease-out),
      box-shadow var(--dur-fast) var(--ease-out);
  }
  .site-form__input:hover {
    background-color: var(--bg-hover);
    border-color: var(--border-strong);
  }
  .site-form__input:focus-visible {
    outline: none;
    border-color: var(--accent);
    box-shadow: var(--ring-glow);
  }
  .saved-sites__count {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: var(--tracking-mono);
    color: var(--text-muted);
  }
  .saved-sites {
    margin-top: var(--space-6);
  }
  .saved-sites__header {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: var(--space-4);
    margin-bottom: var(--space-3);
  }
  .saved-sites__title {
    margin: 0 0 var(--space-1);
    font-family: var(--font-display);
    font-size: var(--text-lg);
    font-weight: 600;
    color: var(--text);
  }
  .saved-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .saved-row {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
    padding: var(--space-3);
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  .saved-row__text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }
  .saved-row__name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--text);
  }
  .saved-row__url {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-muted);
    font-variant-ligatures: none;
  }
  .site-form {
    display: flex;
    flex-direction: column;
    /* Discord field rhythm without markup churn (research 0021 spacing
       round): 8px owns label → control → hint; each label after the first
       stands 24px off the previous hint (8px gap + 16px margin). */
    gap: var(--space-2);
  }
  .site-form__label:not(:first-child) {
    margin-top: var(--space-4);
  }
  .site-form__actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
    margin-top: var(--space-2);
  }
  .site-form__error {
    margin: 0;
    font-size: var(--text-xs);
    color: var(--danger-text);
  }
</style>
