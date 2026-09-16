# 213 — Shared python owner (both OSes)

**Parent:** [206 — Mac Quick-access port, portable Python shell, and dual-train releases (spec)](206-mac-quick-access-port-python-release-spec.md)

**What to build:** One portable Python module both platforms call: Windows resolves via its launcher with PATH fallback, Mac via its own interpreter name, and a missing runtime on either OS produces the same honest not-found outcome. This is the single shared placement in the package — justified by a genuine second adapter plus hidden version-parse complexity.

**Blocked by:** 208 (interface; 211's probe tables serve as prior art, not a gate).

**Status:** ready-for-agent

- [ ] Single `python_argv` + `probe_python` interface serving both OS adapters
- [ ] Windows adapter: launcher-first, PATH fallback; Mac adapter: system interpreter probe (table-tested, no spawning in tests)
- [ ] Missing runtime is a first-class honest outcome (named, with install guidance surfaced by callers) — never a silent failure or a shell fallback
- [ ] Deletion-test note recorded in the ticket: removing this module re-scatters version parsing + fallback order across two callers, so the shared placement earns its keep
- [ ] Verification: adapter-matrix tests green; ownership gate pass (no second invocation site)

**Explicitly not built:**

- Wiring `python3` into the Windows Quick Action shell choices (ticket 214)
- Backup allowlist/version work (tickets 214/216)
