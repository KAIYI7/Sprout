# 211 — Staged-dir unconditional grace

**Parent:** [205 — Companion picker cap, Python detection + shell, file Download, staging grace (spec)](205-companion-cap-python-download-staging-grace-spec.md)

**What to build:** Handed-off opens stop racing the cleanup: the per-run staged directory always lingers past shell exit, so a cold-starting viewer or player finds its file whether or not a child process was observed.

**Blocked by:** None — can start immediately (needs only the 179 lifetime).

**Status:** ready-for-agent

- [ ] Release always lingers past shell exit even with no observed children (repairs the observe-then-grace TOCTOU hole); drain-then-handoff behavior retained when children exist
- [ ] Non-file runs provably unchanged (no placeholder means no staging, no lingering); orphan sweep and 24h ceiling untouched
- [ ] Cold-start opens verified for a document (pdf) and audio (mp3) handler: staged path resolves after shell exit
- [ ] Verification: unit tests through the existing grace parameter (immediate-release case now lingers, drain case unchanged); backend suite + ownership gate green

**Explicitly not built:**

- Quoting/expansion changes; new viewer-launch policy; navigation handling (171)