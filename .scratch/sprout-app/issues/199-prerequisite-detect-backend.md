# 199 — Prerequisite detect backend (read-only, single owner)

**What to build:** A read-only prerequisite check the AI skill and dialog can trust: ask whether X is installed and get back `present` / `not-found for X` / `not-verifiable` — without running any draft text, without network, without changing the machine.

**Blocked by:** None — can start immediately. Supplies 200/203. Makes no skill-template or dialog edits (200 owns those).

**Status:** complete - all ACs closed (batch-199-201-202-20260914, combined validation 2026-09-14).

**Parent:** [198 — Prerequisite + offload + motion (spec)](198-prerequisite-offload-motion-spec.md). Extends [ADR-0032](../../../../docs/adr/0032-model-recommendations-and-skills-ship-with-app.md) detection surface; ADR-0029 ownership, ADR-0030/0031 boundaries unchanged.

## ACs

- [x] One detect command owned by the process/shell invocation owner — no second Windows-invocation site (ownership gate passes).
- [x] Closed v1 source list only: PATH lookup, winget install state, runtime `--version` probes (node, python), Playwright probe, editor-extension probe.
- [x] Per-prerequisite verdict: `present` (with version where meaningful), `not-found for X` (check actually ran), `not-verifiable` (offline/timeout/uncertain — never a guess).
- [x] Timeboxed and cancellable; timeout reads as `not-verifiable`, never as absent.
- [x] Zero script-execution calls across detect/validate/cancel (startup of managed runtime + fixed read-only discovery remain distinguished, as before).
- [x] Deterministic tests: present/not-found/not-verifiable matrix per source + timeout path; relevant backend checks/tests green.

## Implementation notes

- Sources: existing timed-capture helper + `winget/authoring` search/show patterns as prior art; 184 fixture style for verdict matrix. Claims above are estimates to recheck against code at dispatch (CodeGraph first).

## Verification

- Backend suite + ownership gate green; verdict-shape contract documented for 200 (field names, version semantics, timebox value).

## Validation 2026-09-14 (coordinator, batch-199-201-202-20260914, candidate-1 + warning repairs)

Checks (all in `C:\Sprout`, post-repair source):
- `node tools/ownership-gate.mjs`: pass (62 owned references). Detect lives in
  `src-tauri/src/windows_execution/prereqs.rs`, probes run through the owner's
  `run_timed_process` — no second invocation site.
- `cargo check`: 1 warning only, pre-existing `windows_execution/files.rs::quote_staged_path`
  (byte-identical to baseline, owned by ticket 137).
- `cargo test windows_execution::prereqs`: 10/10 pass.
- Full backend suite: 681 passed, 0 failed.
- `npm.cmd run check`: 0 errors. `npm.cmd run test`: 298 passed.

Verdict-shape contract for 200 (as shipped):
- Rust: `PrerequisiteVerdict { name: String, status: PrereqStatus, version: Option<String>, detail: String }`
  (`src-tauri/src/windows_execution/prereqs.rs:68-72`); `PrereqStatus = Present | NotFound | NotVerifiable`.
- TS seam: `PrerequisiteStatus = "present" | "not-found" | "not-verifiable"`;
  `PrerequisiteVerdict { name, status, version: string | null, detail }`
  (`src/lib/types.ts:730-740`); `detectPrerequisites(names)` in `src/lib/api.ts`.
- Version semantics: `version` is set only for `present`-with-version; null for
  PATH-only presence and every non-present verdict.
- Timebox: `DETECT_PER_PROBE_TIMEOUT = 15s`, `DETECT_TOTAL_BUDGET = 60s`;
  any timeout / spawn failure / exhausted budget reads as `not-verifiable`.
- Closed v1 key catalog: `node`, `python`, `playwright`, `winget:<id>`,
  `extension:<id>`, or a PATH executable name.
- No-execution proof: `zero_script_execution_calls_across_detect` test oracles
  every spawned argv against the fixed `DETECT_PROBE_ALLOW_LIST`; no separate
  `cancel_detect` command (per-probe process-tree kill at the 15s box + 60s
  budget + async abandon; `cancelled` flag plumbed for 200 if the dialog needs it).
