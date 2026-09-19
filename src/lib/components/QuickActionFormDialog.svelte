<script lang="ts">
import { tick, untrack } from "svelte";
import type { Group, LaunchCommandTest, QuickAction, QuickActionShell } from "$lib/types";
  import type {
    AiApprovedRoot,
    AiBoundTarget,
    AiDiscoveryScope,
    AiFileContent,
    AiFindOutcome,
  } from "$lib/types";
  import { quickActionShellLabel } from "$lib/types";
  import type { AiDiagnoseOutcome, AiDraftOutcome } from "$lib/types";
  import {
    aiApproveDisclosure,
    aiApproveRoot,
    aiBindTarget,
    aiCancelDraft,
    aiCheckCandidate,
    aiDiagnoseDraft,
    aiFindTargets,
    aiGenerateDraft,
    aiListApprovedRoots,
    aiReadTargetFile,
    aiRevokeRoot,
    aiVerifyRevision,
    assignToGroup,
    attachQuickActionFile,
    createGroup,
    createQuickAction,
    detectPrerequisites,
    formatActionFileBytes,
    listQuickActionFiles,
    QUICK_ACTION_FILE_MAX_BYTES,
    QUICK_ACTION_FILES_MAX_BYTES,
    removeQuickActionFile,
    testQuickAction,
    unassignFromGroup,
    updateQuickAction,
  } from "$lib/api";
  import {
    applyCompletion,
    filesCompletion,
    filesHint,
    tokenizeQuickActionCommand,
  } from "$lib/quickActionEditor";
  import {
    extractPrereqKeys,
    prereqLines as toPrereqLines,
    type PrereqLine,
  } from "$lib/prereqGuidance";
  import { detectCmdStartTitleTrap } from "$lib/quickActionFiles";
  import { downloadFilesZip, downloadSingleFile } from "$lib/quickActionDownload";
  import { open as openFolderPicker } from "@tauri-apps/plugin-dialog";
  import Dialog from "./Dialog.svelte";
  import Button from "./Button.svelte";
  import Checkbox from "./Checkbox.svelte";
  import TextInput from "./TextInput.svelte";
  import Disclosure from "./Disclosure.svelte";
  import InfoTip from "./InfoTip.svelte";
  import Notice from "./Notice.svelte";
  import Select from "./Select.svelte";
  import TestResult from "./TestResult.svelte";
  import { t } from "$lib/copy";

  let {
    open,
    action,
    groups = [],
    groupsEnabled = false,
    aiReady = false,
    aiSetupKind = "generic",
    onsave,
    oncancel,
    onsetupai,
  }: {
    open: boolean;
    /** The action being edited; null = adding a new action. */
    action: QuickAction | null;
    /** Live `action`-collection groups (ticket 131): empty = no group field
     *  at all (research 0004 rule 2, 0006 patterns 2/11); otherwise an
     *  optional picker defaulting to ungrouped, with a create-and-place
     *  `New group…` entry (0006 pattern 10). */
    groups?: Group[];
    /** The collection's Groups switch (ticket 89): off is fully dormant —
     *  stored groups are never shown or touched, so the picker stays absent
     *  even while groups exist (0006 pattern 12). */
    groupsEnabled?: boolean;
    /** Whether AI drafting is configured and ready: the dialog gate for the
     *  drafting block. Optional assistance stays invisible until deliberately
     *  set up in Settings (ADR-0031), so an unready route renders zero AI
     *  chrome instead of a disabled teaser. The backend remains the
     *  generation gate; this is disclosure only. */
    aiReady?: boolean;
    /** Which unready pointer to show (ticket 192): `managed` names the
     *  stopped managed route, anything else offers generic setup. Rendered
     *  only while unready — at most one plain-text line, never tabs or hero. */
    aiSetupKind?: "managed" | "generic";
    onsave: (message: string) => void | Promise<void>;
    oncancel: () => void;
    /** Ticket 192: routes the unready pointer to Settings (expand the AI
     *  group, focus the provider control). The page owns navigation; the
     *  dialog only calls back. */
    onsetupai?: () => void | Promise<void>;
  } = $props();

  let name = $state("");
  let shell = $state<QuickActionShell>("powershell");
  let command = $state("");
  let cwd = $state("");
  let note = $state("");
  let stoppable = $state(false);
  let stopCommand = $state("");
  let autoRun = $state(false);
  let showInDock = $state(true);
  let detailsOpen = $state(false);
  // Pre-action gate (ADR-0017): an optional check that runs first on every
  // Run under this shell and directory, plus the fix offered only when the
  // check blocks a run. Both ride behind Details — a second disclosure level
  // for a rarely-needed gate (research 0004 rules 2–3) — and both trim to
  // null on save, so an untouched section persists no trace.
  let preCheck = $state("");
  let preFix = $state("");
  let preActionOpen = $state(false);
  // The [Check] probe below: the same timeboxed Test path over the check
  // text only — the action and the fix never run here.
  let checking = $state(false);
  let checkRan = $state(false);
  let checkResult = $state<LaunchCommandTest | null>(null);
  let saving = $state(false);
  let error = $state("");
  let testing = $state(false);
  // AI drafting (ADR-0030): an explicit-shell request becomes a reviewable
  // candidate the user applies into Manual and saves normally. Generation
  // never runs, tests, or stops anything — this section never calls the
  // run/test controls. `aiSeq` drops late completions so a slow answer can
  // never replace a newer draft or claim a cancelled success. It is
  // deliberately NOT reactive state: nothing renders it, and a
  // read-modify-write inside the reset effect below would retrigger that
  // effect forever.
  let aiView = $state<"ai" | "manual">(aiReady && action === null ? "ai" : "manual");
  let aiRequest = $state("");
  let aiPending = $state(false);
  let aiSeq = 0;
  let aiRequestId: string | null = null;
  let aiOutcome = $state<AiDraftOutcome | null>(null);
  let aiNotice = $state("");
  let aiApplied = $state<{ shell: QuickActionShell; command: string } | null>(null);
  let clarifyPick = $state("");
  let clarifyFree = $state("");
  // The grill chain (ADR-0032): answers accumulate onto the previous narrowed
  // request so no round loses earlier context; `chainedFrom` guards the chain
  // against a request edited mid-grill, and `clarifyRound` counts answers for
  // the question indicator.
  let lastNarrowed = $state<string | null>(null);
  let chainedFrom = $state("");
  let clarifyRound = $state(0);
  let manualNotice = $state("");
  // The draft under review, if any — flat derivations keep the review
  // markup free of inline declarations and narrowing chains.
  const aiDraft = $derived(aiOutcome?.kind === "draft" ? aiOutcome.draft : null);
  const aiClarify = $derived(aiOutcome?.kind === "clarify" ? aiOutcome : null);
  const aiClarifyChoices = $derived(aiClarify?.choices ?? []);
  const aiClarifyAspect = $derived(
    aiClarify && typeof aiClarify.aspect === "string" && aiClarify.aspect
      ? aiClarify.aspect
      : null
  );
  const aiRefusal = $derived(aiOutcome?.kind === "refused" ? aiOutcome.message : null);
  const aiFailure = $derived(aiOutcome?.kind === "failed" ? aiOutcome.message : null);
  // Prerequisite surfacing (ticket 200): the dialog verifies what the draft
  // assumes through the read-only detect command and warns beside the draft —
  // saving, testing, and running stay untouched. `prereqSeq` drops late
  // verdicts the same way `aiSeq` drops late drafts.
  let prereqRows = $state<PrereqLine[]>([]);
  let prereqsChecking = $state(false);
  let prereqSeq = 0;

  $effect(() => {
    const outcome = aiOutcome;
    // WHY untrack: the request text is a snapshot of what produced this
    // outcome — subscribing to it would re-detect on every keystroke.
    const requestText = untrack(() => aiRequest);
    const narrowed = untrack(() => lastNarrowed);
    prereqSeq += 1;
    const mine = prereqSeq;
    prereqRows = [];
    prereqsChecking = false;
    if (!outcome) return;
    const texts =
      outcome.kind === "draft"
        ? outcome.draft.assumptions
        : outcome.kind === "clarify" && aiClarifyAspect === "unknown-prerequisite"
          ? [requestText, narrowed ?? ""]
          : [];
    const keys = extractPrereqKeys(texts);
    if (keys.length === 0) return;
    prereqsChecking = true;
    void detectPrerequisites(keys).then(
      (verdicts) => {
        if (mine !== prereqSeq) return;
        prereqRows = toPrereqLines(verdicts);
        prereqsChecking = false;
      },
      () => {
        if (mine !== prereqSeq) return;
        prereqsChecking = false;
      }
    );
  });
  const aiAppliedCurrent = $derived(
    aiDraft !== null &&
      aiApplied !== null &&
      aiApplied.shell === shell &&
      aiApplied.command === command.trim()
  );
  /** Group picker selection: "" = ungrouped, a group id, or NEW_GROUP. */
  let groupPick = $state("");
  let newGroupName = $state("");

  const NEW_GROUP = "__new__";

  const editing = $derived(action !== null);

  $effect(() => {
    if (open) {
      if (aiRequestId) void aiCancelDraft(aiRequestId).catch(() => {});
      aiRequestId = null;
      if (diagRequestId) void aiCancelDraft(diagRequestId).catch(() => {});
      diagRequestId = null;
      name = action?.name ?? "";
      shell = action?.shell ?? "powershell";
      command = action?.command ?? "";
      cwd = action?.cwd ?? "";
      note = action?.note ?? "";
      stoppable = action?.stoppable ?? false;
      stopCommand = action?.stop_command ?? "";
      autoRun = action?.auto_run ?? false;
      showInDock = action?.show_in_dock ?? true;
      detailsOpen = false;
      preCheck = action?.pre_check ?? "";
      preFix = action?.pre_fix ?? "";
      preActionOpen = false;
      checking = false;
      checkRan = false;
      checkResult = null;
      saving = false;
      error = "";
      // AI-first on Add, manual-first on Edit: a new action starts from the
      // drafting view when one is available, while an edit keeps the
      // author's fields in front (ADR-0030 keeps applied drafts reviewable
      // either way — generation never fills anything unreviewed).
      aiView = aiReady && action === null ? "ai" : "manual";
      aiRequest = "";
      aiPending = false;
      aiSeq += 1;
      aiOutcome = null;
      aiNotice = "";
      aiApplied = null;
      clarifyPick = "";
      clarifyFree = "";
      manualNotice = "";
      diagError = "";
      diagPending = false;
      diagSeq += 1;
      diagRequestId = null;
      diagOutcome = null;
      diagNotice = "";
      diagConflict = "";
      diagAppliedBaseline = null;
      findQuery = "";
      findScope = "apps";
      findPending = false;
      findSeq += 1;
      findOutcome = null;
      findPick = "";
      findNotice = "";
      roots = [];
      rootsNotice = "";
      boundTarget = null;
      filePreview = null;
      // Default ungrouped; an edit preselects its current group when that
      // group is still live.
      groupPick =
        action?.group_id != null &&
        groups.some((g) => g.id === action.group_id)
          ? String(action.group_id)
          : "";
      newGroupName = "";
      // Files start empty every open: an edit reloads them below, an add
      // stages picks in memory until the action exists. The editor's popup
      // state resets likewise so no stale suggestion survives reopening.
      fileRows = [];
      filesError = "";
      filesBusy = false;
      filesAnnouncement = "";
      suggestCaret = 0;
      suggestActive = 0;
      suggestClosed = false;
      if (action) void loadFiles(action.id);
    }
  });

  /** The working-directory rule, mirroring the backend (ticket 50): when set,
   *  it must be an absolute path — a relative one would silently mean
   *  different things per machine. */
  function cwdError(value: string): string | null {
    const trimmed = value.trim();
    if (!trimmed) return null;
    if (!/^[A-Za-z]:[\\/]/.test(trimmed) && !/^\\\\/.test(trimmed)) {
      return t("actionform.cwdError").replace("{path}", trimmed);
    }
    return null;
  }

  /** One [Check] click: runs only the check text under the action's shell
   *  and directory through the timeboxed Test path and reports it inline.
   *  The probe reads the check alone, so the main command and the fix can
   *  never run here — passing here still re-checks on Run. */
  async function runCheck() {
    if (checking) return;
    if (!preCheck.trim()) {
      error = t("actionform.preCheckFirst");
      return;
    }
    const badCwd = cwdError(cwd);
    if (badCwd) {
      error = badCwd;
      return;
    }
    error = "";
    checkRan = true;
    checking = true;
    checkResult = null;
    try {
      checkResult = await testQuickAction(shell, preCheck.trim(), cwd.trim() || null);
    } catch (e) {
      console.error(e);
      error = String(e);
      checkRan = false;
    } finally {
      checking = false;
    }
  }

  async function generateDraft() {
    if (aiPending) return;
    if (!aiRequest.trim()) {
      aiOutcome = null;
      aiNotice = t("actionform.describeFirst");
      return;
    }
    aiPending = true;
    aiOutcome = null;
    aiNotice = "";
    clarifyPick = "";
    clarifyFree = "";
    lastNarrowed = null;
    chainedFrom = "";
    clarifyRound = 0;
    error = "";
    const mine = ++aiSeq;
    const requestId = `draft-${Date.now()}-${mine}`;
    aiRequestId = requestId;
    try {
      const outcome = await aiGenerateDraft(aiRequest.trim(), shell, null, requestId);
      if (mine !== aiSeq) return;
      aiOutcome = outcome;
      clarifyPick = "";
      clarifyFree = "";
      await tick();
      document.getElementById("ai-outcome")?.focus();
    } catch (e) {
      console.error(e);
      if (mine !== aiSeq) return;
      aiOutcome = { kind: "failed", message: String(e) };
      await tick();
      document.getElementById("ai-outcome")?.focus();
    } finally {
      if (mine === aiSeq) {
        aiPending = false;
        aiRequestId = null;
      }
    }
  }

  async function regenerateWithClarification() {
    const selection = clarifyFree.trim() || clarifyPick.trim();
    if (!selection) {
      aiNotice = t("actionform.clarifyFirst");
      return;
    }
    // The chain carries every prior answer forward, and the aspect key tells
    // the backend which question this answers — answered questions never
    // repeat, distinct ones still ask (ADR-0032).
    const base =
      lastNarrowed && chainedFrom === aiRequest.trim() ? lastNarrowed : aiRequest.trim();
    const narrowed = aiClarifyAspect
      ? `${base} — clarified choice [${aiClarifyAspect}]: ${selection}`
      : `${base} — clarified choice: ${selection}`;
    lastNarrowed = narrowed;
    chainedFrom = aiRequest.trim();
    clarifyRound += 1;
    if (aiPending) return;
    aiPending = true;
    aiOutcome = null;
    aiNotice = "";
    error = "";
    const mine = ++aiSeq;
    const requestId = `draft-${Date.now()}-${mine}`;
    aiRequestId = requestId;
    try {
      const outcome = await aiGenerateDraft(narrowed, shell, null, requestId);
      if (mine !== aiSeq) return;
      aiOutcome = outcome;
      clarifyPick = "";
      clarifyFree = "";
      await tick();
      document.getElementById("ai-outcome")?.focus();
    } catch (e) {
      console.error(e);
      if (mine !== aiSeq) return;
      aiOutcome = { kind: "failed", message: String(e) };
      await tick();
      document.getElementById("ai-outcome")?.focus();
    } finally {
      if (mine === aiSeq) {
        aiPending = false;
        aiRequestId = null;
      }
    }
  }

  async function draftAnyway() {
    // The explicit user override ends the grill: the chained context goes back
    // with the override tag, so vagueness is skipped while refusal and
    // shell-compatibility still guard the draft (ADR-0032).
    const base =
      lastNarrowed && chainedFrom === aiRequest.trim() ? lastNarrowed : aiRequest.trim();
    if (!base) {
      aiNotice = t("actionform.describeFirst");
      return;
    }
    if (aiPending) return;
    aiPending = true;
    aiOutcome = null;
    aiNotice = "";
    error = "";
    const mine = ++aiSeq;
    const requestId = `draft-${Date.now()}-${mine}`;
    aiRequestId = requestId;
    try {
      const outcome = await aiGenerateDraft(
        `${base} — draft-anyway: proceed with what you have`,
        shell,
        null,
        requestId
      );
      if (mine !== aiSeq) return;
      aiOutcome = outcome;
      clarifyPick = "";
      clarifyFree = "";
      await tick();
      document.getElementById("ai-outcome")?.focus();
    } catch (e) {
      console.error(e);
      if (mine !== aiSeq) return;
      aiOutcome = { kind: "failed", message: String(e) };
      await tick();
      document.getElementById("ai-outcome")?.focus();
    } finally {
      if (mine === aiSeq) {
        aiPending = false;
        aiRequestId = null;
      }
    }
  }

  function clarifyKeydown(event: KeyboardEvent) {
    if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
      event.preventDefault();
      void regenerateWithClarification();
    }
  }

  function cancelDraft() {
    if (aiRequestId) void aiCancelDraft(aiRequestId).catch(() => {});
    aiRequestId = null;
    aiSeq += 1;
    aiPending = false;
    aiNotice = t("actionform.genCancelled");
  }

  async function applyDraft() {
    if (aiOutcome?.kind !== "draft") return;
    command = aiOutcome.draft.command;
    shell = aiOutcome.draft.shell;
    aiApplied = { shell: aiOutcome.draft.shell, command: aiOutcome.draft.command };
    aiNotice = "";
    error = "";
    manualNotice = t("actionform.appliedManual");
    aiView = "manual";
    await tick();
    document.getElementById("qa-command")?.focus();
  }

  function dismissDraft() {
    aiSeq += 1;
    aiOutcome = null;
    aiApplied = null;
    aiNotice = "";
    clarifyPick = "";
    clarifyFree = "";
  }

  // Diagnosis (ticket 153, ADR-0030): an edit-only block that sends the
  // selected saved script plus pasted error output and returns an
  // explanation or a separately reviewed revision. It never runs, tests, or
  // stops anything — this section calls only the diagnose/verify/cancel
  // commands. Rejecting, cancelling, failing, or regenerating leaves the
  // saved fields intact; only explicit acceptance applies the reviewed
  // fields (shell, command, working directory, note). `diagSeq` drops late
  // completions the same way `aiSeq` does for drafts.
  let diagError = $state("");
  let diagPending = $state(false);
  let diagSeq = 0;
  let diagRequestId: string | null = null;
  let diagOutcome = $state<AiDiagnoseOutcome | null>(null);
  let diagNotice = $state("");
  let diagConflict = $state("");
  // The accepted revision's baseline: submit() rechecks it against the live
  // row so source deletion or newer edits block the save with both states
  // retained, instead of overwriting silently.
  let diagAppliedBaseline = $state<string | null>(null);
  const diagRevision = $derived(diagOutcome?.kind === "revision" ? diagOutcome.revision : null);
  const diagBaseline = $derived(diagOutcome?.kind === "revision" ? diagOutcome.baseline : null);
  const diagExplanationOnly = $derived(
    diagOutcome?.kind === "explanation" ? diagOutcome.explanation : null
  );
  const diagRefusal = $derived(diagOutcome?.kind === "refused" ? diagOutcome.message : null);
  const diagFailure = $derived(diagOutcome?.kind === "failed" ? diagOutcome.message : null);
  const diagClarify = $derived(diagOutcome?.kind === "clarify" ? diagOutcome : null);

  async function diagnose() {
    if (diagPending || !action) return;
    if (!command.trim()) {
      diagOutcome = null;
      diagNotice = t("actionform.diagEmpty");
      return;
    }
    if (!diagError.trim()) {
      diagOutcome = null;
      diagNotice = t("actionform.diagPasteFirst");
      return;
    }
    diagPending = true;
    diagOutcome = null;
    diagNotice = "";
    diagConflict = "";
    error = "";
    const mine = ++diagSeq;
    const requestId = `diagnose-${Date.now()}-${mine}`;
    diagRequestId = requestId;
    try {
      const outcome = await aiDiagnoseDraft(
        action.id,
        command,
        diagError.trim(),
        shell,
        cwd.trim() || null,
        requestId
      );
      if (mine !== diagSeq) return;
      diagOutcome = outcome;
      await tick();
      document.getElementById("diag-outcome")?.focus();
    } catch (e) {
      console.error(e);
      if (mine !== diagSeq) return;
      diagOutcome = { kind: "failed", message: String(e) };
      await tick();
      document.getElementById("diag-outcome")?.focus();
    } finally {
      if (mine === diagSeq) {
        diagPending = false;
        diagRequestId = null;
      }
    }
  }

  function cancelDiagnose() {
    if (diagRequestId) void aiCancelDraft(diagRequestId).catch(() => {});
    diagRequestId = null;
    diagSeq += 1;
    diagPending = false;
    diagNotice = t("actionform.diagCancelled");
  }

  function diagKeydown(event: KeyboardEvent) {
    if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
      event.preventDefault();
      void diagnose();
    }
  }

  async function applyRevision() {
    if (diagOutcome?.kind !== "revision" || !action) return;
    // Acceptance rechecks the baseline first: a deleted source or a newer
    // edit keeps both states and blocks the apply instead of overwriting.
    try {
      const conflict = await aiVerifyRevision(action.id, diagOutcome.baseline);
      if (conflict) {
        diagConflict = conflict;
        return;
      }
    } catch (e) {
      console.error(e);
      diagConflict = String(e);
      return;
    }
    const revision = diagOutcome.revision;
    // Only reviewed fields move: shell, command, working directory, note.
    // Name, Group picker, stoppable settings, auto-run, dock visibility,
    // and pre-action gates stay exactly as the user left them.
    shell = revision.shell;
    command = revision.command;
    cwd = revision.cwd ?? "";
    if (revision.note !== null) note = revision.note;
    aiApplied = { shell: revision.shell, command: revision.command };
    diagAppliedBaseline = diagOutcome.baseline;
    diagConflict = "";
    diagNotice = "";
    error = "";
    manualNotice = t("actionform.revisionApplied");
    aiView = "manual";
    await tick();
    document.getElementById("qa-command")?.focus();
  }

  function dismissRevision() {
    diagSeq += 1;
    diagOutcome = null;
    diagNotice = "";
    diagConflict = "";
  }

  function selectView(view: "ai" | "manual") {
    aiView = view;
  }

  function viewTabsKeydown(event: KeyboardEvent) {
    if (event.key !== "ArrowRight" && event.key !== "ArrowLeft" && event.key !== "Home" && event.key !== "End")
      return;
    event.preventDefault();
    if (event.key === "Home") {
      selectView("ai");
      document.getElementById("tab-ai")?.focus();
      return;
    }
    if (event.key === "End") {
      selectView("manual");
      document.getElementById("tab-manual")?.focus();
      return;
    }
    selectView(aiView === "ai" ? "manual" : "ai");
    requestAnimationFrame(() =>
      document.getElementById(aiView === "ai" ? "tab-manual" : "tab-ai")?.focus(),
    );
  }

  // AI local-target search (ADR-0031): an explicit find over installed apps
  // and approved folders returns opaque references; the user picks a match
  // and trusted code binds it into the command below. Finding reads
  // names/paths only — contents need the separate preview, disclosure needs
  // its own approval, and nothing here runs. `findSeq` drops late answers
  // the same way `aiSeq` does for drafts.
  let findQuery = $state("");
  let findScope = $state<AiDiscoveryScope>("apps");
  let findPending = $state(false);
  let findSeq = 0;
  let findOutcome = $state<AiFindOutcome | null>(null);
  let findPick = $state("");
  let findNotice = $state("");
  let roots = $state<AiApprovedRoot[]>([]);
  let rootsNotice = $state("");
  let boundTarget = $state<AiBoundTarget | null>(null);
  let filePreview = $state<AiFileContent | null>(null);
  const findPicked = $derived(
    findOutcome?.matches.find((m) => m.ref_id === findPick) ?? null
  );

  async function loadRoots() {
    rootsNotice = "";
    try {
      roots = await aiListApprovedRoots();
    } catch (e) {
      console.error(e);
      rootsNotice = String(e);
    }
  }

  async function approveRoot() {
    rootsNotice = "";
    try {
      const picked = await openFolderPicker({ directory: true, multiple: false });
      if (typeof picked !== "string" || !picked) return;
      const root = await aiApproveRoot(picked);
      roots = [...roots.filter((r) => r.path !== root.path), root];
    } catch (e) {
      console.error(e);
      rootsNotice = String(e);
    }
  }

  async function revokeRoot(path: string) {
    rootsNotice = "";
    try {
      await aiRevokeRoot(path);
      roots = roots.filter((r) => r.path !== path);
    } catch (e) {
      console.error(e);
      rootsNotice = String(e);
    }
  }

  async function runFind() {
    if (findPending) return;
    if (!findQuery.trim()) {
      findNotice = t("actionform.findFirst");
      return;
    }
    findPending = true;
    findNotice = "";
    filePreview = null;
    const mine = ++findSeq;
    try {
      const outcome = await aiFindTargets(findQuery.trim(), findScope);
      if (mine !== findSeq) return;
      findOutcome = outcome;
      findPick = outcome.matches.length === 1 ? outcome.matches[0].ref_id : "";
      if (outcome.matches.length === 0) {
        findNotice = outcome.notice ?? t("actionform.noMatch");
      }
    } catch (e) {
      console.error(e);
      if (mine !== findSeq) return;
      findNotice = String(e);
    } finally {
      if (mine === findSeq) findPending = false;
    }
  }

  function cancelFind() {
    findSeq += 1;
    findPending = false;
    findNotice = t("actionform.findCancelled");
  }

  async function useTarget() {
    const pick = findOutcome?.matches.find((m) => m.ref_id === findPick);
    if (!pick) {
      findNotice = t("actionform.pickFirst");
      return;
    }
    try {
      const bound = await aiBindTarget(pick.ref_id, shell);
      command = bound.command;
      shell = bound.shell;
      boundTarget = bound;
      error = "";
      findNotice = "";
    } catch (e) {
      console.error(e);
      findNotice = String(e);
    }
  }

  function dismissBound() {
    boundTarget = null;
  }

  async function previewFile(refId: string) {
    filePreview = null;
    findNotice = "";
    try {
      filePreview = await aiReadTargetFile(refId);
    } catch (e) {
      console.error(e);
      findNotice = String(e);
    }
  }

  async function approvePreviewDisclosure() {
    if (!filePreview) return;
    try {
      await aiApproveDisclosure(filePreview.ref_id, ["name", "path", "contents"]);
      findNotice = t("actionform.disclosureRecorded");
    } catch (e) {
      console.error(e);
      findNotice = String(e);
    }
  }

  function aiKeydown(event: KeyboardEvent) {
    // Ctrl/Cmd+Enter generates here instead of submitting the whole dialog —
    // preventing default keeps the shared Dialog handler's precedence rule.
    if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
      event.preventDefault();
      void generateDraft();
    }
  }

  async function submit() {
    if (!name.trim()) {
      error = t("actionform.nameFirst");
      return;
    }
    if (!command.trim()) {
      error = t("commandform.cmdEmpty");
      return;
    }
    const badCwd = cwdError(cwd);
    if (badCwd) {
      error = badCwd;
      return;
    }
    // A fix without a check has no trigger — it would sit in storage with
    // no path that ever runs it. Refused plainly, with the backend's own
    // wording, before anything persists.
    const trimmedPreCheck = preCheck.trim() || null;
    const trimmedPreFix = preFix.trim() || null;
    if (!trimmedPreCheck && trimmedPreFix) {
      error = t("actionform.fixNeedsCheck");
      return;
    }
    // Content applied from a draft and then edited is rechecked before it
    // may save on AI authority: a refusal or clarification blocks with an
    // escape hatch to manual saving, never a silent pass. Unchanged applied
    // text was already checked at generation; purely manual text skips this.
    if (
      aiApplied &&
      (shell !== aiApplied.shell || command.trim() !== aiApplied.command)
    ) {
      try {
        const verdict = await aiCheckCandidate(shell, command.trim());
        if (verdict.verdict !== "allow") {
          error = `${verdict.message}${t("actionform.recheckSuffix")}`;
          return;
        }
      } catch (e) {
        console.error(e);
      }
    }
    // A revision applied from diagnosis is rechecked against the live row
    // before it may save: source deletion or a newer edit blocks with both
    // states retained instead of overwriting silently. A revision the user
    // then edited is their own text — the recheck hatch above already
    // covered it, so only untouched applies verify here.
    if (
      diagAppliedBaseline &&
      editing &&
      action &&
      aiApplied &&
      shell === aiApplied.shell &&
      command.trim() === aiApplied.command
    ) {
      try {
        const conflict = await aiVerifyRevision(action.id, diagAppliedBaseline);
        if (conflict) {
          error = conflict;
          aiView = "ai";
          await tick();
          document.getElementById("diag-outcome")?.focus();
          return;
        }
      } catch (e) {
        console.error(e);
      }
    }
    // Group placement (ticket 131): the picker exists only while the Groups
    // switch is on and live groups do, and the New-group entry needs a name
    // — validated inline like the GroupNameDialog's, before anything is
    // created.
    const placing = groupsEnabled && groups.length > 0;
    const creatingGroup = placing && groupPick === NEW_GROUP;
    const trimmedGroupName = newGroupName.trim();
    if (creatingGroup && !trimmedGroupName) {
      error = t("actionform.groupNameFirst");
      return;
    }
    saving = true;
    error = "";
    try {
      const trimmedNote = note.trim() || null;
      if (editing && action) {
        await updateQuickAction({
          ...action,
          name: name.trim(),
          shell,
          command: command.trim(),
          cwd: cwd.trim() || null,
          stoppable,
          stop_command: stoppable ? stopCommand.trim() || null : null,
          note: trimmedNote,
          auto_run: autoRun,
          show_in_dock: showInDock,
          pre_check: trimmedPreCheck,
          pre_fix: trimmedPreFix,
        });
        // Group membership rides outside the edit payload (ticket 89) — the
        // same assign/unassign the row menu uses.
        if (placing) {
          if (creatingGroup) {
            const created = await createGroup("action", trimmedGroupName);
            await assignToGroup("action", action.id, created.id);
          } else if (groupPick === "") {
            if (action.group_id !== null) {
              await unassignFromGroup("action", action.id);
            }
          } else if (Number(groupPick) !== action.group_id) {
            await assignToGroup("action", action.id, Number(groupPick));
          }
        }
        await onsave(t("actionform.savedFlash").replace("{name}", name.trim()));
      } else {
        const created = await createQuickAction({
          name: name.trim(),
          shell,
          command: command.trim(),
          cwd: cwd.trim() || null,
          stoppable,
          stop_command: stoppable ? stopCommand.trim() || null : null,
          note: trimmedNote,
          auto_run: autoRun,
          show_in_dock: showInDock,
          pre_check: trimmedPreCheck,
          pre_fix: trimmedPreFix,
        });
        // Files picked while adding hang off the new row here — the same
        // outside-the-payload shape as group placement below, and the backend
        // revalidates names, dupes, and caps before anything is written.
        for (const row of fileRows) {
          if (row.bytesBase64 === null) continue;
          try {
            await attachQuickActionFile(created.id, row.filename, row.bytesBase64);
          } catch (e) {
            console.error(e);
            error = String(e);
            return;
          }
        }
        if (placing) {
          if (creatingGroup) {
            const group = await createGroup("action", trimmedGroupName);
            await assignToGroup("action", created.id, group.id);
          } else if (groupPick !== "") {
            await assignToGroup("action", created.id, Number(groupPick));
          }
        }
        await onsave(t("actionform.addedFlash").replace("{name}", name.trim()));
      }
    } catch (e) {
      console.error(e);
      error = String(e);
    } finally {
      saving = false;
    }
  }

  /** One attached-file row: the stored id once persisted, in-memory bytes
   *  while the action is still unsaved (an add has no row to hang files on
   *  until creation resolves). */
  interface AttachedFileRow {
    id: number | null;
    filename: string;
    size: number;
    bytesBase64: string | null;
  }

  let fileRows = $state<AttachedFileRow[]>([]);
  let filesError = $state("");
  let filesBusy = $state(false);
  let filesAnnouncement = $state("");
  let filePick: HTMLInputElement | null = $state(null);

  /** The command editor stays a plain textarea (native paste/undo/IME); the
   *  highlight copy behind it and the suggestion list below it are visual
   *  only, announced through a live region instead of focus moves. */
  let cmdEl = $state<HTMLTextAreaElement | null>(null);
  let cmdHl = $state<HTMLPreElement | null>(null);
  let suggestCaret = $state(0);
  let suggestActive = $state(0);
  let suggestClosed = $state(false);

  const fileNames = $derived(fileRows.map((row) => row.filename));
  const highlighted = $derived(tokenizeQuickActionCommand(command, shell, fileNames));
  const filesCompletionState = $derived(
    suggestClosed ? null : filesCompletion(command, suggestCaret, fileNames),
  );
  const suggestItems = $derived(filesCompletionState?.items ?? []);
  const suggestOpen = $derived(
    filesCompletionState !== null && suggestItems.length > 0,
  );
  const filesHintState = $derived(filesHint(command, fileRows.length));
  const suggestAnnouncement = $derived(
    !suggestOpen
      ? ""
      : suggestItems.length === 1 && suggestItems[0] === "<FilesDir>"
        ? t("actionform.suggestOne")
        : t("actionform.suggestMany").replace("{count}", String(suggestItems.length)),
  );

  function fileSizeOf(name: string): number {
    return fileRows.find((row) => row.filename === name)?.size ?? 0;
  }

  function sortFileRows() {
    fileRows = [...fileRows].sort((a, b) =>
      a.filename.localeCompare(b.filename, undefined, { sensitivity: "base" }),
    );
  }

  async function loadFiles(id: number) {
    filesError = "";
    try {
      const listed = await listQuickActionFiles(id);
      fileRows = listed.map((f) => ({
        id: f.id,
        filename: f.filename,
        size: Number(f.size),
        bytesBase64: null,
      }));
    } catch (e) {
      console.error(e);
      filesError = String(e);
    }
  }

  function readPickedFile(file: File): Promise<string> {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onerror = () => reject(new Error(t("clips.imgErrReadShort")));
      reader.onload = () => {
        const url = String(reader.result ?? "");
        const bytes = url.split(",", 2)[1] ?? "";
        if (!bytes) reject(new Error(t("actionform.fileReadRetry")));
        else resolve(bytes);
      };
      reader.readAsDataURL(file);
    });
  }

  async function onFilesPicked(event: Event) {
    const input = event.target as HTMLInputElement | null;
    const picked = [...(input?.files ?? [])];
    if (input) input.value = "";
    if (picked.length === 0) return;
    filesError = "";
    filesBusy = true;
    try {
      for (const file of picked) {
        const base = file.name.split(/[\\/]/).pop()?.trim() ?? "";
        if (!base) {
          filesError = t("actionform.fileNoName");
          continue;
        }
        if (
          fileRows.some(
            (row) => row.filename.toLowerCase() === base.toLowerCase(),
          )
        ) {
          filesError = t("actionform.fileDupe").replace("{name}", base);
          continue;
        }
        if (file.size > QUICK_ACTION_FILE_MAX_BYTES) {
          filesError = t("actionform.fileTooBig")
            .replace("{name}", base)
            .replace("{size}", formatActionFileBytes(file.size));
          continue;
        }
        const used = fileRows.reduce((n, row) => n + row.size, 0);
        if (used + file.size > QUICK_ACTION_FILES_MAX_BYTES) {
          filesError = t("actionform.filesTooMany");
          continue;
        }
        let bytesBase64: string;
        try {
          bytesBase64 = await readPickedFile(file);
        } catch (e) {
          console.error(e);
          filesError = t("actionform.fileReadFail")
            .replace("{name}", base)
            .replace("{detail}", e instanceof Error ? e.message : String(e));
          continue;
        }
        // An edit persists straight away (the row exists); an add stages in
        // memory until creation resolves above.
        if (editing && action) {
          try {
            const meta = await attachQuickActionFile(action.id, base, bytesBase64);
            fileRows = [
              ...fileRows,
              {
                id: meta.id,
                filename: meta.filename,
                size: Number(meta.size),
                bytesBase64: null,
              },
            ];
            sortFileRows();
            filesAnnouncement = t("actionform.fileAttached").replace("{name}", meta.filename);
          } catch (e) {
            console.error(e);
            filesError = String(e);
          }
        } else {
          fileRows = [
            ...fileRows,
            { id: null, filename: base, size: file.size, bytesBase64 },
          ];
            sortFileRows();
            filesAnnouncement = t("actionform.fileAttached").replace("{name}", base);
        }
      }
    } finally {
      filesBusy = false;
    }
  }

  async function removeFile(row: AttachedFileRow) {
    filesError = "";
    if (row.id === null) {
      fileRows = fileRows.filter((r) => r !== row);
      filesAnnouncement = t("actionform.fileRemoved").replace("{name}", row.filename);
      return;
    }
    filesBusy = true;
    try {
      await removeQuickActionFile(row.id);
      fileRows = fileRows.filter((r) => r !== row);
      filesAnnouncement = t("actionform.fileRemoved").replace("{name}", row.filename);
    } catch (e) {
      console.error(e);
      filesError = String(e);
    } finally {
      filesBusy = false;
    }
  }

  // Download lives where the files do (research 0006 pattern 4): per-file
  // beside its row plus Download-all-zip while two or more persist, main-app
  // only — the dock stays read-only (research 0004 rule 3). Persisted rows
  // only; Add-dialog staged bytes need nothing until creation resolves.
  const persistedFileCount = $derived(fileRows.filter((row) => row.id !== null).length);

  async function downloadFile(row: AttachedFileRow) {
    if (row.id === null) return;
    filesError = "";
    filesBusy = true;
    try {
      const result = await downloadSingleFile(row.id, row.filename);
      if (result === "saved")
        filesAnnouncement = t("actionform.fileDownloaded").replace("{name}", row.filename);
    } catch (e) {
      console.error(e);
      filesError = String(e);
    } finally {
      filesBusy = false;
    }
  }

  async function downloadAllFiles() {
    if (!editing || !action) return;
    filesError = "";
    filesBusy = true;
    try {
      const result = await downloadFilesZip(action.id, action.name);
      if (result === "saved") {
        filesAnnouncement = t("actionform.filesDownloadedAll").replace(
          "{count}",
          String(persistedFileCount),
        );
      }
    } catch (e) {
      console.error(e);
      filesError = String(e);
    } finally {
      filesBusy = false;
    }
  }

  // Cmd `start` without an empty title eats its own quoted path: warn with
  // the fixed forms, never block — the check is advisory, Save stays open.
  const startTrap = $derived(detectCmdStartTitleTrap(command, shell));

  function trackCaret(reopen: boolean) {
    if (cmdEl) suggestCaret = cmdEl.selectionStart ?? command.length;
    if (reopen) {
      suggestClosed = false;
      suggestActive = 0;
    }
  }

  function syncHlScroll() {
    if (cmdEl && cmdHl) {
      cmdHl.scrollTop = cmdEl.scrollTop;
      cmdHl.scrollLeft = cmdEl.scrollLeft;
    }
  }

  // Escape stays with the shared dialog's close (research 0010 keeps dialog
  // keys un-rerouted): the list dismisses by typing on, choosing, or leaving
  // the field — never by swallowing the dialog's own key.
  function cmdKeydown(event: KeyboardEvent) {
    if (!suggestOpen) return;
    if (event.key === "ArrowDown") {
      event.preventDefault();
      suggestActive = (suggestActive + 1) % suggestItems.length;
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      suggestActive =
        (suggestActive - 1 + suggestItems.length) % suggestItems.length;
    } else if (event.key === "Enter" || (event.key === "Tab" && !event.shiftKey)) {
      event.preventDefault();
      acceptSuggestion(suggestActive);
    }
  }

  async function placeCaret(pos: number) {
    await tick();
    cmdEl?.focus();
    cmdEl?.setSelectionRange(pos, pos);
    suggestCaret = pos;
  }

  function acceptSuggestion(index: number) {
    const state = filesCompletionState;
    const item = state?.items[index];
    if (!state || !item) return;
    // Inserted raw: the run-time owner shell-quotes the staged path
    // (ADR-0029), so the editor must not pre-quote either half.
    const applied = applyCompletion(command, state.start, state.end, item);
    command = applied.text;
    suggestClosed = true;
    suggestActive = 0;
    void placeCaret(applied.caret);
  }

  /** One suggestion row's detail voice: the folder for the placeholder, the
   *  file's size for a file — a `<FilesDir>\<name>` path reads back through
   *  its filename so every row stays distinguishable. */
  function suggestDetail(item: string): string {
    if (item === "<FilesDir>") return t("actionform.suggestFolderDetail");
    const name = item.startsWith("<FilesDir>\\")
      ? item.slice("<FilesDir>\\".length)
      : item;
    return formatActionFileBytes(fileSizeOf(name));
  }

  function insertFilesDir() {
    const el = cmdEl;
    const at = el?.selectionStart ?? command.length;
    const to = el?.selectionEnd ?? at;
    const applied = applyCompletion(
      command,
      Math.min(at, to),
      Math.max(at, to),
      "<FilesDir>",
    );
    command = applied.text;
    suggestClosed = true;
    void placeCaret(applied.caret);
  }
