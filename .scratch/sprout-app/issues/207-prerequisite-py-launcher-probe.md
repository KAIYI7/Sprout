# 207 — Prerequisite py-launcher probe

**Parent:** [205 — Companion picker cap, Python detection + shell, file Download, staging grace (spec)](205-companion-cap-python-download-staging-grace-spec.md)

**What to build:** Sprout stops being blind to the Python launcher: asking whether Python is installed recognizes `py`, probes the launcher first, and reports the same honest verdicts as every other prerequisite.

**Blocked by:** None — can start immediately (extends the complete 199 contract).

**Status:** ready-for-agent

- [ ] `py` joins the detect allow-list and routes to the Python runtime probe; probe order is launcher first, then PATH (`py -3`, then `python`, then `python3`)
- [ ] Verdicts stay `present` (with version token) / `not-found` / `not-verifiable`; 15s-per-probe and 60s-total budgets, read-only boundaries, and install guidance unchanged
- [ ] Frontend alias already maps `py` to Python — backend and frontend agree with no second invocation site (ownership gate passes)
- [ ] Verification: present/not-found/not-verifiable matrix per candidate incl. launcher-first order; backend prereq tests green; supplies 208

**Explicitly not built:**

- Native `python3` shell (ticket 208); AI qualification for it (spec-145 pattern, separate round)