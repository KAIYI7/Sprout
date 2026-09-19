<script lang="ts">
  import { page } from "$app/state";
  import ConfirmDialog from "./ConfirmDialog.svelte";
  import Notice from "./Notice.svelte";
  import { installNow, updateState } from "$lib/updateState.svelte";
  import { t } from "$lib/copy";

  // Frequency split (research 0004): daily quick surfaces first, setup next,
  // reference last — clusters separated by hairline dividers, no headings.
  // Derived (not const) so the rail follows a language switch live.
  const clusters = $derived([
    [
      { id: "launch", label: t("nav.launch"), href: "/" },
      {
        id: "quick-actions",
        label: t("nav.actions"),
        href: "/quick-actions",
      },
      { id: "clips", label: t("nav.clips"), href: "/clips" },
    ],
    [
      { id: "products", label: t("nav.products"), href: "/products" },
      { id: "presets", label: t("nav.presets"), href: "/presets" },
      { id: "plan", label: t("nav.plan"), href: "/plan" },
    ],
    [
      { id: "history", label: t("nav.history"), href: "/history" },
      { id: "logs", label: t("nav.logs"), href: "/logs" },
      { id: "settings", label: t("nav.settings"), href: "/settings" },
    ],
  ]);

  const current = $derived(page.url.pathname);

  let confirmOpen = $state(false);
  let installError = $state("");

  function openConfirm() {
    installError = "";
    confirmOpen = true;
  }

  async function applyUpdate() {
    try {
      await installNow();
      // A successful spawn exits the app within the second; closing the
      // dialog is only for the moment before that lands.
      confirmOpen = false;
    } catch (e) {
      // Failure reopens the dialog with the error so trying again is one
      // click away; the pill stays clickable meanwhile.
      installError = String(e);
      confirmOpen = true;
    }
  }
</script>

<nav class="rail" aria-label={t("nav.sections")}>
  <ul class="rail__list">
    {#each clusters as cluster, ci}
      {#if ci > 0}
        <li class="rail__divider" aria-hidden="true"></li>
      {/if}
      {#each cluster as item}
        <li>
          <a
            href={item.href}
            class="rail__item"
            class:active={current === item.href}
            aria-current={current === item.href ? "page" : undefined}
          >
            <span class="rail__label">{item.label}</span>
          </a>
        </li>
      {/each}
    {/each}
  </ul>

  <div class="rail__foot">
    {#if updateState.installing}
      <span class="rail__update rail__update--busy" role="status"
        >{t("common.busy.updating")}</span
      >
    {:else if updateState.available}
      <button
        type="button"
        class="rail__update"
        title={t("update.available")}
        aria-label={t("update.railAria").replace("{version}", updateState.available.version)}
        onclick={openConfirm}
      >
        v{updateState.currentVersion} ↑ {updateState.available.version}
      </button>
    {:else}
      <p class="rail__version">
        {updateState.currentVersion ? `v${updateState.currentVersion}` : ""}
      </p>
    {/if}
  </div>
</nav>

<ConfirmDialog
  open={confirmOpen}
  title={t("update.available")}
  confirmLabel={updateState.installing ? t("common.busy.installing") : t("update.installRestart")}
  onconfirm={applyUpdate}
  oncancel={() => (confirmOpen = false)}
>
  <p>{t("update.confirmBody").replace("{version}", updateState.available?.version ?? "")}</p>
  <p>{t("update.confirmNote")}</p>
  {#if installError}
    <Notice tone="error">{installError}</Notice>
  {/if}
</ConfirmDialog>

<style>
  .rail {
    display: flex;
    flex-direction: column;
    width: 200px;
    flex-shrink: 0;
    /* One continuous surface with the header (research 0025): no hairline of
       its own — the surface change against the page carries the structure. */
    background: var(--bg-surface);
    padding: var(--space-5) var(--space-3) var(--space-4);
  }

  .rail__list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .rail__divider {
    height: 1px;
    margin: var(--space-2) var(--space-1);
    background: var(--border);
  }

  .rail__item {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 7px var(--space-3);
    border-radius: var(--radius);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    font-weight: 500;
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    text-decoration: none;
    color: var(--text-muted);
    transition: background var(--dur-fast) var(--ease-out),
      color var(--dur-fast) var(--ease-out);
  }

  .rail__item:hover {
    background: var(--bg-hover);
    color: var(--text);
  }

  .rail__item.active {
    background: var(--accent-tint);
    color: var(--accent);
  }

  .rail__foot {
    margin: auto 0 0;
    padding: var(--space-3) var(--space-2) 0;
  }

  .rail__foot :global(p) {
    margin: 0;
  }

  .rail__version {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
    color: var(--text-muted);
  }

  .rail__update {
    display: block;
    width: 100%;
    padding: var(--space-1) var(--space-3);
    border: 1px solid var(--accent-tint-border);
    border-radius: var(--radius-pill);
    background: var(--accent-tint);
    color: var(--accent);
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
    text-align: center;
    cursor: pointer;
    transition: background var(--dur-fast) var(--ease-out),
      border-color var(--dur-fast) var(--ease-out);
  }

  .rail__update:hover:not(:disabled),
  .rail__update--busy {
    background: var(--bg-sunken);
    border-color: var(--accent);
  }

  .rail__update--busy {
    color: var(--accent);
    cursor: default;
  }
</style>
