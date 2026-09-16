# 214 — Windows python3 shell

**Parent:** [206 — Mac Quick-access port, portable Python shell, and dual-train releases (spec)](206-mac-quick-access-port-python-release-spec.md)

**What to build:** Windows users can choose `python3` alongside PowerShell/cmd for a Quick Action; Test, Run, and Stop all flow through the shared Python owner with the same timeout/kill-tree policy, and a missing runtime explains itself with an install pointer.

**Blocked by:** 213 (needs the shared interface). Follows the ticket-147 shell-field shape as prior art.

**Status:** ready-for-agent

- [ ] `python3` offered as a shell choice with validation; legacy records keep their meaning
- [ ] Run/stop/test route through the shared owner — no new invocation site (single-execution-owner rule holds)
- [ ] Probe order launcher-first then PATH; missing runtime surfaces honestly in Test and Run
- [ ] Same hidden/unelevated/tracked/logged behavior as existing shells
- [ ] Verification: shell-matrix tests green (including missing-runtime case); backend suite green; ownership gate pass

**Explicitly not built:**

- Backup version/allowlist evolution (ticket 216)
- AI authoring qualification for the new shell (follows the spec-145 pattern separately)
