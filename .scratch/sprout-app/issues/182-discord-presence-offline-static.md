# 182 — Discord presence backend (offline static)

**What to build:** Sprout attempts a static offline Discord Rich Presence on every run via the handwired IPC crate, silently doing nothing when Discord is closed. No account access, no Settings UI in v1.

**Blocked by:** None — can start immediately. Requests the Discord Application ID from the user (see ACs).

**Status:** done — user-verified live 2026-09-12 (Discord running → presence visible)

**Parent:** [181 — Discord + AI two-view + clarify (spec)](181-discord-presence-ai-two-view-clarify-spec.md). New decision [ADR-0033](../../../docs/adr/0033-discord-rich-presence-offline-static.md).

## Scope

- New single-owner presence module (the only Discord IPC site per ADR-0029) + app setup/exit wiring + crate dependency. No Tauri wrapper plugin.
- Handwire per crate + official docs: client constructed with the Application ID, `connect()` → `set_activity(static)` → reconnect-with-backoff, `clear/close` on actual exit (tray-only close is not exit). Never blocks startup or window open.
- Static v1 payload only: `details: "Using Sprout"`, `state: "Composing presets"`. No names/paths/counts/run-states, no art assets, no Join buttons.
- Explicitly not built: Settings toggle, per-action/dynamic presence, images, any OAuth/token/user-ID handling, any second IPC site, any window-size change.

## ACs

- [x] **Requests the Application ID:** user supplied `1548219927264235580` on 2026-09-12; hardcoded as `presence::APPLICATION_ID` (not Settings-editable/backed-up/exported). No secret, redirect URI, or OAuth step requested or stored.
- [x] Discord running → presence appears with exactly the static details/state; Discord closed/absent → app starts/runs normally with no toast, dialog, or error; local debug log only; reconnect attempted with backoff. (Verified via deterministic fake-IPC loop tests; live Discord-visible confirmation left to the user.)
- [x] Proof of no account access: no OAuth scopes, no token storage, no user-ID read; outbound IPC payload is the static activity only (asserted at the seam with a fake IPC: `only_the_static_pair_ever_crosses_the_seam` + serialized-shape test).
- [x] Actual app exit clears/closes presence (`RunEvent::Exit → presence::shutdown`); main-window close-to-tray does not (no hook in the close path). Single-flight guard: overlapping starts never spawn duplicate loops (second `start()` is a no-op).
- [x] `node tools/ownership-gate.mjs` passes; size budget holds (only new crate is `discord-rich-presence` v1.1.0; all 7 transitive deps already in-tree — exe delta is the crate itself, negligible; no release binary built in this unit).

## Implementation notes

- Research applied: crate `DiscordIpcClient::new → connect → set_activity → close` over the local `discord-ipc-*` pipe; desktop client required; activity-sharing must be on client-side. Conventions: version in `Cargo.toml` only; `codebase-design` seam language; deletion test for the module boundary.
- Claims above are estimates to recheck against code at dispatch (CodeGraph first).

## Results (2026-09-12)

- New module `src-tauri/src/presence.rs` (sole Discord IPC owner): `APPLICATION_ID` / `DETAILS` / `STATE` constants, `static_activity()`, `start()` (background connect→set + 30 s heartbeat + capped 1/2/5/10 s backoff, silent `eprintln`-only failures), `shutdown()` (stop flag + synchronous clear for real exit). Internal `Ipc` seam with real + fake adapters.
- Wiring: `mod presence` in `lib.rs`; `presence::start()` in setup (never blocks); `presence::shutdown()` in `RunEvent::Exit` only.
- Verification: `cargo check` clean (0 warnings), `cargo test` 612 passed / 0 failed (10 new presence tests), ownership gate pass. Frontend untouched. Handoff scope (prompts/skills/ai_assist parsing/fixtures) untouched — pinned qualification evidence unaffected.
- Live user verification 2026-09-12: Discord running → presence visible with the static details/state; ticket closed on that confirmation.

## Verification

- Deterministic fake-IPC tests for set/clear/reconnect/silent-fail + payload-exactness + zero-token assertions; relevant existing backend checks/tests; frontend check/build unaffected (or run if touched); ownership gate before sync.

## Follow-up — 2026-09-13 (continuous total elapsed clock, text updates independently)

Process note: appended here per owner rule — amend the existing presence
ticket instead of opening a new one unless completely irrelevant.

**Change:** Discord Rich Presence shows a continuous total elapsed time: a
single `started at` timestamp set once when the app launches/connects, never
reset. `details`/`state` text updates independently as the user's activity
changes within the app — every `set_activity()` call reuses the same original
start timestamp so the elapsed clock keeps counting instead of resetting to
`0:00` when the activity message changes.

**Requires before implementation:** dated `## Amendment` to
[ADR-0033](../../../docs/adr/0033-discord-rich-presence-offline-static.md) —
v1 explicitly promises no `timestamps` and no dynamic per-screen text
(`static_payload_sets_no_optional_sections` locks it). Dynamic text also needs
its disclosure review: which activities map to which fixed strings, and proof
no preset/action names, paths, counts, or run states leak.

## ACs (follow-up — done 2026-09-13)

- [x] Start timestamp is captured once at launch/first-connect and reused on
  every `set_activity()` (including heartbeat re-asserts and reconnects) —
  changing `details`/`state` never resets the elapsed clock.
- [x] Elapsed clock survives Discord restarts/reconnects within the same app
  run (pipe drop → retry → re-set keeps the original start, not `now()`).
- [x] Fake-IPC regression test: two consecutive sets with different
  `details`/`state` carry the identical start timestamp.
- [x] ADR-0033 amendment + spec-181 reconciliation ship in the same unit of
  work (no silent overwrite of the static-only promise).

## Results (follow-up 2026-09-13)

- `src-tauri/src/presence.rs` (still the sole Discord IPC owner): session
  clock (`SESSION_START_MS`, captured in `start()` at launch), current section
  text (`CURRENT`, validated non-blank / 128 chars max), `activity_for()` pure
  in the start, `set_status()` stores + pushes one immediate set (Discord
  absent stays a silent no-op), loop snapshots text per set against the pinned
  start. Old static-only helpers removed; old loud `timestamps` refusal
  replaced by a start-presence + still-no-assets/buttons/party/secrets shape
  test.
- Wiring: `set_presence_status` Tauri command (registered in `lib.rs`
  `invoke_handler`); `src/lib/api.ts` wrapper; `src/routes/+layout.svelte`
  reports the main-window section from a fixed route map (default
  `"Composing presets"`), one fire per section, failures silent. No user
  content crosses — fixed vocabulary only.
- Decision: dated `## Amendment — 2026-09-13` in ADR-0033 (original + 2026-09-12
  amendment untouched).
- Verification: `cargo check` 0 warnings, `cargo test` 617 passed / 0 failed
  (13 presence tests, incl. `consecutive_sets_reuse_the_original_start_when_text_changes`
  and reconnect-keeps-start), `npm.cmd run check` 0 errors (2 pre-existing
  warnings in untouched `QuickActionFormDialog.svelte`), ownership gate pass.
  Live Discord-visible elapsed clock left to the user.