</script>

<Dialog
  {open}
  title={editing ? t("actionform.editTitle") : t("actionform.addTitle")}
  onclose={oncancel}
  width={560}
  focusTarget={aiReady && !editing ? "#qa-ai-request" : undefined}
>
  <form
    class="form"
    onsubmit={(e) => {
      e.preventDefault();
      submit();
    }}
  >
    {#if aiReady}
      <!-- Two exclusive views (ADR-0028): tabs top-right on-surface, absent
           when unready. Add opens AI-first, Edit opens manual-first; flips
           are instant with both labels visible for scent. -->
      <div class="viewtabs" role="tablist" aria-label={t("actionform.viewLabel")} tabindex="-1" onkeydown={viewTabsKeydown}>
        <button
          id="tab-ai"
          type="button"
          role="tab"
          aria-selected={aiView === "ai"}
          aria-controls="panel-ai"
          tabindex={aiView === "ai" ? 0 : -1}
          class="viewtabs__tab"
          class:active={aiView === "ai"}
          onclick={() => selectView("ai")}
        >
          {t("actionform.viewAi")}
        </button>
        <button
          id="tab-manual"
          type="button"
          role="tab"
          aria-selected={aiView === "manual"}
          aria-controls="panel-manual"
          tabindex={aiView === "manual" ? 0 : -1}
          class="viewtabs__tab"
          class:active={aiView === "manual"}
          onclick={() => selectView("manual")}
        >
          {t("actionform.viewManual")}
        </button>
      </div>
    {/if}

    {#if aiReady && aiView === "ai"}
      <!-- AI view: one Describe textarea plus Generate and outcome. No shell
           picker, no find/roots chrome up front — the shell lives in Manual,
           discovery appears only when a clarification needs it. -->
      <div id="panel-ai" role="tabpanel" aria-labelledby="tab-ai" class="ai">
        <!-- Describe block: one field owns label → textarea → status →
             Generate (8px internal gap). The button belongs to the textarea
             directly — siblings under .ai would each add the 24px field-stack
             gap and strand it 48px below (research 0023). -->
        <div class="field">
          <div class="field__label-row">
            <label class="field__label" for="qa-ai-request">{t("actionform.describeLabel")}</label>
            <InfoTip label={t("actionform.draftHow")}>
              <p>{t("actionform.draftBody")}</p>
            </InfoTip>
          </div>
          <textarea
            id="qa-ai-request"
            name="ai-request"
            class="field__cmd"
            rows="3"
            maxlength="2000"
            placeholder={t("actionform.describePh")}
            autocomplete="off"
            spellcheck="true"
            value={aiRequest}
            oninput={(e) => (aiRequest = (e.target as HTMLTextAreaElement).value)}
            onkeydown={aiKeydown}
          ></textarea>
          <div class="ai__actions">
            {#if aiPending}
              <p class="ai__status" role="status">{t("actionform.drafting")}</p>
              <Button type="button" variant="ghost" onclick={cancelDraft}>{t("common.cancel")}</Button>
            {:else}
              <Button type="button" variant="secondary" onclick={() => void generateDraft()}>
                {t("actionform.genDraft")}
              </Button>
            {/if}
          </div>
          {#if aiNotice}
            <p class="field__hint" role="status">{aiNotice}</p>
          {/if}
        </div>

        {#if aiOutcome}
          <div id="ai-outcome" tabindex="-1" class="ai__outcome">
            {#if aiDraft}
              <p class="ai__flag">{t("actionform.notRun")}</p>
              <p class="ai__shell">{quickActionShellLabel[aiDraft.shell]}</p>
              <p class="ai__command">{aiDraft.command}</p>
              {#if aiDraft.assumptions.length > 0}
                <p class="ai__meta">{t("actionform.assumes").replace("{items}", aiDraft.assumptions.join("; "))}</p>
              {/if}
              {#if aiDraft.affected_targets.length > 0}
                <p class="ai__meta">{t("actionform.touches").replace("{items}", aiDraft.affected_targets.join("; "))}</p>
              {/if}
              {#if aiDraft.explanation}
                <p class="ai__explanation">{aiDraft.explanation}</p>
              {/if}
              {#if prereqsChecking}
                <p class="ai__status" role="status">{t("actionform.checkingPrereqs")}</p>
              {/if}
              {#each prereqRows as line (line.key)}
                {#if line.status === "present"}
                  <p class="ai__meta">{line.text}</p>
                {:else}
                  <Notice tone="warn">{line.text}</Notice>
                {/if}
              {/each}
              <div class="ai__actions">
                {#if aiAppliedCurrent}
                  <p class="ai__status" role="status">{t("actionform.appliedInManual")}</p>
                {:else}
                  <Button type="button" variant="secondary" onclick={() => void applyDraft()}>
                    {t("actionform.useDraft")}
                  </Button>
                {/if}
                <Button type="button" variant="ghost" onclick={dismissDraft}>{t("common.dismiss")}</Button>
              </div>
            {:else if aiClarify}
              <Notice tone="warn">{aiClarify.message}</Notice>
              <p class="ai__meta">
                {t("actionform.clarifyRound").replace("{n}", String(clarifyRound + 1))}
              </p>
              {#if aiClarifyChoices.length > 0}
                <div class="find__list" role="radiogroup" aria-label={t("actionform.clarifyChoices")}>
                  {#each aiClarifyChoices as choice (choice)}
                    <label class="find__row">
                      <input
                        type="radio"
                        name="qa-clarify-pick"
                        value={choice}
                        checked={clarifyPick === choice}
                        onchange={() => (clarifyPick = choice)}
                      />
                      <span class="find__name">{choice}</span>
                    </label>
                  {/each}
                </div>
                {#if aiClarifyAspect === "unknown-prerequisite"}
                  {#if prereqsChecking}
                    <p class="ai__status" role="status">{t("actionform.checkingPrereqs")}</p>
                  {/if}
                  {#each prereqRows as line (line.key)}
                    {#if line.status === "present"}
                      <p class="ai__meta">{line.text}</p>
                    {:else}
                      <Notice tone="warn">{line.text}</Notice>
                    {/if}
                  {/each}
                {/if}
                <div class="field">
                  <div class="field__label-row">
                    <label class="field__label" for="qa-clarify-free">{t("actionform.describeYourself")}</label>
                  </div>
                  <textarea
                    id="qa-clarify-free"
                    class="field__cmd"
                    rows="2"
                    maxlength="2000"
                    placeholder={t("actionform.clarifyPh")}
                    autocomplete="off"
                    spellcheck="true"
                    value={clarifyFree}
                    oninput={(e) => (clarifyFree = (e.target as HTMLTextAreaElement).value)}
                    onkeydown={clarifyKeydown}
                  ></textarea>
                </div>
                <div class="ai__actions">
                  <Button type="button" variant="secondary" onclick={() => void regenerateWithClarification()}>
                    {t("actionform.continue")}
                  </Button>
                  <Button type="button" variant="ghost" onclick={() => void draftAnyway()}>
                    {t("actionform.draftAnyway")}
                  </Button>
                  <Button type="button" variant="ghost" onclick={dismissDraft}>{t("common.dismiss")}</Button>
                </div>
              {:else}
                <div class="ai__actions">
                  <Button type="button" variant="ghost" onclick={dismissDraft}>{t("common.dismiss")}</Button>
                </div>
              {/if}
            {:else if aiRefusal}
              <Notice tone="warn">{aiRefusal}</Notice>
              <div class="ai__actions">
                <Button type="button" variant="ghost" onclick={dismissDraft}>{t("common.dismiss")}</Button>
              </div>
            {:else if aiFailure}
              <Notice tone="error">{aiFailure}</Notice>
              <div class="ai__actions">
                <Button type="button" variant="ghost" onclick={dismissDraft}>{t("common.dismiss")}</Button>
              </div>
            {/if}
          </div>
        {/if}

        {#if editing}
          <!-- Diagnosis (ticket 153): edit-only. Sends the current script
               plus pasted error output; returns an explanation or a
               separately reviewed revision. Nothing runs, and accepting
               applies only the reviewed fields. Owns its status + Diagnose
               the same way Describe owns Generate (research 0023). -->
          <div class="field">
            <div class="field__label-row">
              <label class="field__label" for="qa-diag-error">{t("actionform.diagLabel")}</label>
              <InfoTip label={t("actionform.diagHow")}>
                <p>{t("actionform.diagBody")}</p>
              </InfoTip>
            </div>
            <textarea
              id="qa-diag-error"
              name="diag-error"
              class="field__cmd"
              rows="3"
              maxlength="8000"
              placeholder={t("actionform.diagPh")}
              autocomplete="off"
              spellcheck="false"
              value={diagError}
              oninput={(e) => (diagError = (e.target as HTMLTextAreaElement).value)}
              onkeydown={diagKeydown}
            ></textarea>
            <div class="ai__actions">
              {#if diagPending}
                <p class="ai__status" role="status">{t("actionform.diagnosing")}</p>
                <Button type="button" variant="ghost" onclick={cancelDiagnose}>{t("common.cancel")}</Button>
              {:else}
                <Button type="button" variant="secondary" onclick={() => void diagnose()}>
                  {t("actionform.diagnose")}
                </Button>
              {/if}
            </div>
            {#if diagNotice}
              <p class="field__hint" role="status">{diagNotice}</p>
            {/if}
            {#if diagConflict}
              <Notice tone="warn">{diagConflict}</Notice>
            {/if}
          </div>

          {#if diagOutcome}
            <div id="diag-outcome" tabindex="-1" class="ai__outcome">
              {#if diagRevision}
                <p class="ai__flag">{t("actionform.notRunAccept")}</p>
                {#if diagRevision.explanation}
                  <p class="ai__explanation">{diagRevision.explanation}</p>
                {/if}
                <p class="ai__shell">{quickActionShellLabel[diagRevision.shell]}</p>
                <p class="ai__command">{diagRevision.command}</p>
                {#if diagRevision.cwd}
                  <p class="ai__meta">{t("actionform.workingDirMeta").replace("{cwd}", diagRevision.cwd)}</p>
                {/if}
                {#if diagRevision.note}
                  <p class="ai__meta">{t("actionform.proposedNote").replace("{note}", diagRevision.note)}</p>
                {/if}
                {#if diagRevision.assumptions.length > 0}
                  <p class="ai__meta">{t("actionform.assumes").replace("{items}", diagRevision.assumptions.join("; "))}</p>
                {/if}
                {#if diagRevision.affected_targets.length > 0}
                  <p class="ai__meta">{t("actionform.touches").replace("{items}", diagRevision.affected_targets.join("; "))}</p>
                {/if}
                <p class="ai__meta">{t("actionform.acceptNote").replace("{state}", autoRun ? t("common.on") : t("common.off"))}</p>
                <div class="ai__actions">
                  <Button type="button" variant="secondary" onclick={() => void applyRevision()}>
                    {t("actionform.acceptRevision")}
                  </Button>
                  <Button type="button" variant="ghost" onclick={dismissRevision}>{t("common.dismiss")}</Button>
                </div>
              {:else if diagExplanationOnly}
                <p class="ai__explanation">{diagExplanationOnly}</p>
                <div class="ai__actions">
                  <Button type="button" variant="ghost" onclick={dismissRevision}>{t("common.dismiss")}</Button>
                </div>
              {:else if diagRefusal}
                <Notice tone="warn">{diagRefusal}</Notice>
                <div class="ai__actions">
                  <Button type="button" variant="ghost" onclick={dismissRevision}>{t("common.dismiss")}</Button>
                </div>
              {:else if diagClarify}
                <Notice tone="warn">{diagClarify.message}</Notice>
                <div class="ai__actions">
                  <Button type="button" variant="ghost" onclick={dismissRevision}>{t("common.dismiss")}</Button>
                </div>
              {:else if diagFailure}
                <Notice tone="error">{diagFailure}</Notice>
                <div class="ai__actions">
                  <Button type="button" variant="ghost" onclick={dismissRevision}>{t("common.dismiss")}</Button>
                </div>
              {/if}
            </div>
          {/if}
        {/if}

        {#if boundTarget}
          <div class="ai__outcome">
            <p class="ai__flag">{t("actionform.notRun")}</p>
            <p class="ai__shell">{quickActionShellLabel[boundTarget.shell]}</p>
            <p class="ai__command">{boundTarget.command}</p>
            <p class="ai__meta">{t("actionform.opensTarget").replace("{target}", boundTarget.target)}</p>
            {#if boundTarget.warning}
              <Notice tone="warn">{boundTarget.warning}</Notice>
            {/if}
            <div class="ai__actions">
              <Button type="button" variant="ghost" onclick={dismissBound}>{t("common.dismiss")}</Button>
            </div>
          </div>
        {/if}

        <div class="form__actions">
          <Button type="button" variant="secondary" onclick={oncancel} disabled={aiPending}>
            {t("common.cancel")}
          </Button>
        </div>
      </div>
    {:else}
      <div
        id={aiReady ? "panel-manual" : undefined}
        role={aiReady ? "tabpanel" : undefined}
        aria-labelledby={aiReady ? "tab-manual" : undefined}
        class="manual"
      >
      {#if !aiReady}
        <!-- Ticket 192: one plain-text pointer from the unready view to
             Settings — no tabs, no hero, no duplicated config, no disabled
             teaser. The focus ring is the only highlight: no pulse, no flash,
             no smooth-scroll library. -->
        <p class="ai-setup">
          {#if aiSetupKind === "managed"}
            {t("actionform.aiUnreadyManaged")}<button
              type="button"
              class="ai-setup__link"
              onclick={() => void onsetupai?.()}>{t("actionform.aiEnableIt")}</button
            >
          {:else}
            <button
              type="button"
              class="ai-setup__link"
              onclick={() => void onsetupai?.()}>{t("actionform.aiSetupLink")}</button
            >
          {/if}
        </p>
      {/if}
      {#if manualNotice}
        <p class="ai__status" role="status">{manualNotice}</p>
      {/if}
    <!-- Manual view: today's fields unchanged (Name/Shell/Command + files +
         Details rares + Test/Save + recheck hatch). Flips preserve typing;
         no validation fires on flip; Save keeps Manual semantics. -->

    <TextInput
      id="qa-name"
      label={t("common.name")}
      required
      autofocus={!aiReady || editing}
      placeholder={t("actionform.namePh")}
      value={name}
      onchange={(v) => (name = v)}
    />

    <div class="field">
      <div class="field__label-row">
        <label class="field__label" for="qa-shell">{t("qdetails.shell")}</label>
        <InfoTip label={t("commandform.shellHow")}>
          <p>{t("actionform.shellBody")}</p>
        </InfoTip>
      </div>
      <Select
        id="qa-shell"
        value={shell}
        onchange={(v) => (shell = v as QuickActionShell)}
        options={[
          { value: "powershell", label: quickActionShellLabel.powershell },
          { value: "cmd", label: quickActionShellLabel.cmd },
          { value: "python3", label: quickActionShellLabel.python3 },
        ]}
      />
      <p class="field__hint">
        {shell === "powershell"
          ? t("actionform.shellHintPowershell")
          : shell === "cmd"
            ? t("actionform.shellHintCmd")
            : t("actionform.shellHintPython")}
      </p>
    </div>

    <div class="field">
      <div class="field__label-row">
        <label class="field__label" for="qa-command">{t("qdetails.command")}</label>
        <InfoTip label={t("actionform.placeholdersHow")}>
          <p>{t("actionform.placeholdersBody")}</p>
        </InfoTip>
      </div>
      <div class="cmdwrap">
        <pre
          class="cmdwrap__hl"
          aria-hidden="true"
          bind:this={cmdHl}
        ><code>{#each highlighted as tok}<span class="tok-{tok.kind}">{tok.text}</span>{/each}{#if command.endsWith("\n")}<span>&#8203;</span>{/if}</code></pre>
        <textarea
          id="qa-command"
          class="field__cmd cmdwrap__input"
          rows="6"
          placeholder={shell === "python3" ? t("actionform.cmdPhPython") : t("actionform.cmdPh")}
          autocomplete="off"
          autocapitalize="none"
          spellcheck="false"
          role="combobox"
          aria-autocomplete="list"
          aria-expanded={suggestOpen}
          aria-controls="qa-files-suggest"
          aria-activedescendant={suggestOpen ? `qa-suggest-${suggestActive}` : undefined}
          bind:this={cmdEl}
          value={command}
          oninput={(e) => {
            command = (e.target as HTMLTextAreaElement).value;
            trackCaret(true);
            syncHlScroll();
          }}
          onkeyup={() => trackCaret(false)}
          onclick={() => trackCaret(false)}
          onscroll={syncHlScroll}
          onblur={() => (suggestClosed = true)}
          onkeydown={cmdKeydown}
        ></textarea>
      </div>
      {#if suggestOpen}
        <div
          class="suggest"
          role="listbox"
          id="qa-files-suggest"
          aria-label={t("actionform.suggestLabel")}
        >
          {#each suggestItems as item, idx (item)}
            <button
              type="button"
              role="option"
              id="qa-suggest-{idx}"
              aria-selected={idx === suggestActive}
              class="suggest__item"
              class:suggest__item--active={idx === suggestActive}
              tabindex="-1"
              onmousedown={(e) => e.preventDefault()}
              onmouseenter={() => (suggestActive = idx)}
              onclick={() => acceptSuggestion(idx)}
            >
              <span class="suggest__label">{item}</span>
              <span class="suggest__detail">{suggestDetail(item)}</span>
            </button>
          {/each}
          <p class="field__hint">{t("actionform.suggestKeys")}</p>
        </div>
      {/if}
      <p class="sr-only" role="status">{suggestAnnouncement}</p>
      {#if filesHintState === "unused"}
        <p class="field__hint">
          {t("actionform.filesUnused")}
        </p>
      {:else if filesHintState === "missing"}
        <p class="field__hint">
          {t("actionform.filesMissing")}
        </p>
      {/if}
      {#if startTrap}
        <Notice tone="warn">{t("actionform.startTrap")}</Notice>
      {/if}
    </div>

    <div class="field">
      <div class="field__label-row">
        <span class="field__label" id="qa-files-label">{t("actionform.filesHead")}</span>
        <InfoTip label={t("actionform.filesHow")}>
          <p>{t("actionform.filesBody")}</p>
        </InfoTip>
      </div>
      {#if fileRows.length === 0}
        <p class="field__hint">{t("actionform.filesNone")}</p>
      {:else}
        <ul class="files__list" aria-labelledby="qa-files-label">
          {#each fileRows as row (row.filename.toLowerCase())}
            <li class="files__row">
              <span class="files__name">{row.filename}</span>
              <span class="files__size">{formatActionFileBytes(row.size)}</span>
              {#if editing && row.id !== null}
                <Button
                  type="button"
                  variant="ghost"
                  disabled={filesBusy}
                  onclick={() => void downloadFile(row)}
                >
                  {t("menu.download")}
                </Button>
              {/if}
              <Button
                type="button"
                variant="ghost"
                disabled={filesBusy}
                onclick={() => void removeFile(row)}
              >
                {t("common.remove")}
              </Button>
            </li>
          {/each}
        </ul>
      {/if}
      <div class="files__actions">
        <Button
          type="button"
          variant="secondary"
          disabled={filesBusy}
          onclick={() => filePick?.click()}
        >
          {t("actionform.attachFiles")}
        </Button>
        <Button type="button" variant="secondary" onclick={insertFilesDir} disabled={filesBusy || fileRows.length === 0}>
          {t("actionform.insert")}
        </Button>
        {#if editing && persistedFileCount >= 2}
          <Button
            type="button"
            variant="secondary"
            disabled={filesBusy}
            onclick={() => void downloadAllFiles()}
          >
            {t("menu.downloadAll")}
          </Button>
        {/if}
      </div>
      <input
        bind:this={filePick}
        type="file"
        multiple
        class="sr-only"
        tabindex="-1"
        aria-hidden="true"
        onchange={(e) => void onFilesPicked(e)}
      />
      {#if filesError}
        <p class="files__error" role="alert">{filesError}</p>
      {/if}
      <p class="sr-only" role="status">{filesAnnouncement}</p>
    </div>

    <!-- Rarely-touched fields behind one Details disclosure: frequent intent
         stays up front while the layout stays scannable without a new page.
         Each choice applies on Save, never immediately. -->
    <div class="advanced">
      <Disclosure
        open={detailsOpen}
        controls="qa-details-body"
        label={t("actionform.details")}
        onclick={() => (detailsOpen = !detailsOpen)}
      />

      <div id="qa-details-body" class="advanced__body" hidden={!detailsOpen}>
        <TextInput
          id="qa-cwd"
          label={t("qdetails.cwd")}
          placeholder={t("actionform.cwdPh")}
          value={cwd}
          onchange={(v) => (cwd = v)}
          info={t("actionform.cwdHow")}
        >
          {#snippet infobody()}
            <p>{t("actionform.cwdBody")}</p>
          {/snippet}
        </TextInput>

        {#if groupsEnabled && groups.length > 0}
          <div class="field">
            <div class="field__label-row">
              <label class="field__label" for="qa-group">{t("actionform.groupLabel")}</label>
            </div>
            <Select
              id="qa-group"
              value={groupPick}
              onchange={(v) => (groupPick = v)}
              options={[
                { value: "", label: t("menu.ungrouped") },
                ...groups.map((group) => ({
                  value: String(group.id),
                  label: group.name,
                })),
                { value: NEW_GROUP, label: t("menu.newGroup") },
              ]}
            />
            {#if groupPick === NEW_GROUP}
              <TextInput
                id="qa-new-group"
                label={t("actionform.newGroupName")}
                required
                placeholder={t("actions.groupEx")}
                value={newGroupName}
                onchange={(v) => (newGroupName = v)}
              />
            {/if}
          </div>
        {/if}

        <div class="field">
          <div class="field__label-row">
            <label class="field__label" for="qa-note">{t("actionform.notesLabel")}</label>
            <InfoTip label={t("actionform.notesHow")}>
              <p>{t("actionform.notesBody")}</p>
            </InfoTip>
          </div>
          <textarea
            id="qa-note"
            class="field__cmd field__cmd--note"
            rows="4"
            placeholder={t("actionform.notesPh")}
            autocomplete="off"
            spellcheck="true"
            value={note}
            oninput={(e) => (note = (e.target as HTMLTextAreaElement).value)}
          ></textarea>
        </div>

        <Checkbox
          checked={stoppable}
          onchange={(v) => (stoppable = v)}
          title={t("actionform.stopBtnTitle")}
          hint={t("actionform.stopBtnHint")}
        />

        {#if stoppable}
          <div class="field">
            <div class="field__label-row">
              <label class="field__label" for="qa-stop-command">{t("actionform.stopCmdLabel")}</label>
              <InfoTip label={t("actionform.stopCmdHow")}>
                <p>{t("actionform.stopCmdBody")}</p>
              </InfoTip>
            </div>
            <textarea
              id="qa-stop-command"
              class="field__cmd"
              rows="2"
              placeholder={t("actionform.stopCmdPh")}
              autocomplete="off"
              spellcheck="false"
              value={stopCommand}
              oninput={(e) => (stopCommand = (e.target as HTMLTextAreaElement).value)}
            ></textarea>
          </div>
        {/if}

        <Checkbox
          checked={autoRun}
          onchange={(v) => (autoRun = v)}
          title={t("actionform.autoRunTitle")}
          hint={t("actionform.autoRunHint")}
        />

        <!-- Dock visibility: the control lives on its object — hiding is
             per-item, not a feature switch. All three flags keep a short
             inline consequence only, no InfoTip, so the rows stay consistent. -->
        <Checkbox
          checked={showInDock}
          onchange={(v) => (showInDock = v)}
          title={t("common.showInDock")}
          hint={t("common.showInDockHint")}
        />

        <!-- Pre-action gate: an optional check that runs first on every Run,
             plus the fix offered only when the check blocks one. Collapsed
             until needed; empty leaves no trace on save. -->
        <div class="preaction">
          <Disclosure
            open={preActionOpen}
            controls="qa-preaction-body"
            label={t("actionform.preAction")}
            onclick={() => (preActionOpen = !preActionOpen)}
          />

          <div id="qa-preaction-body" class="preaction__body" hidden={!preActionOpen}>
            <div class="field">
              <div class="field__label-row">
                <label class="field__label" for="qa-pre-check">{t("actionform.preCheckLabel")}</label>
                <InfoTip label={t("actionform.preCheckHow")}>
                  <p>{t("actionform.preCheckBody")}</p>
                </InfoTip>
              </div>
              <textarea
                id="qa-pre-check"
                class="field__cmd"
                rows="3"
                placeholder={t("actionform.preCheckPh")}
                autocomplete="off"
                spellcheck="false"
                value={preCheck}
                oninput={(e) => (preCheck = (e.target as HTMLTextAreaElement).value)}
              ></textarea>
            </div>

            <div class="field">
              <div class="field__label-row">
                <label class="field__label" for="qa-pre-fix">{t("actionform.preFixLabel")}</label>
                <InfoTip label={t("actionform.preFixHow")}>
                  <p>{t("actionform.preFixBody")}</p>
                </InfoTip>
              </div>
              <textarea
                id="qa-pre-fix"
                class="field__cmd"
                rows="3"
                placeholder={t("actionform.preFixPh")}
                autocomplete="off"
                spellcheck="false"
                value={preFix}
                oninput={(e) => (preFix = (e.target as HTMLTextAreaElement).value)}
              ></textarea>
            </div>

            <div class="check">
              <div class="check__row">
                <Button
                  variant="secondary"
                  disabled={checking || !preCheck.trim()}
                  onclick={() => void runCheck()}
                >
                  {checking ? t("common.busy.checking") : t("actionform.checkBtn")}
                </Button>
                <p class="check__note">
                  {t("actionform.checkNote")}
                </p>
              </div>
              {#if checkRan && !checking}
                {#if checkResult?.timed_out}
                  <Notice tone="warn">{t("actionform.checkBlockedTimeout")}</Notice>
                {:else if checkResult?.exit_code === 0}
                  <Notice tone="ok">{t("actions.checkPassed")}</Notice>
                {:else if checkResult?.exit_code === null}
                  <Notice tone="warn">{t("actionform.checkBlockedNoStart")}</Notice>
                {:else}
                  <Notice tone="warn">{t("actionform.checkBlockedFailed").replace("{code}", String(checkResult?.exit_code))}</Notice>
                {/if}
                {#if checkResult?.output.trim()}
                  <pre class="check__output">{checkResult?.output}</pre>
                {/if}
              {/if}
            </div>
          </div>
        </div>
      </div>
    </div>

    <TestResult
      {open}
      {command}
      bind:testing
      validate={() => cwdError(cwd)}
      probe={() => testQuickAction(shell, command.trim(), cwd.trim() || null)}
      onerror={(message) => (error = message)}
    />

    {#if error}
      <p class="form__error" role="alert">{error}</p>
    {/if}

    <div class="form__actions">
      <Button variant="secondary" onclick={oncancel} disabled={saving || testing || checking}>
        {t("common.cancel")}
      </Button>
      <Button kind="submit" disabled={saving || testing || checking}>
        {saving
          ? editing
            ? t("common.busy.saving")
            : t("common.busy.adding")
          : editing
            ? t("common.saveChanges")
            : t("actionform.addBtn")}
      </Button>
    </div>
      </div>
    {/if}
  </form>
</Dialog>

<style>
  .form {
    display: flex;
    flex-direction: column;
    /* Discord field rhythm in Ledger tokens (research 0021 spacing round):
       24px field → field; the 8px field-internal gap owns label → control
       → hint. */
    gap: var(--space-5);
    min-width: 0;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    min-width: 0;
  }

  /* Manual tab panel: the fields are NOT direct children of .form (the
     panel wrapper sits between), so the form stack gap never reached them
     and Name stitched straight into the Shell eyebrow with 0px. The panel
     owns the same 24px field → field rhythm (research 0021 spacing round,
     0005 rule 6). */
  .manual {
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
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
       hover wash, glowing focus (research 0020 enhancement). The highlight
       overlay textarea stays out of the wash so the single chrome never
       doubles (see .cmdwrap below). */
    transition: border-color var(--dur-fast) var(--ease-out),
      background-color var(--dur-fast) var(--ease-out),
      box-shadow var(--dur-fast) var(--ease-out);
  }

  .field__cmd:not(.cmdwrap__input):hover {
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

  .field__cmd--note {
    min-height: 88px;
    font-family: var(--font-body);
  }

  .field__hint {
    margin: 0;
    font-size: var(--text-xs);
    line-height: var(--leading-tight);
    color: var(--text-muted);
  }

  /* Details-collapsed rare options: the same flat disclosure treatment as
     the product form's Advanced — no frame, body separated by a dashed rule.
     `hidden` needs its own rule or the flex display above keeps the panel
     permanently open. */
  .advanced {
    display: flex;
    flex-direction: column;
    /* The Details chevron is a section, not a field (research 0021 spacing
       round): one extra token over the form stack gap so it stands off the
       previous field even when closed. */
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

  /* Single chrome on the wrap: the highlight copy and the textarea paint
     only text, so the field can never grow a second edge — and the textarea
     is block-level, so no inline baseline gap lingers under its bottom edge. */
  .cmdwrap {
    position: relative;
    /* The shared filled frame (research 0021, ticket 204). */
    background: var(--bg-sunken);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    transition: border-color var(--dur-fast) var(--ease-out),
      background-color var(--dur-fast) var(--ease-out),
      box-shadow var(--dur-fast) var(--ease-out);
  }

  .cmdwrap:hover {
    background-color: var(--bg-hover);
    border-color: var(--border-strong);
  }

  .cmdwrap:focus-within {
    border-color: var(--accent);
    box-shadow: var(--ring-glow);
  }

  .cmdwrap__hl {
    position: absolute;
    inset: 0;
    margin: 0;
    overflow: hidden;
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    line-height: var(--leading-normal);
    color: var(--text);
    background: transparent;
    border: 0;
    border-radius: var(--radius-lg);
    padding: 8px 10px;
    white-space: pre-wrap;
    overflow-wrap: break-word;
    pointer-events: none;
  }

  .cmdwrap__hl code {
    font: inherit;
    white-space: pre-wrap;
    overflow-wrap: break-word;
  }

  .field__cmd.cmdwrap__input {
    position: relative;
    display: block;
    width: 100%;
    border: 0;
    background: transparent;
    color: transparent;
    caret-color: var(--text);
  }

  .field__cmd.cmdwrap__input:focus {
    outline: none;
    border: 0;
    box-shadow: none;
  }

  /* Token colors reuse the design-system palette only — comments faint,
     strings warm, commands the single reserved accent (0006 pattern 6),
     the files placeholder info. Color only, never weight, so the overlay
     keeps the textarea's metrics. */
  .tok-comment {
    color: var(--text-faint);
  }

  .tok-string {
    color: var(--warm-text);
  }

  .tok-command {
    color: var(--accent);
  }

  .tok-placeholder {
    color: var(--info-text);
  }

  /* Placeholder/file suggestions: the same flat in-flow list treatment as
     the AI find rows — no overlay, no absolute placement, mouse and
     keyboard share it while focus stays in the command box. */
  .suggest {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    min-width: 0;
  }

  .suggest__item {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
    min-width: 0;
    text-align: left;
    cursor: pointer;
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    color: var(--text);
    background: var(--bg-page);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: var(--space-1) var(--space-2);
  }

  .suggest__item--active {
    border-color: var(--accent);
    box-shadow: var(--ring-glow);
  }

  .suggest__label {
    overflow-wrap: anywhere;
  }

  .suggest__detail {
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  /* Attached files: the same flat list treatment — name plus mono size,
     per-file Remove beside its row, attach/insert actions below. The list
     stays absent until the first file (minimal-until-content). */
  .files__list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    min-width: 0;
  }

  .files__row {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
  }

  .files__name {
    flex: 1;
    min-width: 0;
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--text);
    overflow-wrap: anywhere;
  }

  .files__size {
    flex: none;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  .files__actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
  }

  .files__error {
    margin: 0;
    font-size: var(--text-sm);
    color: var(--danger-text);
    overflow-wrap: anywhere;
  }

  /* Pre-action gate nested in Details: the same flat disclosure treatment —
     no frame, body separated by a dashed rule. `hidden` needs its own rule
     or the flex display above keeps the panel permanently open. */
  .preaction {
    display: flex;
    flex-direction: column;
  }

  .preaction__body {
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
    padding-top: var(--space-3);
    border-top: 1px dashed var(--border);
  }

  .preaction__body[hidden] {
    display: none;
  }

  /* [Check] probe: the same dashed-box treatment as the Test block — one
     secondary button, one constraint line, verdict plus verbatim output. */
  .check {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    border: 1px dashed var(--border-strong);
    border-radius: var(--radius);
    padding: var(--space-3);
  }

  .check__row {
    display: flex;
    align-items: flex-start;
    gap: var(--space-3);
  }

  .check__note {
    margin: 0;
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  .check__output {
    margin: 0;
    max-height: 160px;
    overflow: auto;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    line-height: var(--leading-normal);
    color: var(--text-muted);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  /* AI view: single hero plus outcome. Flat, no frame — the view switch
     above owns the structure. */
  .ai {
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
  }

  .ai__actions {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  /* Local-target search rows: the same flat list treatment as the AI
     outcome — no overlay, no absolute placement, radios stay native. */
  .find__list {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    min-width: 0;
  }

  .find__row {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
    cursor: pointer;
    min-width: 0;
  }

  .find__row input {
    margin: 0;
    accent-color: var(--accent);
    flex: none;
    align-self: center;
  }

  .find__name {
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--text);
    overflow-wrap: anywhere;
  }

  .find__path {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-muted);
    overflow-wrap: anywhere;
    min-width: 0;
  }

  .find__kind {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    color: var(--text-muted);
    flex: none;
  }

  /* Two-view tabs top-right: view-scoped switch on the dialog surface.
     Labels never wrap, so dock sizes and DPI cannot reflow them. */
  .viewtabs {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-1);
    border-bottom: 1px solid var(--border);
  }

  .viewtabs__tab {
    appearance: none;
    margin: 0 0 -1px;
    padding: var(--space-2);
    border: none;
    border-bottom: 2px solid transparent;
    background: transparent;
    font-family: var(--font-body);
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--text-muted);
    cursor: pointer;
    white-space: nowrap;
  }

  .viewtabs__tab:hover {
    color: var(--text);
  }

  .viewtabs__tab.active {
    color: var(--accent);
    border-bottom-color: var(--accent);
  }

  .viewtabs__tab:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: -2px;
  }

  .ai__status {
    margin: 0;
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  /* Ticket 192: the unready setup pointer — one plain-text line, tokens
     only. The focus ring is the only highlight: no transition, no pulse, so
     the Animation switch and reduced-motion have nothing to gate. */
  .ai-setup {
    margin: 0 0 var(--space-3);
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .ai-setup__link {
    appearance: none;
    background: none;
    border: none;
    padding: 0;
    font: inherit;
    color: var(--accent);
    text-decoration: underline;
    text-underline-offset: 2px;
    cursor: pointer;
  }

  .ai-setup__link:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: 2px;
  }

  .ai__outcome {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-3);
    background: var(--bg-page);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }

  .ai__outcome:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: var(--ring-glow);
  }

  .ai__flag {
    margin: 0;
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    font-weight: 500;
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .ai__shell {
    margin: 0;
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .ai__command {
    margin: 0;
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    line-height: var(--leading-normal);
    color: var(--text);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .ai__meta {
    margin: 0;
    font-size: var(--text-xs);
    line-height: var(--leading-tight);
    color: var(--text-muted);
  }

  .ai__explanation {
    margin: 0;
    font-size: var(--text-sm);
    line-height: var(--leading-normal);
    color: var(--text);
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

  .mono {
    font-family: var(--font-mono);
  }
</style>
