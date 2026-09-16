# Native python3 shell vs `python` inside cmd

## Question

Does a separate native `python3` Quick Action shell make sense when the user
could install Python and type the same `python` command inside a cmd action?
Are spec-205 tickets 207 (py-launcher probe) and 208 (python3 shell) obsolete
— is the only value the missing-runtime case, which cmd's own error could cover?

## Sources

- Spec 205 (parent): `.scratch/sprout-app/issues/205-companion-cap-python-download-staging-grace-spec.md`
  (Python phased Q2/Q7/Q8; stories 4–5; seams `windows_execution/` + `quick_actions`).
- Tickets 207/208: `.scratch/sprout-app/issues/207-prerequisite-py-launcher-probe.md`,
  `.scratch/sprout-app/issues/208-quick-action-python3-shell.md` (208 status: done 2026-09-16).
- Prior art 147: `.scratch/sprout-app/issues/147-quick-action-shells-and-backup-compatibility.md`
  (explicit shell, legacy=PowerShell, shell-aware identity, envelope v2).
- Detect contract 199/200: `.scratch/sprout-app/issues/199-prerequisite-detect-backend.md`
  (read-only `detect_prerequisites`, verdicts, 15s/60s budgets),
  `.scratch/sprout-app/issues/200-prerequisite-skill-dialog-surfacing.md` (warn-never-block).
- Argv builders: `src-tauri/src/windows_execution/process.rs:190-235`
  (`powershell_argv`, `cmd_argv`, `python_argv`, `action_argv`); spawn path
  `process.rs:125-167` (`spawn_action`, `spawn_script`, `hidden`); timed core
  `process.rs:276-296` (spawn failure → `exit_code: None` + `failed to start`).
- Python shell: `src-tauri/src/quick_actions.rs:32-61` (`QuickActionShell`),
  `:769-786` (`explain_python_missing`, `with_python_hint`), `:1017-1051`
  (`test_quick_action_with_timeout` python3 mapping), `:736-763` (run/stop
  through owner); stop dispatch `src-tauri/src/lib.rs:2158-2209`;
  Test command `lib.rs:2296-2307`; `src/lib/api.ts:363-369,508-510`.
- Detect code (on disk): `src-tauri/src/windows_execution/prereqs.rs:33-37`
  (budgets), `:54-57` (allow-list), `:126-151` (`route`), `:228-230`
  (python candidates), `:276-351` (`ran`, `probe_runtime`).
- Frontend: `src/lib/prereqGuidance.ts:9-16` (`py`→`python` alias),
  `:69-82` (install guidance), `:98-125` (warn lines);
  `src/lib/types.ts:452-460` (shell union + labels);
  `src/components/QuickActionFormDialog.svelte:104` (default `powershell`).
- ADRs: `docs/adr/0017-quick-action-execution-model.md` (hidden/unelevated/
  stoppable/tracked), `docs/adr/0029-one-source-of-truth-per-windows-command.md`
  (one owner), `docs/adr/0014-single-backup-document-format.md` + 2026-09-06
  amendment (shell-aware envelope v2), `docs/adr/README.md` (index).
- docs.python.org, "Using Python on Windows",
  https://docs.python.org/3/using/windows.html — §4.1.2 (`python` recommended;
  `py` interchangeable; `python3` "intended to catch accidental uses … not meant
  to be widely used"); §4.1.9 (shebang/`-V:` selection); §4.1.1 (PATH opt-in);
  §4.12 (legacy `py.exe` launcher deprecated, install-manager era).
- Microsoft Learn, `cmd`, https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/cmd
  (`/c` carries out command then exits; `&&`/`||`/`&`/pipes; "'Name' is not
  recognized as an internal or external command" example).
- Rust `std::process::Command`,
  https://doc.rust-lang.org/std/process/struct.Command.html (`spawn` returns
  `io::Error` when the program is not found; a run-then-nonzero-exit returns
  `Ok` with `ExitStatus`; args are literal, `cmd.exe` decoding non-standard).
- Research 0018: `docs/research/0018-ai-qualification-support-report.md`
  (targets 5.1 + cmd only; bears on AI qualification, not on this verdict).

## Decision

**Keep both tickets, narrowed. Neither is obsolete; neither grows.**

