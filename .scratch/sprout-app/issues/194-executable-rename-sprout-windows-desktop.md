# 194 — Executable rename to sprout-windows-desktop (exe-only)

**What to build:** Rename the shipped process to `sprout-windows-desktop.exe` without forking the product, data, update, or identity surfaces.

**Blocked by:** None — can start immediately.

**Status:** complete — all ACs closed (batch-152-153-194-195-196-20260913, combined validation 2026-09-13).

**Parent:** [188 — Managed local lifecycle UX](188-managed-local-lifecycle-ux-spec.md).

## ACs

- [x] `[package] name → sprout-windows-desktop`; keep `[lib] name = sprout_lib`, `productName=Sprout`, `identifier=com.sprout.app`, `%LOCALAPPDATA%\Sprout` data dir, display strings, updater `SETUP_ASSET_PREFIX=Sprout_` + workflow glob semantics.
- [x] Worker relaunch uses the current executable (no hardcoded old name); autostart registration rewrites to the new path; NSIS `MAINBINARYNAME` migration (old-binary delete + shortcut retarget) verified; stale Run-entry behavior documented.
- [x] CI release flow (version bump → build → sign → publish `Sprout_*_x64-setup.exe` + `.sig`) + self-update accept/reject + signature verification green on the renamed tree; repro scripts + test/doc strings updated.
- [x] Full product rename explicitly excluded (no uninstall-key/shortcut/Run orphans, no asset-prefix break, no identifier change).

## Verification

Release-process dry checks: glob/signer/asset-pattern assertions, updater fixture battery, worker/autostart tests, installer migration review; `cargo check/test` + ownership gate; no local installer hand-build (releases stay CI-built per `docs/agents/releases.md`).

## Implementation notes

Estimates to recheck at dispatch: `Cargo.toml`/`Cargo.lock`, `tauri.conf.json` (no product change), `release.yml`, `update.rs` + fixtures, `nsis/installer.nsi`, `worker.rs`/`autostart.rs`/`shell.rs`, `tools/repro-*`, `docs/release/release-process.md` if wording names the exe.
