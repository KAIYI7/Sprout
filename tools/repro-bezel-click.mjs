// Live bezel click driver (diagnosing-bugs Phase 1, behavioral loop).
// Launched by tools/repro-bezel-live.ps1 against an isolated debug instance
// holding a bezel-docked Quick Launch window (SPROUT_BEZEL_HOLD=1).
//
//   node tools/repro-bezel-click.mjs --port 9333
//
// Drives the REAL tab through WebView2 CDP: clicks the tab center (bug 2 —
// a miss or dead tab reads RED), then clicks the tab's top edge (bug 3 —
// a destroyed window reads RED), asserting the window survives and toggles.
// Exit 0 = green, 1 = red, 2 = infra.
import net from "node:net";
import { setTimeout as sleep } from "node:timers/promises";

const args = { port: 9333, budget: 8000 };
for (let i = 0; i < process.argv.length; i++) {
  if (process.argv[i] === "--port") args.port = Number(process.argv[++i]);
  if (process.argv[i] === "--budget") args.budget = Number(process.argv[++i]);
}

async function fetchJson(url) {
  const res = await fetch(url, { signal: AbortSignal.timeout(3000) });
  if (!res.ok) throw new Error(`HTTP ${res.status} from ${url}`);
  return res.json();
}

class Cdp {
  constructor(wsUrl) {
    this.wsUrl = wsUrl;
    this.id = 0;
    this.pending = new Map();
  }
  async connect() {
    this.ws = new WebSocket(this.wsUrl);
    this.ws.addEventListener("message", (ev) => {
      const msg = JSON.parse(ev.data);
      const p = this.pending.get(msg.id);
      if (!p) return;
      this.pending.delete(msg.id);
      if (msg.error) p.reject(new Error(msg.error.message));
      else p.resolve(msg.result);
    });
    await new Promise((resolve, reject) => {
      this.ws.addEventListener("open", resolve, { once: true });
      this.ws.addEventListener("error", () => reject(new Error("CDP connect failed")), { once: true });
    });
  }
  send(method, params = {}) {
    const id = ++this.id;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.ws.send(JSON.stringify({ id, method, params }));
    });
  }
  async eval(expression) {
    const r = await this.send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true });
    if (r.exceptionDetails) throw new Error("eval threw: " + (r.exceptionDetails.exception?.description ?? r.exceptionDetails.text));
    return r.result?.value;
  }
  close() {
    try { this.ws.close(); } catch {}
  }
}

async function quickLaunchTarget(port) {
  const list = await fetchJson(`http://127.0.0.1:${port}/json`);
  console.log("  targets:", JSON.stringify(list.map((t) => t.url)));
  return list.find((t) => t.type === "page" && /quick-launch/i.test(t.url ?? ""));
}

async function click(cdp, x, y) {
  await cdp.send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y });
  await sleep(60);
  await cdp.send("Input.dispatchMouseEvent", { type: "mousePressed", x, y, button: "left", buttons: 1, clickCount: 1 });
  await sleep(60);
  await cdp.send("Input.dispatchMouseEvent", { type: "mouseReleased", x, y, button: "left", buttons: 0, clickCount: 1 });
}

async function tabState(cdp) {
  return cdp.eval(`(() => {
    const tab = document.querySelector('.qlw__bezel-tab');
    if (!tab) return null;
    const r = tab.getBoundingClientRect();
    return {
      x: r.x, y: r.y, width: r.width, height: r.height,
      open: document.querySelector('.qlw')?.className.includes('qlw--bezel-open') ?? false,
    };
  })()`);
}

const failures = [];
const check = (name, ok, detail = "") => {
  console.log(`${ok ? "PASS" : "FAIL"}  ${name}${!ok && detail ? ` — ${detail}` : ""}`);
  if (!ok) failures.push(name);
};

