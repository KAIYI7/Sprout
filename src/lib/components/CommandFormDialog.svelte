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
  import { t } from "$lib/copy";

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
      error = t("commandform.nameFirst");
      return;
    }
    if (!command.trim()) {
      error = t("commandform.cmdEmpty");
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
      await onsave(t("launch.addedFlash").replace("{name}", name.trim()));
    } catch (e) {
      console.error(e);
      error = String(e);
    } finally {
      saving = false;
    }
  }
</script>

<Dialog {open} title={t("commandform.addTitle")} onclose={oncancel} width={560} focusTarget="#command-name">
  <form
    class="form"
    onsubmit={(e) => {
      e.preventDefault();
      submit();
    }}
  >
    <TextInput
      id="command-name"
      label={t("common.name")}
      required
      placeholder={t("commandform.namePh")}
      value={name}
      onchange={onNameInput}
    />

    <div class="field">
      <div class="field__label-row">
        <label class="field__label" for="command-shell">{t("qdetails.shell")}</label>
        <InfoTip label={t("commandform.shellHow")}>
          <p>{t("commandform.shellBody")}</p>
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
      <label class="field__label" for="command-line">{t("qdetails.command")}</label>
      <textarea
        id="command-line"
        name="command"
        class="field__cmd"
        rows="3"
        placeholder={shell === "none" ? t("commandform.cmdPhNone") : shell === "powershell" ? t("commandform.cmdPhPowershell") : t("commandform.cmdPhCmd")}
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
        label={t("actionform.details")}
        onclick={() => (detailsOpen = !detailsOpen)}
      />
      <div id="command-details-body" class="advanced__body" hidden={!detailsOpen}>
        <Checkbox
          checked={showWindow}
          onchange={(v) => (showWindow = v)}
          title={t("commandform.showWindow")}
          hint={t("commandform.showWindowHint")}
        />

        <Checkbox
          checked={showInDock}
          onchange={(v) => (showInDock = v)}
          title={t("common.showInDock")}
          hint={t("common.showInDockHint")}
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
        {t("common.cancel")}
      </Button>
      <Button kind="submit" disabled={saving || testing}>
        {saving ? t("common.busy.adding") : t("commandform.addBtn")}
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
