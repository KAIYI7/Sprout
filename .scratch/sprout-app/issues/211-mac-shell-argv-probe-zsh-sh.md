# 211 — Mac shell argv/probe module (zsh/sh)

**Parent:** [206 — Mac Quick-access port, portable Python shell, and dual-train releases (spec)](206-mac-quick-access-port-python-release-spec.md)

**What to build:** Mac Quick Actions in the native shells resolve to the right invocation and environment probes without spawning anything in tests: one small argv/probe interface hiding quoting, PATH lookup, and version parsing, owned solely by the Mac execution module.

**Blocked by:** 208 (interface), 209 (exclusion in place).

**Status:** ready-for-agent

- [ ] One `argv(shell, command)` + `probe()` interface: `zsh` default, `sh` accepted; quoting and lookup hidden behind it (depth: small surface, real behavior)
- [ ] Probe tables (shell present / absent / wrong version) driven without spawning processes
- [ ] Unknown shell values fail honestly instead of falling back to another shell
- [ ] Mac-only compilation: none of this module reaches the Windows build (gate 210 stays green)
- [ ] Verification: seam-level tests green; backend suite green; ownership gate pass

**Explicitly not built:**

- Run/stop tracking lifecycle (ticket 212)
- Python discovery (ticket 213)
- Any Windows-side change
