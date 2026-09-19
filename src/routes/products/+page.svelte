<script lang="ts">
  import type { Product } from "$lib/types";
  import { listProducts, createProduct, updateProduct, deleteProduct, productPresetImpact } from "$lib/api";
  import { goto } from "$app/navigation";
  import Button from "$lib/components/Button.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import PageHeader from "$lib/components/PageHeader.svelte";
  import SearchInput from "$lib/components/SearchInput.svelte";
  import ProductPacket from "$lib/components/ProductPacket.svelte";
  import ProductFormDialog from "$lib/components/ProductFormDialog.svelte";
  import ProductDetailsDialog from "$lib/components/ProductDetailsDialog.svelte";
  import ConfirmDialog from "$lib/components/ConfirmDialog.svelte";
  import EmptyState from "$lib/components/EmptyState.svelte";
  import Notice from "$lib/components/Notice.svelte";
  import ContextMenu, {
    type ContextMenuState,
    type MenuRequest,
  } from "$lib/components/ContextMenu.svelte";
  import { t, tCount } from "$lib/copy";

  let products = $state<Product[]>([]);
  let query = $state("");
  let loading = $state(true);
  let loadFailed = $state(false);
  let error = $state("");
  let notice = $state("");
  let formOpen = $state(false);
  let editing: Product | null = $state(null);
  let deleting: Product | null = $state(null);
  let deletingImpact = $state(0);
  let details: Product | null = $state(null);
  let menu: (ContextMenuState & { productId: string }) | null = $state(null);
  let animateRack = $state(false);

  let debounce: ReturnType<typeof setTimeout>;

  $effect(() => {
    const q = query;
    clearTimeout(debounce);
    debounce = setTimeout(() => load(q), 120);
  });

  async function load(q: string) {
    loading = true;
    try {
      const rows = await listProducts(q.trim() ? q.trim() : null);
      products = rows;
      loadFailed = false;
    } catch (e) {
      console.error(e);
      loadFailed = true;
    } finally {
      loading = false;
    }
  }

  function flash(message: string) {
    notice = message;
    setTimeout(() => (notice = ""), 3200);
  }

  function openAdd() {
    editing = null;
    error = "";
    formOpen = true;
  }

  function openEdit(product: Product) {
    editing = product;
    error = "";
    formOpen = true;
  }

  async function save(product: Product) {
    error = "";
    try {
      if (editing) {
        await updateProduct(product);
        flash(t("common.savedName").replace("{name}", product.name));
      } else {
        await createProduct(product);
        flash(t("products.addedFlash").replace("{name}", product.name));
        animateRack = true;
      }
      formOpen = false;
      await load(query);
    } catch (e) {
      console.error(e);
      error =
        String(e) ||
        (editing
          ? t("products.saveFailName").replace("{name}", product.name)
          : t("products.addFail"));
    }
  }

  async function confirmDelete() {
    if (!deleting) return;
    const name = deleting.name;
    try {
      await deleteProduct(deleting.id);
      flash(t("products.removedFlash").replace("{name}", name));
      deleting = null;
      await load(query);
    } catch (e) {
      console.error(e);
      error = t("common.removeFail").replace("{name}", name);
    }
  }

  function closeMenu() {
    menu = null;
  }

  function openProductMenu(product: Product, request: MenuRequest) {
    // The ⋯ button and Enter on a focused card toggle; right-click re-positions.
    if (request.kind === "anchor" && menu?.productId === product.id) {
      closeMenu();
      return;
    }
    menu = {
      productId: product.id,
      open: true,
      label: t("packet.actionsFor").replace("{name}", product.name),
      focusFirst: request.kind === "anchor" ? request.focusFirst : false,
      returnTo: request.returnTo,
      ...(request.kind === "cursor"
        ? { x: request.x, y: request.y }
        : { anchor: request.anchor }),
      items: [
        {
          label: t("menu.installNow"),
          icon: "play",
          onselect: () =>
            goto(`/plan?quick=${encodeURIComponent(product.id)}`),
        },
        { label: t("common.edit"), icon: "pencil", onselect: () => openEdit(product) },
        { label: t("common.moreInfo"), icon: "info", onselect: () => (details = product) },
        // Ticket 106's ordering standard: destruction last, separated.
        { label: "", separator: true, onselect: () => {} },
        {
          label: t("common.remove"),
          icon: "trash",
          danger: true,
          onselect: () => {
            deleting = product;
            deletingImpact = 0;
            productPresetImpact(product.id)
              .then((impact) => (deletingImpact = impact.preset_count))
              .catch((e) => console.error(e));
          },
        },
      ],
    };
  }
