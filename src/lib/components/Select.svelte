<script lang="ts">
  import ContextMenu, { type ContextMenuState } from "./ContextMenu.svelte";
  import Icon from "./Icon.svelte";
  import { t } from "$lib/copy";

  /** One row in the dropdown popout (ticket 204): label-first like the old
   *  <option> text; title keeps the old option title tooltips (e.g. version
   *  policies, winget-less products); disabled rows render greyed and are
   *  skipped by keyboard navigation. */
  export interface SelectOption {
    value: string;
    label: string;
    disabled?: boolean;
    title?: string;
  }

  /** Shared select (ticket 45, rebuilt ticket 204): one treatment for every
   *  dropdown in the app. The trigger is a button in the shared filled input
   *  frame; the open state is a themed ContextMenu popout (align start,
   *  trigger-width) instead of the OS-native option list, which no styling
   *  can keep in sync (research 0021). Variants: default (body, full size),
   *  small (body, tighter), compact (mono, for env wiring action rows). */
  type Rest = Omit<
    import("svelte/elements").HTMLButtonAttributes,
    "value" | "onchange" | "onclick" | "class" | "children" | "disabled"
  >;

  let {
    id,
    value,
    options,
    onchange,
    variant = "default",
    class: klass = "",
    disabled = false,
    menuLabel,
    ...rest
  }: {
    id?: string;
    value: string;
    options: SelectOption[];
    onchange: (v: string) => void;
    variant?: "default" | "small" | "compact";
    class?: string;
    disabled?: boolean;
    /** Accessible name for the popout menu; defaults to the trigger's
     *  aria-label, else a generic prompt. */
    menuLabel?: string;
  } & Rest = $props();

  let trigger: HTMLButtonElement | undefined = $state();
  let menu: ContextMenuState | null = $state(null);

  const selectedLabel = $derived(
    options.find((o) => o.value === value)?.label ?? value,
  );

  // The companion site picker and dock filter (research 0006 pattern 10,
  // 0008 rule 3) are the prior art: an anchored shared menu with checked
  // radios, keyboard nav, and focus return — reused here, not reinvented.
  function toggle(viaKeyboard: boolean) {
    if (disabled) return;
    if (menu?.open) {
      menu = null;
      return;
    }
    const triggerLabel =
      menuLabel ??
      (typeof rest["aria-label"] === "string" && rest["aria-label"]
        ? (rest["aria-label"] as string)
        : t("select.chooseOption"));
    menu = {
      open: true,
      label: triggerLabel,
      anchor: trigger ?? null,
      focusFirst: viaKeyboard,
      returnTo: trigger ?? null,
      align: "start",
      matchAnchorWidth: true,
      items: options.map((o) => ({
        label: o.label,
        title: o.title,
        checked: o.value === value,
        disabled: o.disabled,
        onselect: () => onchange(o.value),
      })),
    };
  }
</script>

<div
  class="select {variant === "small" ? "select--small" : ""}{variant === "compact" ? " select--compact" : ""} {klass}"
  class:select--open={menu?.open ?? false}
>
  <button
    type="button"
    bind:this={trigger}
    {id}
    class="select__control"
    data-value={value}
    aria-haspopup="menu"
    aria-expanded={menu?.open ?? false}
    {disabled}
    data-ctx-trigger
    onclick={(e) => toggle(e.detail === 0)}
    {...rest}
  >
    <span class="select__value">{selectedLabel}</span>
    <span class="select__chevron" aria-hidden="true">
      <Icon name="chevron-down" size={variant === "compact" ? 12 : 14} />
    </span>
  </button>
  <ContextMenu ctx={menu} onclose={() => (menu = null)} />
</div>

<style>
  .select {
    position: relative;
    display: inline-flex;
    min-width: 0;
  }

  .select__control {
    position: relative;
    display: inline-flex;
    align-items: center;
    width: 100%;
    min-width: 0;
    font-family: var(--font-body);
    font-size: var(--text-base);
    text-align: left;
    color: var(--text);
    /* The shared filled frame (research 0021): sunken rest with a hairline
       edge — the Discord staging in Ledger neutrals — washing on hover and
       glowing on focus, accent spent at focus only (0006 pattern 6). */
    background: var(--bg-sunken);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    padding: 8px 30px 8px 10px;
    cursor: pointer;
    /* WHY three properties: quiet rest, hover wash, glowing focus (research
       0020 enhancement, ticket 204 restyle). Paint-only; layout never
       animates. */
    transition: border-color var(--dur-fast) var(--ease-out),
      background-color var(--dur-fast) var(--ease-out),
      box-shadow var(--dur-fast) var(--ease-out);
  }

  .select__control:hover:not(:disabled) {
    background-color: var(--bg-hover);
    border-color: var(--border-strong);
  }

  .select__control:focus-visible {
    outline: none;
    border-color: var(--accent);
    box-shadow: var(--ring-glow);
  }

  .select__control:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }

  .select__value {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .select--small .select__control {
    font-size: var(--text-sm);
    padding: 6px 26px 6px 8px;
  }

  .select--compact .select__control {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    border-radius: var(--radius-sm);
    padding: 6px 24px 6px 8px;
  }

  .select__chevron {
    position: absolute;
    right: 8px;
    top: 50%;
    transform: translateY(-50%);
    display: inline-flex;
    color: var(--text-muted);
    pointer-events: none;
    /* The chevron arrival rides the restrained spring (ADR-0034, ticket 202):
       interruptible, settling instead of snapping. */
    transition: color var(--dur-fast) var(--ease-out),
      transform var(--dur-fast) var(--ease-spring);
  }

  .select__control:hover:not(:disabled) .select__chevron {
    color: var(--text);
  }

  .select--open .select__chevron {
    color: var(--accent);
    transform: translateY(-50%) rotate(180deg);
  }

  .select--compact .select__chevron {
    right: 6px;
  }
</style>
