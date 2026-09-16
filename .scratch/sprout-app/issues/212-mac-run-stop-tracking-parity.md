# 212 — Mac run/stop/tracking parity

**Parent:** [206 — Mac Quick-access port, portable Python shell, and dual-train releases (spec)](206-mac-quick-access-port-python-release-spec.md)

**What to build:** Mac Quick Actions behave like their Windows counterparts from the user's perspective: hidden runs as the current user with no elevation, a live Run-becomes-Stop lifecycle, per-run logs, and honest reporting of detached commands — reusing the shared tracking interface, not duplicating it.

**Blocked by:** 211 (needs the argv/probe contract).

**Status:** ready-for-agent

- [ ] Run → Stop/Stopping state machine matches the documented Windows behavior (ADR-0017 model)
- [ ] Stop runs the action's own stop command when configured, else terminates the process tree; hung stops are force-ended at the watchdog box
- [ ] Every tracked run writes its log with header + exit line; foreground-only tracking (detached commands honestly report not-running)
- [ ] No new module: implementation lives inside the Mac execution module behind internal seams; the external interface from 208 is unchanged
- [ ] Verification: seam-level lifecycle tests green (mirroring the Windows tracking matrix); ownership gate pass

**Explicitly not built:**

- Python discovery (ticket 213)
- UI surfaces (they consume the same run-state events already shared)
