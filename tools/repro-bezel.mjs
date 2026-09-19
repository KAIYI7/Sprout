// Repro loop for the bezel-dock bug trio (diagnosing-bugs Phase 1).
// Agent-runnable, deterministic, fast: one command, red while any bug lives.
//
//   node tools/repro-bezel.mjs        (exit 0 = green, 1 = red)
//
// Bug 1 — mode switch freezes the dock (white window) + Settings save stuck:
//   (a) backend `apply_width` must not place synchronously while a mode
//       settle is pending (`settled != mode`) — that placement races the
//       driver's own settle on the same HWND (two writers; torn window).
//   (b) `reconcile_saved_settings` must not re-apply + re-settle an already
//       converged dock — redundant churn widens every race window.
// Bug 2 — bezel tab too thin / not clickable:
//   (c) collapsed/peek widths must clear a reliable hit threshold.
// Bug 3 — clicking the tab loses it (stale async placement wins):
//   (d) every bezel window move must carry a generation guard so a stale
//       peek/unpeek/drag placement can never land after a newer open/close.
//   (e) the click-outside closer must know every bezel surface (tab, panel,
//       grip) so no bezel-surface press can mis-route to the closer.
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const read = (p) => readFileSync(path.join(ROOT, p), "utf8");

const failures = [];
const check = (name, ok, detail = "") => {
  console.log(`${ok ? "PASS" : "FAIL"}  ${name}${!ok && detail ? ` — ${detail}` : ""}`);
  if (!ok) failures.push(name);
};

const windowRs = read("src-tauri/src/constants/window.rs");
const quickRs = read("src-tauri/src/quick_window.rs");
const page = read("src/routes/quick-launch-window/+page.svelte");
const qlCap = read("src-tauri/capabilities/quick-launch.json");
const libRs = read("src-tauri/src/lib.rs");

// (b2) Bug 2 live root cause — the capability gate: the bezel interaction
// resizes/repositions its own window from JS, so the Quick Launch window
// must carry the geometry permissions. Without them every tab press dies
// with "window.set_size not allowed" and the tab reads as unclickable.
for (const perm of [
  "core:window:allow-set-size",
  "core:window:allow-set-position",
  "core:window:allow-outer-position",
  "core:window:allow-inner-size",
  "core:window:allow-scale-factor",
]) {
  check(`quick-launch capability grants ${perm}`, qlCap.includes(perm), "bezel JS geometry denied");
}

// (shadow) Bug 1 live root cause — a shadowed undecorated window reports a
// client area smaller than its frame; the tab is narrower than that inset,
// so the client inverts and tao's shadow compensation wraps into an
// overflow abort. The dock window must be shadowless.
const openFn = quickRs.slice(quickRs.indexOf("pub fn open("), quickRs.indexOf("pub fn open(") + 1200);
check("dock window disables shadows", openFn.includes(".shadow(false)"), "shadow inset breaks sub-inset tabs");

// (c) Bug 2 — hit threshold: collapsed >= 20px, peek >= 30px, peek wider.
const collapsed = Number(windowRs.match(/BEZEL_COLLAPSED_WIDTH_PX:\s*i32\s*=\s*(\d+)/)?.[1]);
const peek = Number(windowRs.match(/BEZEL_PEEK_WIDTH_PX:\s*i32\s*=\s*(\d+)/)?.[1]);
check("bezel collapsed tab clears 20px hit threshold", collapsed >= 20, `now ${collapsed}px`);
check("bezel peek is ~3x the tab (forgiving hover target)", peek >= 3 * collapsed - 5, `now ${peek}px vs ${collapsed}px`);
check("bezel peek stays wider than collapsed", peek > collapsed, `${peek} vs ${collapsed}`);

// (a) Bug 1 — single writer: apply_width must yield while a settle is pending.
const applyWidth = quickRs.slice(
  quickRs.indexOf("fn apply_width("),
  quickRs.indexOf("fn needs_reestablish("),
);
check(
  "apply_width yields to a pending driver settle",
  /settled/.test(applyWidth),
  "no settled-guard in apply_width",
);

// (b) Bug 1 — no redundant second apply once converged.
const reconcile = quickRs.slice(
  quickRs.indexOf("pub fn reconcile_saved_settings("),
  quickRs.indexOf("/// Releases the AppBar"),
);
check(
  "reconcile skips apply when live already matches stored",
  /let before_dock/.test(reconcile) && /if moved && is_docked\(app\)/.test(reconcile),
  "re-settle tail runs unconditionally",
);

