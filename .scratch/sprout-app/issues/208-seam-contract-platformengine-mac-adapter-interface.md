# 208 — Seam contract: PlatformEngine Mac adapter interface + owner map

**Parent:** [206 — Mac Quick-access port, portable Python shell, and dual-train releases (spec)](206-mac-quick-access-port-python-release-spec.md)

**What to build:** The contract every later Mac ticket builds against: a small, documented execution interface (intent in, outcome out) with a working test adapter proving the seam, plus an owner map assigning each OS operation to exactly one owning module. No production behavior changes.

**Blocked by:** 207 (needs the version contract deciding which package builds which tag).

**Status:** ready-for-agent

- [ ] Execution interface defined at the existing platform seam (execute/detect/stop intents; outcomes; error modes) — small surface, behavior behind it
- [ ] Test adapter implements the interface and is exercised by tests through the seam (not past it)
- [ ] Owner map written: each Windows operation keeps its single owner; each planned Mac operation names its future owner (no code moves yet)
- [ ] No shared placement introduced (one Mac adapter is a hypothetical seam until later tickets prove two)
- [ ] Verification: full existing suite green; zero Windows behavior change; ownership gate pass

**Explicitly not built:**

- Any `cfg` gating or code relocation (ticket 209)
- Gate/size-harness changes (ticket 210)
- Executor behavior (tickets 211–214)
