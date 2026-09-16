<script lang="ts">
  import type { LaunchShell } from "$lib/types";
  import { launchShellLabel } from "$lib/types";
  import { createLaunchEntry, testLaunchCommand } from "$lib/api";
  import Dialog from "./Dialog.svelte";
  import Button from "./Button.svelte";
  import Checkbox from "./Checkbox.svelte";
  import TextInput from "./TextInput.svelte";
  import Select from "./Select.svelte";
  import TestResult from "./TestResult.svelte";
  import Disclosure from "./Disclosure.svelte";
  import InfoTip from "./InfoTip.svelte";

  let {
    open,
    onsave,
    oncancel,
  }: {
    open: boolean;
    onsave: (message: string) => void | Promise<void>;
    oncancel: () => void;
  } = $props();

  let name = $state("");
  let shell = $state<LaunchShell>("powershell");
  let command = $state("");
  let showWindow = $state(false);
  let showInDock = $state(true);
  let saving = $state(false);
  let error = $state("");
  let testing = $state(false);
  let detailsOpen = $state(false);

  // The name follows the command until the user edits it by hand.
  let nameAuto = $state(true);

  $effect(() => {
    if (open) {
      name = "";
      shell = "powershell";
      command = "";
      showWindow = false;
      showInDock = true;
      saving = false;
      error = "";
      nameAuto = true;
      detailsOpen = false;
    }
  });

  /** The entry's name suggestion from the command: the first token (quotes
   * honored), its path basename, without a script/exe extension. */
  function nameFromCommand(value: string): string {
    const trimmed = value.trim();
    if (!trimmed) return "";
    let token = "";
    let inQuotes = false;
    for (const ch of trimmed) {
      if (ch === '"') inQuotes = !inQuotes;
      else if (!inQuotes && /\s/.test(ch)) break;
      else token += ch;
    }
    const base = token.split(/[\\/]/).pop() ?? token;
    return base.replace(/\.(exe|cmd|bat|ps1)$/i, "").trim();
  }

  function onCommandInput(value: string) {
    command = value;
    if (nameAuto) name = nameFromCommand(value);
  }

  function onNameInput(value: string) {
    name = value;
    nameAuto = false;
  }

  async function submit() {
    if (!name.trim()) {
      error = "Give the entry a name.";
      return;
    }
    if (!command.trim()) {
      error = "The command must not be empty.";
      return;
    }
    saving = true;
    error = "";
    try {
      await createLaunchEntry({
        name: name.trim(),
        kind: "command",
        target: command.trim(),
        shell,
        show_window: showWindow,
        desktop_id: null,
        show_in_dock: showInDock,
      });
      await onsave(`${name.trim()} added to Quick Launch.`);
    } catch (e) {
      console.error(e);
      error = String(e);
    } finally {
      saving = false;
    }
  }
</script>

<Dialog {open} title="Add a command" onclose={oncancel} width={560} focusTarget="#command-name">
  <form
    class="form"
    onsubmit={(e) => {
      e.preventDefault();
      submit();
    }}
  >
    <TextInput
      id="command-name"
      label="Name"
      required
      placeholder="e.g. dev server…"
      value={name}
      onchange={onNameInput}
    />

    <div class="field">
      <div class="field__label-row">
        <label class="field__label" for="command-shell">Shell</label>
        <InfoTip label="How the shell works">
          <p>PowerShell runs scripts in Windows PowerShell 5.1. CMD uses the Windows command shell. Direct exe starts an executable without expanding shell variables.</p>
        </InfoTip>
      </div>
      <Select
        id="command-shell"
        value={shell}
        onchange={(v) => (shell = v as LaunchShell)}
        options={[
          { value: "powershell", label: launchShellLabel.powershell },
          { value: "cmd", label: launchShellLabel.cmd },
          { value: "none", label: launchShellLabel.none },
        ]}
      />
    </div>

    <div class="field">
      <label class="field__label" for="command-line">Command</label>
      <textarea
        id="command-line"
        name="command"
        class="field__cmd"
        rows="3"
        placeholder={shell === "none" ? 'e.g. C:\\Tools\\dev-server.exe --port 8080…' : shell === "powershell" ? 'e.g. Start-Process notepad.exe…' : 'e.g. start "" http://localhost:3000…'}
        autocomplete="off"
        spellcheck="false"
        value={command}
        oninput={(e) => onCommandInput((e.target as HTMLTextAreaElement).value)}
      ></textarea>
    </div>

    <div class="advanced">
      <Disclosure
        open={detailsOpen}
        controls="command-details-body"
        label="Details"
        onclick={() => (detailsOpen = !detailsOpen)}
      />
      <div id="command-details-body" class="advanced__body" hidden={!detailsOpen}>
        <Checkbox
          checked={showWindow}
          onchange={(v) => (showWindow = v)}
          title="Show a window"
          hint="Commands run without a console window by default."
        />

        <Checkbox
          checked={showInDock}
          onchange={(v) => (showInDock = v)}
          title="Show in dock"
          hint="Uncheck to keep it in the main app only."
        />

        <TestResult
          {open}
          {command}
          bind:testing
          probe={() => testLaunchCommand(shell, command.trim())}
          onerror={(message) => (error = message)}
        />
      </div>
    </div>

    {#if error}
      <p class="form__error" role="alert">{error}</p>
    {/if}

    <div class="form__actions">
      <Button variant="secondary" onclick={oncancel} disabled={saving || testing}>
        Cancel
      </Button>
      <Button kind="submit" disabled={saving || testing}>
        {saving ? "Adding…" : "Add command"}
      </Button>
    </div>
  </form>
</Dialog>

<style>
  .form {
    display: flex;
    flex-direction: column;
    /* Discord field rhythm in Ledger tokens (research 0021 spacing round):
       24px field → field; 8px owns label → control → hint inside a field. */
    gap: var(--space-5);
    min-width: 0;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
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

  .field__label-row {
    display: flex;
    align-items: center;
    gap: var(--space-1);
  }

  .advanced {
    display: flex;
    flex-direction: column;
    /* Section standoff (research 0021 spacing round): the Details chevron
       stands off the previous field even when closed. */
    margin-top: var(--space-1);
  }

  .advanced__body {
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
    padding-top: var(--space-3);
    border-top: 1px dashed var(--border);
  }

  .advanced__body[hidden] {
    display: none;
  }

  .field__cmd {
    width: 100%;
    resize: vertical;
    min-height: 64px;
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

  .field__cmd:hover {
    background-color: var(--bg-hover);
    border-color: var(--border-strong);
  }

  .field__cmd:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: var(--ring-glow);
  }

  .field__cmd::placeholder {
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
