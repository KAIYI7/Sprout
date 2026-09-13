# 189 — Managed Start/Stop with running status

**What to build:** Start/Stop the active managed model without touching provider, with a live Running/Stopped indicator that tells the user which action applies.

**Blocked by:** None — can start immediately (assumes 151 lifecycle + 186 Off-grammar; coordinates files with 190/191/193 via 197).

**Status:** ready-for-agent

**Parent:** [188 — Managed local lifecycle UX](188-managed-local-lifecycle-ux-spec.md).

## ACs

- [x] Same control morphs Start↔Stop (Run-accent/Stop-danger); Stop calls the existing stop path, keeps `provider=managed` + `ai_model`, cancels owned requests; Start pre-warms via the health path so metrics read live immediately.
- [x] Status line under `Managed local model` knob only when `managed` + active installed: green `Running — {Tier} · up {m}` + Stop vs gray `Stopped` + Start; static dot, no pulse; full model identity stays in Details.
- [x] `Off` behavior unchanged (stops server + cancels, keeps downloads, Save-deferred); Generate after Stop lazily restarts; no provider mutation by Start/Stop.
- [x] Cheap status (~5s) polls only while Settings open; zero when closed; lifecycle owner unchanged (no second runtime owner).

## Verification

Deterministic lifecycle fakes: stop-keeps-config, Generate-restarts, status reflects process state, Off-path intact; existing managed + Settings suites green; `npm.cmd run check` + `cargo check/test` as affected + ownership gate.

## Implementation notes

Estimates to recheck at dispatch (CodeGraph first): managed lifecycle owner, status-command extension, Settings managed knob. No new Windows-invocation site. Shared contract (running/stopped + Stop-keeps-provider) supplies 190/191.
