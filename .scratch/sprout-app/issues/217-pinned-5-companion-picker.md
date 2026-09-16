# 217 — Pinned-5 Companion picker

**Parent:** [206 — Mac Quick-access port, portable Python shell, and dual-train releases (spec)](206-mac-quick-access-port-python-release-spec.md)

**What to build:** The dock Companion picker stays a fast one-click surface no matter how many sites are saved: it shows the user's 5 pinned sites in their order plus a management row, while the main-app Companion manager remains the unlimited home for all sites.

**Blocked by:** None — can start immediately (independent; evidence: research 0024).

**Status:** ready-for-agent

- [ ] Dock picker renders at most 5 pinned sites in user order with the active site marked, plus a `Manage in Sprout…` row; no search bar or lazy loading inside the dock
- [ ] Pin/reorder/add/edit live in the main app; unpinned sites are main-app-only until re-pinned — nothing is deleted by the cap
- [ ] Saved list itself stays unbounded in settings/backup
- [ ] Verification: picker-model tests green (0/1/5/6+/unnamed-site cases), keyboard + narrow-dock check per research 0024

**Explicitly not built:**

- Main-app list filter/virtualization (deferred until real lists approach ~15)
- Any change to the single-site lifecycle, isolation, or docked-only visibility
