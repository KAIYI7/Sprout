# 195 — Discord presence brand assets

**What to build:** Attach the shipped brand art to presence with static STE-clean hover texts.

**Blocked by:** None — can start immediately (assumes 182 payload/seam; portal upload is a manual prerequisite before landing).

**Status:** complete — all ACs closed (batch-152-153-194-195-196-20260913, combined validation 2026-09-13).

**Parent:** [188 — Managed local lifecycle UX](188-managed-local-lifecycle-ux-spec.md).

## ACs

- [x] Manual prerequisite first: upload `rp_large.png` + `rp_small.png` to portal app `1548219927264235580` → Rich Presence → Art Assets → Save (keys `rp_large`/`rp_small`).
- [x] Wire `.assets(large_image="rp_large" large_text="Sprout" small_image="rp_small" small_text="Ready")`; static only, ≤128 chars, no user data; `buttons/party/secrets/urls/activity_type` stay absent; silent-absent + exit-clear unchanged.
- [x] Update the payload-shape test that asserts `assets` absent; keep the no-token/user-ID assertions.
- [x] Append dated ADR-0033 amendment (proposed text): assets-in-payload with the two fixed hover strings; file-allowed→payload-attached; dynamic/per-section hover explicitly still banned. Original + prior amendments untouched.

## Verification

Fake-IPC payload-exactness with assets + hover strings; Discord-absent silent no-op; no OAuth/token/user-ID; startup never blocked; exit clears; backend suites + ownership gate green.

## Implementation notes

Estimates to recheck at dispatch: presence owner module only. No Settings toggle, no per-screen text (those need their own amendments).