</script>

<section class="library" aria-labelledby="library-title">
  <PageHeader titleId="library-title" title={t("nav.products")}>
    {#snippet actions()}
      <Button onclick={openAdd}>
        <Icon name="plus" size={15} />
        {t("products.add")}
      </Button>
    {/snippet}
    {#snippet subtitle()}
      {products.length === 1 ? tCount("products.countOne", products.length) : tCount("products.countMany", products.length)}
      {query.trim() ? (products.length === 1 ? t("products.searchOne") : t("products.searchMany")) : t("launch.filterNone")}
      {t("products.hintRight")}
    {/snippet}
    {#snippet toolbar()}
      <SearchInput
        value={query}
        placeholder={t("products.filterPh")}
        onchange={(v) => (query = v)}
      />
    {/snippet}
  </PageHeader>

  {#if notice}
    <Notice tone="ok">{notice}</Notice>
  {/if}
  {#if error}
    <Notice tone="error">{error}</Notice>
  {/if}

  {#if loading && products.length === 0}
    <p class="sifting" aria-live="polite">Loading…</p>
  {:else if loadFailed}
    <EmptyState icon="x" title={t("common.libraryReadFail")}>
      <p>{t("products.dbBody")}
        <span class="mono">%LOCALAPPDATA%\Sprout\sprout.db</span>.</p>
      <p>{t("common.dbLockedHint")}</p>
      <div class="empty-cta">
        <Button variant="secondary" onclick={() => load(query)}>{t("common.retry")}</Button>
      </div>
    </EmptyState>
  {:else if products.length === 0 && !query.trim()}
    <EmptyState title={t("products.noProducts")}>
      <p>{t("products.emptyBody")}</p>
      <div class="empty-cta">
        <Button onclick={openAdd}>
          <Icon name="plus" size={15} />
          {t("products.add")}
        </Button>
      </div>
    </EmptyState>
  {:else if products.length === 0}
    <EmptyState title={t("common.noMatchFor").replace("{query}", query.trim())}>
      <p>{t("products.noSearchHint")}</p>
      <div class="empty-cta">
        <Button variant="secondary" onclick={() => (query = "")}>{t("common.clearSearch")}</Button>
      </div>
    </EmptyState>
  {:else}
    <ul class="rack" class:animate={animateRack}>
      {#each products as product, i (product.id)}
        <li class="rack__cell">
          <ProductPacket
            {product}
            index={i}
            animate={animateRack}
            expanded={menu?.productId === product.id}
            onmenu={(request) => openProductMenu(product, request)}
            oninfo={() => (details = product)}
          />
        </li>
      {/each}
    </ul>
  {/if}
</section>

<ProductFormDialog
  open={formOpen}
  product={editing}
  error={error}
  onsave={save}
  oncancel={() => (formOpen = false)}
  onerror={(message) => (error = message)}
/>

<ProductDetailsDialog open={details !== null} product={details} onclose={() => (details = null)} />

<ConfirmDialog
  open={deleting !== null}
  title={t("products.removeTitle")}
  confirmLabel={t("common.remove")}
  danger
  onconfirm={confirmDelete}
  oncancel={() => (deleting = null)}
>
  <p>
    <strong>{deleting?.name}</strong> ({deleting?.winget_id ?? t("products.customStepLong")}){t("products.removeBodyTail")}
    {#if deletingImpact > 0}
      {deletingImpact === 1 ? t("products.impactOne").replace("{count}", String(deletingImpact)) : t("products.impactMany").replace("{count}", String(deletingImpact))}
    {:else}
      {t("products.impactNone")}
    {/if}
  </p>
</ConfirmDialog>

<ContextMenu ctx={menu} onclose={closeMenu} />

<style>
  .library {
    max-width: 1080px;
    margin: 0 auto;
  }

  .sifting {
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    letter-spacing: var(--tracking-mono);
    color: var(--text-muted);
  }

  .rack {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(230px, 1fr));
    gap: var(--space-4);
  }

  .rack__cell {
    min-width: 0;
  }

  .empty-cta {
    margin-top: var(--space-4);
  }

  .mono {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
  }
</style>