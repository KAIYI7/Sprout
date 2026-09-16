# 210 — Gate + size harness for cross-OS exclusion

**Parent:** [206 — Mac Quick-access port, portable Python shell, and dual-train releases (spec)](206-mac-quick-access-port-python-release-spec.md)

**What to build:** An automated guard so future tickets cannot silently leak one platform's operations into the other's build: the ownership gate fails on cross-OS references, and per-platform size budgets are asserted in CI.

**Blocked by:** 208 (needs the owner map to guard).

**Status:** ready-for-agent

- [ ] Ownership-gate rule: a Windows-owned invocation reachable from Mac-gated code (and vice versa) fails with a message naming the expected owner
- [ ] Failing-first test: a probe cross-reference is rejected by the gate
- [ ] Per-platform binary + frontend-bundle size asserts wired (Windows budget unchanged; Mac budget recorded)
- [ ] Guard documented next to the gate so later tickets know the rule before they trip it
- [ ] Verification: gate pass on the clean tree; suite green

**Explicitly not built:**

- No production code changes of any kind
