# 187 — Disconnect cloud AI and remove its credentials without losing actions

**What to build:** Let users disconnect the cloud route, remove the stored cloud credential and applicable consent records through their owners, and switch between managed/local and cloud routes — reusing 186's removal UX grammar — while keeping saved Quick Actions intact.

**Blocked by:** 150 — cloud credentials/consent; 186 — removal UX/preview grammar + backup-exclusion shapes (defer to 186's shapes on overlap).

**Status:** ready-for-agent

**Parent:** [145 — AI-assisted Quick Action authoring](145-ai-assisted-quick-action-authoring-spec.md). Cloud half of the [154](154-ai-disconnect-cleanup-and-recovery.md) split (154 is superseded by 186 + 187; local half is [186](186-ai-managed-local-cleanup.md)).

## ACs

- [ ] Disconnecting a cloud account removes its stored credential and applicable consent records through their owners (the credential store owner inventoried by 150). Errors are honest; secrets never appear in logs, error bodies, or whole-app export.
- [ ] Disconnecting the cloud route cancels associated active cloud requests and prevents late cloud results from updating drafts or saved actions.
- [ ] Verify managed↔cloud switching and disconnect, missing-key deletion, and cloud late-completion blocking. 155 re-verifies the combined multi-route set.
- [ ] Add the credential-regression test on top of 186's backup exclusions: once 150 stores keys, backups still export no credentials or provider settings. Do not re-edit 186's exclusion shapes except through 186's owner on overlap.
- [ ] Removal UX reuses 186's grammar (deliberate, previewed, moment-of-use disclosure); this ticket gains no model/runtime/file deletion powers beyond credentials and consent records.

## Explicitly not built

- Managed model/runtime/artifact removal, Off-stops-server, keep/remove downloads (186).
- Revision/error-context session clearing (rides with 153; audited in 155).

## Verification

Use protected credential-store test doubles; assert outbound payloads carry no secret pre-consent and no unapproved redirect (150's transport checks are the prerequisite, not re-proven here). Never write a real user secret in a test.

## Implementation notes

- Likely paths/owners (estimates to recheck against code at dispatch, CodeGraph first): 150's credential/consent owners, `settings.rs` cloud knobs, Settings page disconnect UI. No new Windows-invocation site (ADR-0029 holds).
- Candidate wave 2 — starts after 150; on Settings/backup overlap, 187 defers to 186's shapes and reconciles before closing.
