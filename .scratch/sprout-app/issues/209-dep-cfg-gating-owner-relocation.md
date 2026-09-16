# 209 — Dep cfg-gating + owner relocation (exclusion)

**Parent:** [206 — Mac Quick-access port, portable Python shell, and dual-train releases (spec)](206-mac-quick-access-port-python-release-spec.md)

**What to build:** Platform-exclusive compilation: the Windows-only operations and their OS libraries are compiled only for Windows, an empty Mac execution stub only for Mac, so neither binary links the other's OS code and the Windows binary stays within budget of baseline.

**Blocked by:** 208 (gates what gets gated, per the owner map).

**Status:** ready-for-agent

- [ ] Windows-only OS dependencies moved under target-gated dependencies (no unconditional Windows libs in the shared graph)
- [ ] Windows execution/winget/engine owners gated to Windows; empty Mac execution stub gated to Mac
- [ ] Exclusion proof: per-target compile checks pass and Windows binary size diffed against pre-change baseline (no growth beyond noise)
- [ ] Owner map from 208 updated to reflect the relocation; deletion test holds (deleting an owner re-scatters its invocation knowledge across callers)
- [ ] Verification: backend suite green; ownership gate pass

**Explicitly not built:**

- Any Mac executor behavior (tickets 211–213)
- Gate/size-harness extension (ticket 210)
- Frontend route exclusion (ticket 215)