- **207 is a gap-fix, not a feature.** On-disk `route()` maps only
  `python|python3` (`prereqs.rs:126-132`) and probes candidates
  `["python","python3"]` (`:228-230`), while the frontend already claims `py`
  as Python (`prereqGuidance.ts:9-16`). A launcher-only machine (Install
  Manager default; PATH entry optional per python.org §4.1.1) therefore reads
  `not-found` though `py -3` would launch. 207's minimal scope — add the `py`
  route + launcher-first order (`py -3`, `python`, `python3`) under unchanged
  budgets/verdicts/guidance — closes a real false-negative the cmd error path
  cannot, because detection runs pre-save without executing draft text (199
  zero-execution; `detect_prerequisites` at `lib.rs:2706-2723`; warn lines at
  `prereqGuidance.ts:98-125`).
- **208 is a thin argv plus an honest error, already shipped.** `python_argv`
  is `("py",["-3","-c",command])` (`process.rs:214-219`) beside
  `cmd_argv` `("cmd",["/c",command])` (`:205-207`), dispatched by one
  `action_argv` (`:226-235`) under ADR-0029. Same hidden policy (`:181-184`),
  same cwd/log/stop path (`:125-167`), same-shell stop
  (`quick_actions.rs:755-763`; `lib.rs:2184-2189`), same Test core
  (`:1017-1051`), same validation/back-compat shape as 147 (legacy reads
  PowerShell; unknown fails honestly). Missing runtime maps spawn-failure to
  "Python 3 was not found — no script ran (…). Install it from python.org or
  the Microsoft Store, then run again." (`:769-786`, applied `:746,762,1043-1049`).
  208's ticket already records done + green suites; cutting it now churns
  shipped behavior for no contract gain.
- **The value is not only the missing case.** Detection gives pre-run hints and
  AI-draft `assumptions`/editor warnings without running anything (199/200);
  the native shell additionally buys argv fidelity (no `cmd /c` wrapper layer,
  no cmd quoting/AutoRun/extension behavior), machine-checkable failure
  (`exit_code: None` vs string-sniffing cmd output), and shell-aware identity
  (same text under `cmd` vs `python3` coexists per 147).

## Rejected

- **Cut 207+208; rely on cmd's auto-error.** Loses the pre-run signal (detect
  never fires before save/run) and replaces a structured spawn-failure
  (`exit_code: None` → install pointer) with locale-fragile substring matching
  on "'…' is not recognized…" text inside merged logs. Weaker on both axes.
- **Cut only 208; keep 207.** Viable but leaves the known-good shipped shell
  (208 done) on the floor while keeping its probe supplier — backwards.
- **Grow 208 (version pins, venv management, `-V:` picker, AI qualification).**
  Out of scope per 205/208 ("no AI qualification for the new shell"; "Sprout
  never vendors Python"). Upstream selection (`-V:`, shebang, venv activation,
  python.org §§4.1.2/4.1.9) stays user-authored text, preferably in cmd.

## Consequences

- 207 lands as alias + order only: `route()` accepts `py` → `RuntimePython`;
  candidates become launcher-first; allow-list/budgets/verdicts/guidance
  unchanged; frontend/backend agree; ownership gate passes (no second invoker).
- 208 stays frozen at shipped scope: `py -3 -c` + honest-missing + 147
  envelope-v2 backup. No version-pin UI, no venv handling, no shell operators
  (`&&`/pipes/`start` forms stay cmd per 205 item 4 and MS `cmd` remarks).
- Honest overlap remains documented: one-liners mixing builtins, `pip`+`python`
  chains, and venv activation (`python -m venv`, `<env>\Scripts\Activate` per
  python.org) belong in cmd/PowerShell; pure-Python `-c` programs may use
  `python3`. Shell-aware identity keeps both choices from colliding.
- If 207/208 were ever cut instead, the replacement is specified: structured
  mapping of cmd's not-recognized output to the same install pointer — accepted
  as strictly weaker (text-sniffing, locale-sensitive), not equivalent.

## Evidence status

Verified against on-disk source via CodeGraph (argv, spawn, Test/stop,
route/candidates, guidance, shell enum, budgets) plus fetched primaries
(python.org launcher semantics, MS `cmd` behavior/wording, Rust spawn-vs-exit
distinction). Assumed, not measured: that launcher-only installs are common
enough for the 207 false-negative to bite (follows from PATH opt-in docs, no
telemetry); cmd exit 9009 for unknown commands is de facto only — no official
MS list was found, so this note promises only non-zero + text. No user study;
no perf claims.