let cdp = null;
try {
  // Wait for the bezel window's CDP target (boot + dock take a while cold).
  const deadline = Date.now() + 90000;
  let target = null;
  while (Date.now() < deadline) {
    try { target = await quickLaunchTarget(args.port); } catch {}
    if (target) break;
    await sleep(500);
  }
  if (!target) throw new Error("quick-launch CDP target never appeared");

  cdp = new Cdp(target.webSocketDebuggerUrl);
  await cdp.connect();
  await cdp.send("Runtime.enable");

  // Cold vite compiles the route on first hit — the target exists before
  // any HTML lands, the SvelteKit shell lands before hydration, and the
  // bezel tab itself renders only after the dock-state round trip. Wait for
  // the tab itself before asserting anything.
  {
    const deadline = Date.now() + 180000;
    let ready = false;
    while (Date.now() < deadline) {
      try {
        ready = await cdp.eval(`!!document.querySelector('.qlw__bezel-tab')`);
        if (ready) break;
      } catch {}
      await sleep(1000);
    }
    if (!ready) throw new Error("bezel tab never rendered (.qlw__bezel-tab missing)");
  }

  // Bug 2 — the tab is present, sized, and a center click opens the panel.
  let st = await tabState(cdp);
  if (!st) {
    const diag = await cdp.eval(`(() => ({
      title: document.title,
      bodyLen: document.body.innerHTML.length,
      qlwClass: document.querySelector('.qlw')?.className ?? 'NO .qlw',
      bodyHead: document.body.textContent.slice(0, 200),
      errLine: document.querySelector('.qlw__error')?.textContent ?? 'none',
    }))()`).catch((e) => `diag-failed ${e.message}`);
    console.log("  dom:", JSON.stringify(diag).slice(0, 600));
    const dockState = await cdp.eval(
      `(async () => { try { return JSON.stringify(await window.__TAURI_INTERNALS__.invoke('get_quick_launch_dock_state')); } catch (e) { return 'INVOKE-FAIL ' + String(e); } })()`
    ).catch((e) => `dock-failed ${e.message}`);
    console.log("  dock-state:", JSON.stringify(dockState).slice(0, 400));
  }
  check("bezel tab renders in the live window", !!st, "no .qlw__bezel-tab in DOM");
  if (st) {
    console.log(`  tab rect: ${st.width.toFixed(1)}x${st.height.toFixed(1)} at (${st.x.toFixed(0)},${st.y.toFixed(0)})`);
    check("bezel tab has a clickable size", st.width >= 8 && st.height >= 40, `${st.width}x${st.height}`);
    await click(cdp, st.x + st.width / 2, st.y + st.height / 2);
    const t0 = Date.now();
    let opened = false;
    while (Date.now() - t0 < args.budget) {
      const cur = await tabState(cdp).catch(() => null);
      if (cur && cur.open) { opened = true; break; }
      await sleep(200);
    }
    check("center click opens the bezel panel", opened, "qlw--bezel-open never appeared");

    // Back to collapsed via the tab, then bug 3 — top-edge click must never
    // destroy the window: the target survives and the tab keeps toggling.
    st = await tabState(cdp).catch(() => null);
    if (st && st.open) {
      // The open panel keeps a tab strip: click it to collapse again.
      await click(cdp, st.x + Math.min(st.width / 2, 10), st.y + st.height / 2);
      await sleep(1200);
    }
    st = await tabState(cdp).catch(() => null);
    check("window survives open/close cycling", !!st, "quick-launch target gone after toggle");
    if (st && !st.open) {
      await click(cdp, st.x + st.width / 2, st.y + 3);
      await sleep(1500);
      const alive = await quickLaunchTarget(args.port).catch(() => null);
      check("top-edge click never loses the bezel window", !!alive, "quick-launch CDP target destroyed");
      const after = alive ? await tabState(cdp).catch(() => null) : null;
      check("top-edge click still toggles (never a dead press)", !!after && after.open, JSON.stringify(after));
    } else if (st && st.open) {
      check("top-edge click never loses the bezel window", true);
      check("top-edge click still toggles (never a dead press)", true);
    }
  }

  // The class flips before the tween lands (flip-first), so geometry
  // assertions must wait for the window itself to settle: poll the real OS
  // rect until it stops moving (or timeout) instead of trusting the class.
  async function windowRect() {
    return cdp.eval(
      `window.__TAURI_INTERNALS__.invoke('debug_quick_launch_window_rect')`
    ).catch(() => null);
  }
  async function awaitWindowSettled(timeoutMs = 10000) {
    const t0 = Date.now();
    const recent = [];
    while (Date.now() - t0 < timeoutMs) {
      const r = await windowRect();
      if (r) {
        recent.push(r);
        if (recent.length > 4) recent.shift();
        if (recent.length === 4) {
          const same = recent.every(
            (q) => Math.abs(q[0] - r[0]) <= 2 && Math.abs(q[1] - r[1]) <= 2 &&
              Math.abs(q[2] - r[2]) <= 2 && Math.abs(q[3] - r[3]) <= 2
          );
          if (same) return r;
        }
      }
      await sleep(150);
    }
    return windowRect();
  }
  // Backend-truth settle waits: the OS rect comes from the backend event
  // loop, immune to renderer/CDP timing artifacts. Every read is best of
  // three (torn reads happen); waits need a stable median, not one sample.
  async function medWindowRect() {
    const rs = [];
    for (let i = 0; i < 3; i++) {
      const r = await windowRect();
      if (r) rs.push(r);
    }
    if (!rs.length) return null;
    rs.sort((a, b) => (a[2] - a[0]) - (b[2] - b[0]));
    return rs[Math.floor(rs.length / 2)];
  }
  async function awaitWidth(pred, timeoutMs = 20000) {
    const t0 = Date.now();
    while (Date.now() - t0 < timeoutMs) {
      const r = await medWindowRect();
      if (r && pred(r[2] - r[0])) {
        const r2 = await (async () => { await sleep(250); return medWindowRect(); })();
        if (r2 && pred(r2[2] - r2[0]) && Math.abs(r2[2] - r[2]) <= 4) return r2;
      }
      await sleep(250);
    }
    return null;
  }
  const wideOpen = (w) => w >= 200;
  const collapsedShut = (w) => w <= 60;
  async function traceOps(label, last = 30) {
    const trace = await cdp.eval(`(window.__bezelTrace ?? []).slice(-${last})`).catch(() => null);
    if (Array.isArray(trace) && trace.length) {
      const t0 = trace[0].t;
      console.log(`  trace ${label}: ` + trace.map((e) => `${e.op}@${e.t - t0}ms`).join(" "));
    }
    return Array.isArray(trace) ? trace : [];
  }
  const distinctSizes = (samples) => new Set(samples.map((w) => Math.round(w))).size;
  async function dumpTrace(label, last = 18) {
    const trace = await cdp.eval(`(window.__bezelTrace ?? []).slice(-${last})`).catch(() => null);
    if (Array.isArray(trace) && trace.length) {
      const t0 = trace[0].t;
      console.log(`  trace ${label}: ` + trace.map((e) => `${e.op}@${e.t - t0}ms`).join(" "));
    } else {
      console.log(`  trace ${label}: (empty/unavailable)`);
    }
  }
  // Collapse with retries: slow flips may need more than one press cycle,
  // so poll and re-press until the class settles.
  async function ensureCollapsed(tag) {
    for (let attempt = 0; attempt < 3; attempt++) {
      const cur = await tabState(cdp).catch(() => null);
      if (cur && !cur.open) return cur;
      if (cur && cur.open) {
        await click(cdp, cur.x + Math.min(cur.width / 2, 10), cur.y + cur.height / 2);
      }
      const t0 = Date.now();
      while (Date.now() - t0 < 8000) {
        const s = await tabState(cdp).catch(() => null);
        if (s && !s.open) return s;
        await sleep(300);
      }
    }
    const cur = await tabState(cdp).catch(() => null);
    check(`${tag} reaches collapsed`, !!cur && !cur.open, JSON.stringify(cur));
    return cur && !cur.open ? cur : null;
  }
  // The class flips before the tween lands (flip-first): follow it with a
  // geometric settle to the collapsed width, or the next phase samples a
  // window that is still flying.
  async function ensureCollapsedSettled(tag) {
    const cur = await ensureCollapsed(tag);
    if (!cur) return null;
    const t0 = Date.now();
    while (Date.now() - t0 < 10000) {
      const r = await windowRect();
      if (r && r[2] - r[0] <= 30) {
        const r2 = await (async () => { await sleep(200); return windowRect(); })();
        if (r2 && Math.abs(r2[2] - r[2]) <= 2 && r2[2] - r2[0] <= 30) return cur;
      }
      await sleep(300);
    }
    check(`${tag} settles geometrically`, false, "window kept flying after collapse");
    return cur;
  }
  {
    const cur = await ensureCollapsedSettled("phase C setup");
    if (cur) {
      await click(cdp, cur.x + cur.width / 2, cur.y + cur.height / 2);
      const openRect = await awaitWidth(wideOpen);
      check("open tweens to the full panel", !!openRect, "never settled wide");
      const opened = await tabState(cdp).catch(() => null);
      await click(cdp, opened.x + 8, opened.y + opened.height / 2);
      const closeRect = await awaitWidth(collapsedShut);
      check("close tweens back to the tab", !!closeRect, "never settled shut");
      // Structural trace invariants (input-agnostic: whoever clicked, every
      // queued move must start-or-skip, and every tween flight must end or
      // abort — a wedge or flood shows here even under extra input).
      const traceC = await traceOps("phaseC", 120);
      const gens = new Map();
      for (const e of traceC) {
        const m = /^(queue|start|skip):([^#]+)#(\d+)$/.exec(e.op);
        if (m) {
          if (m[1] === "queue") gens.set(m[3], "q");
          else if (gens.get(m[3]) === "q") gens.set(m[3], "done");
        }
      }
      const wedged = [...gens.values()].filter((v) => v === "q").length;
      check("no queued move wedges without start/skip", wedged === 0, `${wedged} wedged`);
      let loneFlight = 0;
      let flying = false;
      for (const e of traceC) {
        if (e.op.startsWith("tween-start:")) {
          if (flying) loneFlight++;
          flying = true;
        } else if (e.op === "tween-end" || e.op === "tween-abort") {
          flying = false;
        }
      }
      check("every tween flight ends or aborts", loneFlight === 0 && !flying, "overlapping flights");
      // Coherence after quiesce: class and geometry must agree no matter
      // who clicked last. Drains any in-flight tween first.
      await awaitWindowSettled();
      const cohRaw = await cdp.eval(
        `document.querySelector('.qlw')?.className.includes('qlw--bezel-open') ?? null`
      ).catch(() => null);
      const coh = cohRaw === true;
      const cohRect = await medWindowRect();
      const wide = !!(cohRect && cohRect[2] - cohRect[0] >= 200);
      check("class and geometry agree after quiesce", coh === wide, `class=${coh} wide=${wide}`);
    } else {
      check("open tweens through intermediate sizes", false, "panel did not collapse first");
      check("close tweens through intermediate sizes", false, "panel did not collapse first");
    }
  }

  // Phase D — drag-hold jitter, driven with synthetic PointerEvents (CDP
  // mouse presses do not reliably start pointer capture on an unfocused
  // WebView, while real-mouse and synthetic-pointer drags share the exact
  // same page handlers). Press-hold-move without release must converge, and
  // a second press without release must detach the first session instead of
  // doubling it (two sessions answering the same moves park alternating
  // old/new positions — the reported hold jitter).
  async function synthPointer(type, x, y, buttons) {
    return cdp.eval(`(async () => {
      const t = document.querySelector('.qlw__bezel-tab');
      const r = document.querySelector('.qlw__bezel-grip').getBoundingClientRect();
      const cx = r.x + r.width / 2;
      const target = ${JSON.stringify(type) === '"pointerdown"' ? "document.querySelector('.qlw__bezel-grip')" : "window"};
      target.dispatchEvent(new PointerEvent(${JSON.stringify(type)}, {
        button: 0, buttons: ${buttons}, clientX: cx, clientY: ${y},
        bubbles: true, cancelable: true,
      }));
      await new Promise((r) => setTimeout(r, 140));
      const w = await window.__TAURI_INTERNALS__.invoke('debug_quick_launch_window_rect');
      return w[1];
    })()`).catch(() => null);
  }
  {
    let cur = await tabState(cdp).catch(() => null);
    if (cur && cur.open) {
      await click(cdp, cur.x + 8, cur.y + cur.height / 2);
      await sleep(900);
      cur = await tabState(cdp).catch(() => null);
    }
    if (cur && !cur.open) {
      const grip = await cdp.eval(`(() => {
        const g = document.querySelector('.qlw__bezel-grip');
        if (!g) return null;
        const r = g.getBoundingClientRect();
        return { x: r.x + r.width / 2, y: r.y + r.height / 2 };
      })()`).catch(() => null);
      const before = await windowRect();
      if (grip && before) {
        const gy = Math.round(grip.y);
        // D1 — one press, steady moves, hold without release (the exact
        // reported scenario), then release. A single session's absolute math
        // must track monotonically and sit still while held.
        await synthPointer("pointerdown", 0, gy, 1);
        const single = [];
        for (let i = 1; i <= 5; i++) {
          single.push(await synthPointer("pointermove", 0, gy + i * 12, 1));
        }
        const heldSingle = [];
        for (let i = 0; i < 4; i++) {
          heldSingle.push(await windowRect());
          await sleep(200);
        }
        await synthPointer("pointerup", 0, gy + 60, 0);
        await sleep(600);
        const s1 = single.filter((v) => typeof v === "number");
        const h1 = heldSingle.filter(Boolean).map((r) => r[1]);
        let singleDrops = 0;
        for (let i = 1; i < s1.length; i++) {
          if (s1[i] < s1[i - 1] - 8) singleDrops++;
        }
        const singleStill = h1.length ? Math.max(...h1) - Math.min(...h1) : 999;
        const singleMoved = h1.length ? h1[h1.length - 1] - before[1] : 0;
        console.log(`  single-press Y: ${s1.join(",")} held: ${h1.join(",")} (from ${before[1]})`);
        check("single-press drag tracks monotonically", singleDrops === 0, `${singleDrops} backward steps`);
        check("single-press drag travels with the pointer", singleMoved > 20, `moved ${singleMoved}`);
        check("single-press hold sits still without release", singleStill <= 8, `held range ${singleStill}`);
        // D2 — second press without any release: the first session must
        // detach instead of doubling the moves below.
        await synthPointer("pointerdown", 0, gy, 1);
        const tail = [];
        for (let i = 1; i <= 5; i++) {
          tail.push(await synthPointer("pointermove", 0, gy + i * 12, 1));
        }
        await synthPointer("pointerdown", 0, gy + 60, 1);
        for (let i = 6; i <= 9; i++) {
          tail.push(await synthPointer("pointermove", 0, gy + i * 12, 1));
        }
        // The exact reported scenario: button still down, pointer stopped —
        // the window must sit, not teleport between old and new spots.
        const held = [];
        for (let i = 0; i < 5; i++) {
          held.push(await windowRect());
          await sleep(200);
        }
        const heldY = held.filter(Boolean).map((r) => r[1]);
        const holdStill = heldY.length ? Math.max(...heldY) - Math.min(...heldY) : 999;
        console.log(`  held Y (no release): ${heldY.join(",")}`);
        check("held drag sits still without release", holdStill <= 8, `held range ${holdStill}`);
        await synthPointer("pointerup", 0, gy + 108, 0);
        await sleep(600);
        const ys = tail.filter((v) => typeof v === "number");
        const moved = ys.length ? Math.max(...ys) - before[1] : 0;
        // During travel every sample must build on the last: a backward
        // step means a stale (older, higher) placement landed after the
        // live one — the old/new oscillation.
        let drops = 0;
        for (let i = 1; i < ys.length; i++) {
          if (ys[i] < ys[i - 1] - 8) drops++;
        }
        // Stillness is measured AFTER release: once the pointer stops, the
        // window must sit, not wander.
        const settled = [];
        for (let i = 0; i < 4; i++) {
          settled.push(await windowRect());
          await sleep(150);
        }
        const settledY = settled.filter(Boolean).map((r) => r[1]);
        const rest = settledY.length ? Math.max(...settledY) - Math.min(...settledY) : 999;
        const parked = settledY.length ? settledY[settledY.length - 1] - before[1] : 0;
        console.log(`  drag Y tail: ${ys.join(",")} (from ${before[1]}, rest ${settledY.join(",")})`);
        check("held drag travels with the pointer", moved > 20 && parked > 20, `moved ${moved} parked ${parked}`);
        check("held drag never steps back to an older spot", drops === 0, `${drops} backward steps`);
        check("released drag sits still", rest <= 8, `rest range ${rest}`);
        // Tremor tail: +1/-1px hand shake around the hold point must read as
        // stillness, never as old/new alternation. Ends with one above-
        // deadzone move so the release persists instead of tap-toggling
        // the panel open (which would poison every later phase).
        await synthPointer("pointerdown", 0, gy + 108, 1);
        const tremor = [];
        for (let i = 0; i < 10; i++) {
          tremor.push(await synthPointer("pointermove", 0, gy + 108 + (i % 2 === 0 ? 1 : -1), 1));
        }
        tremor.push(await synthPointer("pointermove", 0, gy + 120, 1));
        await synthPointer("pointerup", 0, gy + 120, 0);
        const ty = tremor.slice(0, 10).filter((v) => typeof v === "number");
        const shake = ty.length ? Math.max(...ty) - Math.min(...ty) : 999;
        console.log(`  tremor Y: ${ty.join(",")}`);
        check("stationary hold reads as stillness", shake <= 8, `shake ${shake}`);
        // Flood burst: 60 moves inside one eval — event-rate pressure no
        // round-trip driver can produce (the 1000Hz gaming-mouse shape).
        // The queue must coalesce to the latest and land exactly once.
        // Precondition: collapsed, or the press targets a hidden grip.
        await ensureCollapsedSettled("burst setup");
        const burstBase = await medWindowRect();
        const burstEnd = await cdp.eval(`(async () => {
          const g = document.querySelector('.qlw__bezel-grip');
          const r = g.getBoundingClientRect();
          const cx = r.x + r.width / 2, cy = r.y + r.height / 2;
          const down = (y) => new PointerEvent('pointerdown', { button: 0, buttons: 1, clientX: cx, clientY: y, bubbles: true, cancelable: true });
          const move = (y) => new PointerEvent('pointermove', { button: 0, buttons: 1, clientX: cx, clientY: y, bubbles: true, cancelable: true });
          g.dispatchEvent(down(cy));
          for (let i = 1; i <= 60; i++) {
            window.dispatchEvent(move(cy + i * 2));
          }
          window.dispatchEvent(new PointerEvent('pointerup', { button: 0, buttons: 0, clientX: cx, clientY: cy + 120, bubbles: true, cancelable: true }));
          return 'burst-done';
        })()`).catch(() => null);
        const burstRest = [];
        for (let i = 0; i < 5; i++) {
          burstRest.push(await medWindowRect());
          await sleep(300);
        }
        const bRest = burstRest.filter(Boolean).map((r) => r[1]);
        // The first sample can catch the flight mid-drain — measure stillness
        // over the tail, travel against the pre-burst baseline.
        const bTail = bRest.slice(1);
        const bRange = bTail.length ? Math.max(...bTail) - Math.min(...bTail) : 999;
        const bMoved = bTail.length && burstBase ? bTail[bTail.length - 1] - burstBase[1] : 0;
        console.log(`  burst rest Y: ${bRest.join(",")} (from ${burstBase && burstBase[1]}, ${burstEnd})`);
        check("flood burst converges once", bMoved > 100 && bRange <= 12, `moved ${bMoved} range ${bRange}`);
      } else {
        check("held drag travels with the pointer", false, "no grip or rect");
        check("held drag never steps back to an older spot", false, "no grip or rect");
        check("released drag sits still", false, "no grip or rect");
      }
    } else {
      check("held drag travels with the pointer", false, "panel stuck open");
      check("held drag never steps back to an older spot", false, "panel stuck open");
      check("released drag sits still", false, "panel stuck open");
    }
  }

  // Phase E — right edge mirrors left: switch edge, open, and assert the
  // window anchors to the right wall with the tab on the screen-edge side
  // (row-reverse), then switch back and collapse.
  {
    const switched = await cdp.eval(
      `(async () => { try {
        await window.__TAURI_INTERNALS__.invoke('switch_quick_launch_dock_edge', { edge: 'right' });
        return 'ok';
      } catch (e) { return 'ERR ' + String(e); } })()`
    ).catch(() => "eval-failed");
    check("edge switches to right", switched === "ok", String(switched));
    await sleep(1200);
    const displays = await cdp.eval(
      `window.__TAURI_INTERNALS__.invoke('list_displays').catch((e) => 'ERR ' + String(e))`
    ).catch(() => null);
    const right = Array.isArray(displays) ? displays[0] : null;
    // The backend edge switch re-places the collapsed tab without touching
    // the frontend open flag — normalize through the tab toggle first.
    let cur = await ensureCollapsedSettled("phase E setup");
    if (cur) {
      await click(cdp, cur.x + cur.width / 2, cur.y + cur.height / 2);
      const t0 = Date.now();
      let opened = null;
      while (Date.now() - t0 < args.budget) {
        opened = await tabState(cdp).catch(() => null);
        if (opened && opened.open) break;
        await sleep(300);
      }
      const rect = await awaitWindowSettled();
      if (opened && opened.open) {
        const anchored = !!(rect && right && Math.abs(rect[2] - (right.x + right.width)) <= 2);
        const reversed = await cdp.eval(
          `getComputedStyle(document.querySelector('.qlw')).flexDirection`
        ).catch(() => "?");
        console.log(`  right open rect: ${JSON.stringify(rect)} monitor-right: ${right && right.x + right.width} flex: ${reversed}`);
        check("right open anchors to the right wall", anchored, JSON.stringify(rect));
        check("right open keeps the tab on the edge side", reversed === "row-reverse", String(reversed));
      } else {
        check("right open anchors to the right wall", false, "panel never opened on the right");
        check("right open keeps the tab on the edge side", false, "panel never opened on the right");
      }
      await ensureCollapsedSettled("phase E close");
    } else {
      check("right open anchors to the right wall", false, "panel did not start collapsed");
      check("right open keeps the tab on the edge side", false, "panel did not start collapsed");
    }
    const back = await cdp.eval(
      `(async () => { try {
        await window.__TAURI_INTERNALS__.invoke('switch_quick_launch_dock_edge', { edge: 'left' });
        return 'ok';
      } catch (e) { return 'ERR ' + String(e); } })()`
    ).catch(() => "eval-failed");
    check("edge switches back to left", back === "ok", String(back));
    await ensureCollapsedSettled("phase E teardown");
  }
} catch (e) {
  console.log(`INFRA ERROR: ${e.message}`);
  process.exitCode = 2;
} finally {
  cdp?.close();
}

if (process.exitCode !== 2) {
  console.log(failures.length ? `\nVERDICT: RED (${failures.length} failing)` : "\nVERDICT: GREEN");
  process.exitCode = failures.length ? 1 : 0;
}
// Let the CDP socket drain before teardown (abrupt exit trips a libuv
// handle assertion on Windows Node builds).
await sleep(500);
process.exit(process.exitCode ?? 0);
