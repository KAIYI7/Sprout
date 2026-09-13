# 152 — Choose a stronger model without automatic replacement

**What to build:** Offer the qualified stronger tier alongside the lightweight model and preserve explicit installed selections across downloads, switches, failures, and later Sprout releases.

**Blocked by:** 151 — managed lightweight setup and bundled resource format.

**Status:** complete — all ACs closed (batch-152-153-194-195-196-20260913, combined validation 2026-09-13). The shipped stronger candidate stays non-installable (no invented pins); the selection/inventory/migration/validation logic rides on fixtures.

**Parent:** [145 — AI-assisted Quick Action authoring](145-ai-assisted-quick-action-authoring-spec.md).

## ACs

- [x] Reuse 151’s verified installation, lifecycle and authoring integration for the stronger compatible model. Selection changes qualified model/configuration. Test per-model context/template requirements and reject compatibility that JSON alone cannot supply.
- [x] Show both qualified tiers with model identity, source/license, download size, and measured suitability. Present the stronger model as a user choice, not an automatic consequence of detected hardware.
- [x] Install and verify the stronger model through the managed flow. Switching is explicit; failed installation or activation retains a usable previous selection and reports the failure without cloud fallback.
- [x] Manage installed inventory separately from the release's recommendation list. A new app release can recommend a different model without downloading it, switching selections, or deleting an installed model.
- [x] An installed model missing from the new recommendations remains identifiable. Incompatibility is reported with deliberate recovery choices; it cannot silently map to a new model ID or corrupt existing inventory.
- [x] Bundled JSON and fixed skills change only with normal app releases/version tags. There is no raw-repository JSON polling, separate catalog server/publication, independent catalog signing, or remote skill update.
- [x] Catalog schema validation rejects duplicate IDs, invalid artifact metadata, unsupported runtime requirements, and executable installation instructions. Exact artifact hashes remain tied to the requested selection.
- [x] Verify an old/new bundled-catalog fixture migration, lightweight-to-stronger switching, insufficient memory, removed recommendations, failed downloads, and offline startup without losing saved Quick Actions.
- [x] Document the maintainer workflow for qualifying a replacement, updating bundled data/resources/notices, and publishing an app release. Record that editing repository JSON alone does not update installed users.

## Verification

Use two qualified artifacts for manual selection tests where hardware permits; use deterministic compatibility/resource fixtures for failures and older/newer app-catalog transitions. Check that default installation remains weight-free.

## Implementation notes

This ticket does not implement remote recommendation updates. Release mechanics remain owned by the established app-update process; do not introduce another signing-key lifecycle.

## Implementation record — 2026-09-13 (batch-152-153-194-195-196-20260913)

Selection/inventory/migration/validation logic with fixtures; the shipped stronger candidate stays `blocked-pending-artifact-pin-and-measurements` (no invented pins, no JSON value changes):

- `Catalog::parse` now fails a catalog with duplicate tier IDs, executable installation instructions (`install_recipe`/`install_script`/`setup_commands`/`post_install`/`executable_install`), a qualified tier without artifact/context/memory/runtime evidence, or a qualified tier pinned to another runtime version (ADR-0032).
- Installed inventory lives apart from the recommendation list: `catalog_status` surfaces installed-but-removed ids as `removed-from-recommendations` (installed, not installable, deliberate recovery blocker); `remove` frees them explicitly; nothing downloads, switches, or deletes automatically across releases.
- Fixture battery: old/new catalog migration, explicit lightweight↔stronger switching, insufficient memory (download size ≠ RAM), removed recommendations, failed stronger download retaining the previous selection, offline/broken-catalog startup with inventory intact.
- Settings review dialog already shows both tiers with identity/source/license/size/suitability; switching is per-model explicit Install; refresh never auto-switches.

Maintainer workflow for qualifying a replacement: qualify the artifact in research (exact revision, SHA-256, byte size, license, template, memory, runtime minimum, Windows authoring/diagnosis measurements) → update `src-tauri/resources/ai-model-recommendations.json` + skill resources/notices → bump the app version and publish a normal release tag. Editing repository JSON alone never updates installed users; installed selections never reinterpret across releases.
