<script lang="ts">
  import type { Clip } from "$lib/types";
  import { createClip, updateClip } from "$lib/api";
  import { clipTitle } from "$lib/format";
  import Dialog from "./Dialog.svelte";
  import Button from "./Button.svelte";
  import Checkbox from "./Checkbox.svelte";
  import TextInput from "./TextInput.svelte";

  let {
    open,
    clip,
    onsave,
    oncancel,
  }: {
    open: boolean;
    /** The clip being edited; null = adding a new clip. */
    clip: Clip | null;
    onsave: (message: string) => void | Promise<void>;
    oncancel: () => void;
  } = $props();

  let name = $state("");
  let content = $state("");
  let showInDock = $state(true);
  let saving = $state(false);
  let error = $state("");

  const editing = $derived(clip !== null);

  $effect(() => {
    if (open) {
      name = clip?.name ?? "";
      content = clip?.content ?? "";
      showInDock = clip?.show_in_dock ?? true;
      saving = false;
      error = "";
    }
  });

  /** The optional-name field previews the name an untitled save would get —
   *  the content's first non-blank line (ticket 78). */
  const derivedTitle = $derived(clipTitle(name, content));

  async function submit() {
    if (!content.trim()) {
      error = "clip text can't be empty";
      return;
    }
    saving = true;
    error = "";
    try {
      if (editing && clip) {
        await updateClip({ ...clip, name: name.trim(), content: content.trim(), show_in_dock: showInDock });
        await onsave("Clip saved.");
      } else {
        const created = await createClip({
          name: name.trim(),
          content: content.trim(),
          show_in_dock: showInDock,
        });
        await onsave(`"${clipTitle(created.name, created.content)}" added to Quick Clips.`);
      }
    } catch (e) {
      console.error(e);
      error = String(e);
    } finally {
      saving = false;
    }
  }
</script>

<Dialog
  {open}
  title={editing ? "Edit clip" : "Add a clip"}
  onclose={oncancel}
  width={560}
  focusTarget="#clip-content"
>
  <form
    class="form"
    onsubmit={(e) => {
      e.preventDefault();
      submit();
    }}
  >
    <div class="field">
      <div class="field__label-row">
        <label class="field__label" for="clip-content">Text</label>
      </div>
      <textarea
        id="clip-content"
        class="field__text"
        rows="6"
        placeholder="Paste the text to keep…"
        autocomplete="off"
        spellcheck="false"
        value={content}
        oninput={(e) => (content = (e.target as HTMLTextAreaElement).value)}
      ></textarea>
    </div>

    <TextInput
      id="clip-name"
      label="Name"
      placeholder={derivedTitle ? `Untitled clips show “${derivedTitle}”` : "Optional — untitled clips show their first line"}
      value={name}
      onchange={(v) => (name = v)}
      info="How naming works"
    >
      {#snippet infobody()}
        <p>
          Optional. An unnamed clip is listed by its first line, so you never
          have to invent a name.
        </p>
      {/snippet}
    </TextInput>

    <!-- Short dock consequence stays inline like Command's dock flag
         (research 0023) — same shared Checkbox, same one-line voice. -->
    <Checkbox
      checked={showInDock}
      onchange={(v) => (showInDock = v)}
      title="Show in dock"
      hint="Uncheck to keep it in the main app only."
    />

    {#if error}
      <p class="form__error" role="alert">{error}</p>
    {/if}

    <div class="form__actions">
      <Button variant="secondary" onclick={oncancel} disabled={saving}>
        Cancel
      </Button>
      <Button kind="submit" disabled={saving}>
        {saving
          ? editing
            ? "Saving…"
            : "Adding…"
          : editing
            ? "Save changes"
            : "Add clip"}
      </Button>
    </div>
  </form>
</Dialog>

<style>
  .form {
    display: flex;
    flex-direction: column;
    /* Discord field rhythm in Ledger tokens (research 0021 spacing round). */
    gap: var(--space-5);
    min-width: 0;
  }

  .field {
    display: flex;
    flex-direction: column;
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

  .field__text {
    width: 100%;
    resize: vertical;
    min-height: 96px;
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    line-height: var(--leading-normal);
    color: var(--text);
    /* The shared filled frame (research 0021, ticket 204). */
    background: var(--bg-sunken);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    padding: 8px 10px;
    /* WHY three properties: the shared progressive input frame — quiet rest,
       hover wash, glowing focus (research 0020 enhancement). */
    transition: border-color var(--dur-fast) var(--ease-out),
      background-color var(--dur-fast) var(--ease-out),
      box-shadow var(--dur-fast) var(--ease-out);
  }

  .field__text:hover {
    background-color: var(--bg-hover);
    border-color: var(--border-strong);
  }

  .field__text:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: var(--ring-glow);
  }

  .field__text::placeholder {
    color: var(--text-muted);
    opacity: 0.75;
  }

  .form__error {
    margin: 0;
    font-size: var(--text-sm);
    color: var(--danger-text);
    overflow-wrap: anywhere;
  }

  .form__actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
    margin-top: var(--space-2);
  }
</style>
