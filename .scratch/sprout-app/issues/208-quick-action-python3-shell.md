# 208 — Quick Action python3 shell

**Parent:** [205 — Companion picker cap, Python detection + shell, file Download, staging grace (spec)](205-companion-cap-python-download-staging-grace-spec.md)

**What to build:** Windows users can choose `python3` alongside PowerShell/cmd for a Quick Action; Test, Run, and Stop all flow through the single Windows execution owner with the same hidden/unelevated/tracked/logged policy, and a missing runtime explains itself with an install pointer.

**Blocked by:** 207 (needs the launcher-first probe order). Follows the 147 shell-field shape as prior art.

**Status:** ready-for-agent

- [ ] `python3` offered as a shell choice with validation; legacy records keep their meaning; unknown shells fail honestly
- [ ] Run/stop/test route through the shared owner (new argv builder beside the existing shell argvs) — no new invocation site; stop uses the same shell
- [ ] Missing runtime surfaces honestly in Test and Run with the install pointer; Sprout vendors no runtime, so updates change only the reported version
- [ ] Backup carries the shell per the 147 envelope-v2 contract; legacy round-trips unchanged
- [ ] Verification: shell-matrix tests green (including missing-runtime case); backup round-trip; backend suite + ownership gate green

**Explicitly not built:**

- Backup version evolution beyond 147; AI authoring qualification for the new shell