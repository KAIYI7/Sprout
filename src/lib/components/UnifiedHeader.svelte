<!-- The main window's unified header (research 0025): one continuous strip
  carrying the app mark + current-section breadcrumb on the left and the
  native-idiom window buttons on the right. The bar itself is the Tauri drag
  region; the button cluster opts out so presses land on the buttons. Never
  holds a primary button or search (research 0005 rule 2). -->
<script lang="ts">
  import IconButton from "./IconButton.svelte";
  import SproutMark from "./SproutMark.svelte";
  import { t } from "$lib/copy";

  let {
    section,
    maximized = false,
    onMinimize,
    onToggleMaximize,
    onClose,
  }: {
    /** Current-section context label (the shared route map, never blank). */
    section: string;
    /** Reflects the live window state — swaps the glyph and the end padding. */
    maximized?: boolean;
    onMinimize: () => void;
    onToggleMaximize: () => void;
    onClose: () => void;
  } = $props();

  function handleDblClick(e: MouseEvent) {
    // A double-click on a button is a press, not a maximize gesture.
    if ((e.target as HTMLElement).closest("button")) return;
    onToggleMaximize();
  }
</script>

<header
  role="banner"
  aria-label={t("chrome.barLabel")}
  class="win-header"
  class:win-header--maximized={maximized}
  data-tauri-drag-region="deep"
  ondblclick={handleDblClick}
>
  <div class="win-header__context">
    <SproutMark size={14} />
    <span class="win-header__wordmark">Sprout</span>
    <span class="win-header__sep" aria-hidden="true">·</span>
    <span class="win-header__section">{section}</span>
  </div>
  <div class="win-header__controls" data-tauri-drag-region="false">
    <IconButton icon="minus" label={t("chrome.minimize")} onclick={onMinimize} />
    <IconButton
      icon={maximized ? "restore" : "square"}
      label={maximized ? t("chrome.restore") : t("chrome.maximize")}
      onclick={onToggleMaximize}
    />
    <span class="win-header__close">
      <IconButton icon="x" label={t("common.close")} onclick={onClose} />
    </span>
  </div>
</header>

<style>
  .win-header {
    display: flex;
    flex: none;
    align-items: center;
    gap: var(--space-2);
    height: 40px;
    /* One continuous surface with the rail (research 0025): the same surface,
       no hairline — the surface change against the page carries the
       structure, the way Discord and Task Manager carry it. The mark starts
       where the rail pills start, so one left edge runs down the app. */
    padding: 0 var(--space-2) 0 var(--space-3);
    background: var(--bg-surface);
    /* The bar is a drag handle first — text here is context, not content. */
    user-select: none;
    -webkit-user-select: none;
  }

  /* Maximized windows meet the screen edge: square corners are inherent to
     the frameless window, and the controls step off the edge pixels. */
  .win-header--maximized {
    padding-right: var(--space-3);
  }

  .win-header__context {
    display: flex;
    min-width: 0;
    flex: 1;
    align-items: center;
    gap: var(--space-2);
  }

  .win-header__wordmark {
    font-size: var(--text-xs);
    font-weight: 600;
    color: var(--text);
    white-space: nowrap;
  }

  .win-header__sep {
    color: var(--text-faint);
  }

  .win-header__section {
    overflow: hidden;
    font-size: var(--text-xs);
    color: var(--text-muted);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .win-header__controls {
    display: flex;
    flex: none;
    align-items: center;
    gap: 2px;
  }

  /* Accent stays reserved for the primary CTA (research 0006 pattern 6) —
     only Close spends color, and only on hover. */
  .win-header__close :global(.icon-btn:hover:not(:disabled)) {
    background: var(--danger-tint);
    color: var(--danger-text);
  }
</style>
