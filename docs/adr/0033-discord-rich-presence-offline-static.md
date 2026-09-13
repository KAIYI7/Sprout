# Offline static Discord Rich Presence via handwired IPC (no accounts)

> Status: amended 2026-09-13 — delivered in ticket 182 under spec 181 (static presence + session clock + fixed section text; see dated amendments). Original decision text preserved.

Sprout shows a static offline Rich Presence activity while it runs by handwiring the `discord-rich-presence` crate directly over Discord's local IPC. No Tauri wrapper plugin, no OAuth, no account access, no user-information read. This keeps the fully-offline posture (AI cloud mode excepted) while giving Discord users a lightweight "Playing …" status.

## Decisions

- Handwire `discord-rich-presence` (`DiscordIpcClient::new(APPLICATION_ID)` → `connect()` → `set_activity(Activity::new().details().state())` → `close()`). The crate speaks only to the local IPC pipe (`discord-ipc-*`); it performs no network, OAuth, or user-identity exchange.
- Static v1 payload only: `details: "Using Sprout"`, `state: "Composing presets"`. No user content in presence — no preset/action names, paths, counts, or run states. No art assets, Join buttons, or dynamic per-screen text in v1.
- Always attempt on app start (including tray-only boot) on a background thread; never block startup or window open. Discord absent/closed = silent no-op with local debug log and reconnect-with-backoff. No toast, dialog, or error surface. Clear/close on actual app exit; main-window close-to-tray is not exit.
- One hardcoded Application ID constant (placeholder until the user supplies the real numeric ID in ticket 182). Not Settings-editable, not backed up, not exported. No client secret, redirect URI, or token anywhere.
- Single ownership per ADR-0029: the new presence module is the sole Discord IPC site. Crate dependencies are minimal (`serde`/`uuid` class, already in-tree); the NFR-43 size budget is unaffected beyond a negligible delta recorded at delivery.

## Considered options

- A Tauri wrapper plugin was rejected: an extra abstraction over a three-call IPC surface with no lifecycle benefit, against the handwire-per-docs instruction and the single-owner rule.
- A Settings toggle (default off, per research 0008 app-global placement) was considered and deferred: v1 has no switch by explicit user scope ("not relevant") — closing Discord is the opt-out. A future toggle, if requested, belongs in Settings per 0008 rule 1 and needs its own amendment.
- Dynamic presence (current screen, counts, action names) was rejected for v1: any user-derived string risks leaking machine-local names into a public profile and breaks the no-user-info promise without a separate disclosure review.

## Consequences

- Requires the Discord desktop client with activity sharing enabled; web/mobile alone shows nothing. Users without Discord see zero behavior change.
- The Application ID is a release-blocking input: placeholder code cannot show presence until the real ID lands. No presence text may carry user data without a new amendment.
- AI assistance boundaries (ADR-0030/0031/0032) are untouched; presence sends no prompts, discovery results, or diagnostics.

## Amendment — 2026-09-12

Art assets ship after all, at the user's explicit direction: the portal app
icon (`src-tauri/icons/app_icon_1024.png`, byte-identical to the 1024
`app-icon.png` render) plus Rich Presence large/small art (`rp_large.png`
fully opaque brand tile, `rp_small.png` mark-only on transparency with a
doubled stem for ~24 px badge legibility). All three render from the
`app-icon.svg` source of truth through the owned `tools/render-icon.mjs`
pipeline, so no new brand geometry exists to drift. The v1 payload promise
is unchanged — static text only, still no OAuth, tokens, user-ID reads,
Join buttons, or dynamic per-screen text; images carry the fixed mark, no
user content.

## Amendment — 2026-09-13

Continuous session clock plus fixed section text, at the user's explicit
direction. The v1 static-only promise above is superseded in two narrow ways;
everything else (local IPC only, no OAuth, tokens, user-ID reads, Join
buttons, assets, party, secrets) still holds:

- One `timestamps.start` per process: captured at app launch
(`presence::start` reads the session clock before spawning the loop) and
reused on every `set_activity` — heartbeat re-asserts, section changes, and
reconnects — so the elapsed timer measures the whole run and never resets to
`0:00` when the message changes. A late Discord connect still shows total
elapsed since launch.
- `details` stays `"Using Sprout"`; `state` follows the main-window section
through a fixed allowlist (route map owned by the layout, default
`"Composing presets"`). Section changes store validated text (non-blank, 128
chars max) and push one immediate set on top of the heartbeat pickup. Still no
user-derived content — no preset/action names, paths, counts, or run states —
so the no-user-info posture is unchanged, only the fixed vocabulary grew.

## Amendment — 2026-09-13 (round delivery; spec 181, ticket 185)

Delivered in 182 with the 2026-09-13 follow-up: new single-owner module
`src-tauri/src/presence.rs` (sole Discord IPC site per ADR-0029) plus
setup/exit wiring and `discord-rich-presence` v1.1.0 dependency —
`APPLICATION_ID` hardcoded (`1548219927264235580`, user-supplied, not
Settings-editable/backed-up/exported), static `details`/`state` plus session
`timestamps.start` captured once at launch and reused on every set
(heartbeat re-asserts, section changes, reconnects), `set_presence_status`
command with fixed-vocabulary route map, background connect→set loop with
capped backoff, silent no-op when Discord is closed, `clear/close` on actual
exit only (close-to-tray untouched), single-flight start guard. Proof of no
account access asserted at the seam (fake-IPC payload-exactness +
zero-token tests). Size budget holds (only new crate is the IPC client; all
transitive deps already in-tree — exe delta is the crate itself, negligible;
no release binary built in this unit). User-verified live 2026-09-12
(Discord running → presence visible). Original plus 2026-09-12/13 text
untouched.

## Amendment — 2026-09-13 (brand assets in payload)

The 2026-09-12 file allowance becomes a payload attachment, at the user's
explicit direction: every set now carries static `assets`
(`large_image: "rp_large"` with hover `"Sprout"`, `small_image: "rp_small"`
with hover `"Ready"`), resolving to the portal-uploaded art for app
`1548219927264235580` (manual Rich Presence → Art Assets upload with keys
`rp_large`/`rp_small`, saved before landing — code cannot substitute for it).
Both hover strings are fixed literals, far under the 128-char cap, carrying
no user data; `buttons`/`party`/`secrets`/urls/`activity_type` stay absent,
and silent-absent plus exit-clear behavior is unchanged. Dynamic or
per-section hover text is explicitly still banned: the vocabulary stays two
static strings, so the no-user-info posture is unchanged — only the fixed
payload grew.
