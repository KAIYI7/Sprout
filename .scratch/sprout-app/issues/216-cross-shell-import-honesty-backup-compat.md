# 216 — Cross-shell import honesty + backup compat

**Parent:** [206 — Mac Quick-access port, portable Python shell, and dual-train releases (spec)](206-mac-quick-access-port-python-release-spec.md)

**What to build:** Backup and import stay one document format across the shell matrix: `python3` is admitted, foreign shells are refused honestly with a re-author pointer, legacy documents keep their meaning, and nothing is ever auto-translated between shells.

**Blocked by:** 211, 213, 214 (needs all three shell verdicts).

**Status:** ready-for-agent

- [ ] Allowlist/version matrix tested as documents: legacy reads preserved, new shell admitted, unknown shells/versions rejected before merge
- [ ] Importing an action whose shell the current OS cannot run yields an incompatible-shell message naming the needed re-authoring (never executes under the wrong shell)
- [ ] Only `python3` actions roam across OSes; every other cross-shell import is blocked with guidance
- [ ] Machine-local boundary and portable-form stripping unchanged
- [ ] Verification: document-matrix tests green; ownership gate pass

**Explicitly not built:**

- Shell auto-translation in any direction (rejected by decision — no library guarantees it)
