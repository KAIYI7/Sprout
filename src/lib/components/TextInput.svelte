<script lang="ts">
  import type { Snippet } from "svelte";
  import type { HTMLInputAttributes } from "svelte/elements";
  import InfoTip from "./InfoTip.svelte";

  type Rest = Omit<HTMLInputAttributes, "value" | "onchange" | "oninput" | "placeholder">;

  let {
    label,
    id,
    value,
    placeholder,
    required = false,
    onchange,
    hint,
    autofocus = false,
    info,
    infobody,
    infotone = "info",
    ...rest
  }: {
    label: string;
    id: string;
    value: string;
    placeholder?: string;
    required?: boolean;
    onchange?: (v: string) => void;
    hint?: string;
    autofocus?: boolean;
    /** When set, an InfoTip trigger sits beside the label (hint text moved
     * out of the flow — ticket 45). */
    info?: string;
    infobody?: Snippet;
    infotone?: "info" | "warn";
  } & Rest = $props();

  let input: HTMLInputElement | undefined = $state();

  $effect(() => {
    if (autofocus) input?.focus();
  });
</script>

<div class="field">
  <div class="field__label-row">
    <label class="field__label" for={id}>
      {label}{#if required}<span class="field__req" aria-hidden="true">*</span>{/if}
    </label>
    {#if info}
      <InfoTip label={info} tone={infotone}>{@render infobody?.()}</InfoTip>
    {/if}
  </div>
  <input
    bind:this={input}
    {id}
    name={id}
    class="field__input"
    type="text"
    autocomplete="off"
    value={value}
    {placeholder}
    {required}
    oninput={(e) => onchange?.((e.target as HTMLInputElement).value)}
    {...rest}
  />
  {#if hint}<p class="field__hint">{hint}</p>{/if}
</div>

<style>
  .field {
    display: flex;
    flex-direction: column;
    /* Discord field rhythm in Ledger tokens (research 0021 spacing round):
       8px label → control → hint inside the field; the 24px form stack gap
       owns field → field. */
    gap: var(--space-2);
    min-width: 0;
  }

  .field__label-row {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    min-width: 0;
  }

  .field__label {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    font-weight: 500;
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .field__req {
    color: var(--accent);
    margin-left: 2px;
  }

  .field__input {
    width: 100%;
    font-family: var(--font-body);
    font-size: var(--text-base);
    color: var(--text);
    /* The shared filled frame (research 0021, ticket 204): a sunken rest
       with a hairline edge — the Discord staging in Ledger neutrals —
       instead of the old page-fill/strong-border outer line. */
    background: var(--bg-sunken);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    padding: 8px 10px;
    /* WHY three properties: the frame rests quiet, washes faintly on hover
       (anticipation), and glows on focus (commitment) — the Discord
       focus-glow in Ledger neutrals, accent spent at focus only (research
       0006 pattern 6, 0004 rule 2). Paint-only; layout never animates. */
    transition: border-color var(--dur-fast) var(--ease-out),
      background-color var(--dur-fast) var(--ease-out),
      box-shadow var(--dur-fast) var(--ease-out);
  }

  .field__input:hover {
    background-color: var(--bg-hover);
    border-color: var(--border-strong);
  }

  .field__input::placeholder {
    color: var(--text-muted);
    opacity: 0.75;
  }

  .field__input:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: var(--ring-glow);
  }

  .field__hint {
    margin: 0;
    font-size: var(--text-xs);
    line-height: var(--leading-tight);
    color: var(--text-muted);
  }
</style>