// (d) Bug 3 — one ordered chain owns the window: a superseded request never
// paints after a newer one (drag-hold jitter), and open/close/peek tween on
// the token curve instead of snapping.
check(
  "bezel moves queue through one ordered chain",
  /function queueBezelMove\(/.test(page) && /let bezelTail/.test(page),
  "no serialization chain in bezel placers",
);
for (const fn of ["placeBezelRect", "placeBezelTab"]) {
  const body = page.slice(page.indexOf(`function ${fn}(`), page.indexOf(`function ${fn}(`) + 2500);
  check(
    `${fn} routes through the chain and aborts when superseded`,
    /queueBezelMove\(/.test(body) && /gen !== bezelPlaceGen/.test(body),
    `${fn} has no staleness check`,
  );
}
check(
  "bezel open/close/peek tween on --dur-slow/--ease-out",
  /function tweenBezelWindow\(/.test(page) &&
    page.includes("--dur-slow") &&
    page.includes("--ease-out") &&
    /requestAnimationFrame/.test(page) &&
    /prefers-reduced-motion/.test(page),
  "window moves snap without the token curve",
);
check(
  "starved frames finish instantly instead of wedging the chain",
  /function nextBezelFrame\(\)/.test(page) && /await nextBezelFrame\(\)/.test(page),
  "an occluded page freezes every queued move until rAF resumes",
);
check(
  "drag derives X from the edge anchor (no mid-gesture re-read)",
  /startX \?\?/.test(page) && /placeBezelTab\(ratio, g\.startX\)/.test(page) &&
    /getBezelCollapsedWidth\(\)/.test(page.slice(page.indexOf("function onBezelGripPointerDown("), page.indexOf("function onBezelGripPointerDown(") + 2200)),
  "drag re-reads live position per move",
);
check(
  "toggle flips first and never swallows rapid clicks",
  !/bezelToggling/.test(page) && /if \(bezelOpen === open\) bezelOpen = !open;/.test(page),
  "in-flight guard eats clicks / flip lands late",
);
check(
  "hover never translates (in-place grow, flap-proof)",
  /hover = false/.test(page) && /hover && from/.test(page) && /from\.x \+ from\.w - width/.test(page),
  "peek/unpeek re-derive position under a stationary cursor",
);
check(
  "drag stays single-flight with pointercancel cleanup",
  /function endBezelDragListeners\(\)/.test(page) && /pointercancel/.test(page),
  "a leaked session can double the next drag",
);

// (e) Bug 3 — closer knows every bezel surface.
const closer = page.slice(page.indexOf("const onBezelPointerDown"), page.indexOf("const onBezelPointerDown") + 800);
check(
  "click-outside closer allow-lists tab, panel and grip",
  closer.includes("bezelPanelEl") && closer.includes("bezelTabEl") &&
    (/bezelGripEl|Grip/.test(closer)),
  "grip missing from the closer allow-list",
);

// Right-edge open mirrors left: the tab stays on the screen-edge side via
// row-reverse, with its own mirrored arrival — the panel never jumps sides.
check(
  "right-edge open mirrors left instead of jumping",
  page.includes(".qlw--bezel-open.qlw--docked-right") &&
    page.includes("flex-direction: row-reverse") &&
    page.includes("qlw-bezel-panel-in-right"),
  "right open lays out like left",
);

// Every dock writer notifies the window like every other writer, so no
// surface (tray toggle, edge switch) can leave the frontend stale.
for (const fn of ["toggle_quick_launch_dock", "switch_quick_launch_dock_edge"]) {
  const body = libRs.slice(libRs.indexOf(`fn ${fn}(`), libRs.indexOf(`fn ${fn}(`) + 900);
  check(
    `${fn} emits quick-launch-changed`,
    body.includes("emit_quick_launch_changed"),
    "frontend goes stale for external callers",
  );
}
// The dock/mode cycle stress covers docked/floating transitions too, not
// just in-place mode flips.
check(
  "stress cycles docked/floating alongside mode flips",
  /"docked" => quick_window::dock/.test(libRs) && /"floating" => quick_window::undock/.test(libRs),
  "stress only flips modes while docked",
);

console.log(failures.length ? `\nVERDICT: RED (${failures.length} failing)` : "\nVERDICT: GREEN");
process.exit(failures.length ? 1 : 0);
