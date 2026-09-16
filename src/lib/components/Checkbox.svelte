<script lang="ts">
  import type { Snippet } from "svelte";
  import InfoTip from "./InfoTip.svelte";

  /** Shared in-dialog flag checkbox (research 0022 verdict 3): the styled
   *  successor to every hand-rolled `.stoppable` / `.showwin` / `.dockvis`
   *  row. Semantics stay a checkbox — submit-deferred flags must never
   *  promise a switch's immediacy — while the box itself is drawn in Ledger
   *  tokens: quiet sunken rest, accent spent on the checked state only, the
   *  input focus-glow on keyboard focus. The native input stays underneath,
   *  so keyboard and screen-reader behavior come free. */
  let {
    checked,
    onchange,
    title,
    hint,
    info,
    infobody,
    infotone = "info",
  }: {
    checked: boolean;
    onchange: (v: boolean) => void;
    title: string;
    hint?: string;
    /** When set, an InfoTip trigger sits beside the title, outside the
     *  label so its button never nests inside one. */
    info?: string;
    infobody?: Snippet;
    infotone?: "info" | "warn";
  } = $props();
</script>

<div class="check">
  <label class="check__label">
    <span class="check__control">
      <input
        type="checkbox"
        class="check__input"
        {checked}
        onchange={(e) => onchange((e.target as HTMLInputElement).checked)}
      />
      <span class="check__box" aria-hidden="true">
        <svg class="check__tick" viewBox="0 0 24 24" fill="none" aria-hidden="true">
          <path d="m5 12.5 4.5 4.5L19 7.5" />
        </svg>
      </span>
    </span>
    <span class="check__text">
      <span class="check__title">{title}</span>
      {#if hint}<span class="check__hint">{hint}</span>{/if}
    </span>
  </label>
  {#if info}
    <InfoTip label={info} tone={infotone}>{@render infobody?.()}</InfoTip>
  {/if}
</div>

<style>
  .check {
    display: flex;
    align-items: flex-start;
    gap: var(--space-1);
    min-width: 0;
  }

  /* WHY the nested label: the box + title + hint share one hit target with
     no dead zones, while the InfoTip button stays a sibling — a button
     nested inside a label would toggle the flag when opened (research 0022,
     web-design-guidelines label rule). */
  .check__label {
    display: flex;
    align-items: flex-start;
    gap: var(--space-2);
    flex: 1;
    min-width: 0;
    cursor: pointer;
  }

  .check__control {
    position: relative;
    flex: none;
    width: 16px;
    height: 16px;
    margin-top: 2px;
  }

  /* WHY opacity instead of display:none: the native control stays focusable
     and announced, so keyboard and screen-reader behavior come free
     (research 0022 verdict 3). The -4px inset spreads its hit area just past
     the drawn box. */
  .check__input {
    position: absolute;
    inset: -4px;
    margin: 0;
    opacity: 0;
    cursor: pointer;
  }

  .check__box {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 16px;
    height: 16px;
    /* WHY square + hairline: the Discord tickbox staging in Ledger neutrals
       — a quiet sunken rest that washes on hover and spends accent only at
       focus and in the checked state (research 0006 pattern 6, 0021 frame). */
    background: var(--bg-sunken);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    transition: background-color var(--dur-fast) var(--ease-out),
      border-color var(--dur-fast) var(--ease-out),
      box-shadow var(--dur-fast) var(--ease-out);
  }

  .check__label:hover .check__box {
    background-color: var(--bg-hover);
  }

  /* WHY the same focus treatment as text inputs: one glow grammar for every
     control in a dialog (research 0021). :focus-visible so mouse toggles
     never paint the ring. */
  .check__input:focus-visible + .check__box {
    outline: none;
    border-color: var(--accent);
    box-shadow: var(--ring-glow);
  }

  .check__input:checked + .check__box {
    background: var(--accent);
    border-color: var(--accent);
  }

  .check__label:hover .check__input:checked + .check__box {
    background: var(--accent-hover);
    border-color: var(--accent-hover);
  }

  .check__tick {
    width: 11px;
    height: 11px;
    stroke: var(--on-accent);
    stroke-width: 3;
    stroke-linecap: round;
    stroke-linejoin: round;
    opacity: 0;
    transform: scale(0.5);
    /* WHY ease-out, transform-only: the tick draw is a settled announcement,
       not an interruptible arrival, so it keeps the plain ease-out while
       chevrons and menus take the spring (ADR-0034). The global
       reduced-motion rule collapses it to an instant flip. */
    transition: opacity var(--dur-fast) var(--ease-out),
      transform var(--dur-fast) var(--ease-out);
  }

  .check__input:checked + .check__box .check__tick {
    opacity: 1;
    transform: scale(1);
  }

  .check__text {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .check__title {
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--text);
  }

  .check__hint {
    font-size: var(--text-xs);
    line-height: var(--leading-tight);
    color: var(--text-muted);
  }
</style>
