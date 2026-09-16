# 218 — Release + verification round (both installers)

**Parent:** [206 — Mac Quick-access port, portable Python shell, and dual-train releases (spec)](206-mac-quick-access-port-python-release-spec.md)

**What to build:** The demonstrable end of the package: real installers for both platforms from the dual-train process — including a joint multi-tag release from one commit — with per-platform updates offered correctly, size budgets met, and all docs closed out. This ticket is the integration owner for spec 206.

**Blocked by:** 207–217 (all tickets above).

**Status:** ready-for-agent

- [ ] Win-only tag, Mac-only tag, and joint tags on one commit each produce the correct Release(s); no phantom updates on either side
- [ ] Updater offers only the platform's own train (matrix-verified without shipping the wrong asset)
- [ ] Size budgets met on both platforms; ownership gate pass; backend + frontend suites green
- [ ] Docs closed out: release-process current, ADR-0012 amendment landed, spec-206 acceptance boxes checked, parent status updated
- [ ] Integration-owner duties: reconcile the workspace/tag/feed contract, overlapping doc edits, and combined verification; publish through verified sync

**Explicitly not built:**

- Any new product behavior (this round verifies; fixes found here become follow-up tickets)
