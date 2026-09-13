# 186 — Disable or remove owned managed/local AI resources without losing actions

**What to build:** Let users disable managed/local AI, disconnect the managed or existing-local route, revoke discovery access, and remove specifically selected Sprout-owned inference resources — stopping the owned server and releasing its memory — while keeping saved Quick Actions and user-managed installations intact.

**Blocked by:** 151 — managed runtime (complete). No cloud behavior; no revision flow.

**Status:** ready-for-agent

**Parent:** [145 — AI-assisted Quick Action authoring](145-ai-assisted-quick-action-authoring-spec.md). Local half of the [154](154-ai-disconnect-cleanup-and-recovery.md) split (154 is superseded by 186 + 187; cloud half is [187](187-ai-cloud-disconnect-cleanup.md)).

## ACs

- [x] Managed resource removal is fixed trusted Sprout behavior over explicitly selected owned artifacts, never an AI-generated cleanup script or general deletion tool exposed to inference. It creates no exception for destructive AI authoring.
- [x] Disabling managed/local AI or disconnecting the managed or existing-local route cancels associated active requests and prevents late results from updating drafts or saved actions. Manual editing and Run/Stop of saved actions still work without a provider.
- [x] Revoking a discovery root invalidates related pending target references and prevents subsequent access. Closing/discarding the authoring session clears transient prompts and generation candidate state; no hidden persistent conversation archive remains.
- [x] Provide deliberate removal of chosen managed model/runtime artifacts with a preview of owned targets and released space where known. Validate actual ownership/paths, stop only the owned runtime when needed, and never recursively delete a user-managed installation.
- [x] When applying Off to managed AI, stop the Sprout-owned inference server and release its model memory in addition to cancelling requests. Offer an explicit choice to keep installed downloads for later reuse or remove them; Off alone must not silently delete files. Keep the existing Settings Save/Discard behavior and make removal a deliberate, previewed action.
- [x] The removal preview and operation cover the selected model weights, Sprout-owned inference server, bundled runtime libraries/dependencies, and associated app-owned download archives, partial downloads and caches. Retain artifacts required by any managed model the user keeps. Removing all managed models can remove the entire owned inference installation. Do not remove system GPU drivers, shared system prerequisites, or independently installed local services/models; preserve saved Quick Actions.
- [x] Interrupted cleanup, runtime crash, missing/corrupt artifacts, or stale inventory produce recoverable state without deleting saved actions or treating a partial installation as ready. An ordinary app update does not perform unsolicited model removal.
- [x] Backups contain saved Quick Action command/shell content but no credentials, provider settings, discovery grants/maps, transient prompts, model files, or skills. The credential/provider-settings exclusions pass vacuously until a cloud route exists (no such values are stored by this ticket); 187 adds the credential regression once 150 stores keys. Normal backup/import behavior for unrelated collections is preserved.
- [x] Keep existing general app-uninstall/data-retention policy; do not silently expand the uninstaller's deletion scope or alter the default preservation of user data.
- [x] Verify managed/local disable/disconnect, active-request cancellation, root revocation, late completion, interrupted managed removal, and foreign-resource preservation — including both keep/remove paths, absence of a running owned server after Off, complete removal of exclusively owned runtime dependencies, and preservation of dependencies needed by retained models. Document operational metadata retention/redaction and bounded transient-data behavior.

## Explicitly not built

- Cloud credential/consent removal, cloud-route disconnect, multi-route switching, missing-key deletion (187, blocked by 150).
- Revision/error-context session clearing and late-revision blocking (rides with 153; audited in 155).

## Verification

Use isolated owned-resource fixtures plus targeted benign Windows checks where needed. Assert exact ownership and surviving saved actions; never test removal against a real user-managed model directory. Model privacy and ownership claims must remain true during failures, not only successful requests.

Follow-up delivered: the Model details dialog shows the installed folder (`installed_dir` on the model view) with a Copy path action, so the user can find — and, with the server stopped, manually delete — exactly what Sprout owns per model; a hand-deleted model reads back as not installed.

## Implementation notes

- Likely paths/owners (estimates to recheck against code at dispatch, CodeGraph first): `ai_managed.rs` (`shutdown`/`reap_idle`/`stop_runtime`/install manifest — the automatic lifecycle this ticket's deliberate Off/removal builds on), `settings.rs` AI knobs, Settings page removal UI, `backup.rs` exclusion shapes (owned by this ticket; 187 defers to them), `lib.rs` Tauri commands. No new Windows-invocation site (ADR-0029 holds).
- UX grammar set here and reused by 187: moment-of-use owned-target/space preview on removal request (research 0004 rule 2, 0007), never a permanent artifact listing in Settings (2026-09-11 clarification applies to this ticket, not 154).
- Candidate wave 1 — can start immediately; all behavioral prerequisites are complete/implemented.
