// Bisect the dead drag: is the grip press delivered, and does a synthetic
// PointerEvent drag move the tab? Distinguishes input-delivery gaps
// (unfocused WebView2 eats synthetic presses) from logic gaps.
import { setTimeout as sleep } from "node:timers/promises";

const port = Number(process.argv[2] ?? 9333);
const res = await fetch(`http://127.0.0.1:${port}/json`, { signal: AbortSignal.timeout(5000) });
const list = await res.json();
const target = list.find((t) => t.type === "page" && /quick-launch/i.test(t.url ?? ""));
if (!target) { console.log("NO TARGET"); process.exit(2); }
const ws = new WebSocket(target.webSocketDebuggerUrl);
await new Promise((resolve, reject) => {
  ws.addEventListener("open", resolve, { once: true });
  ws.addEventListener("error", () => reject(new Error("ws failed")), { once: true });
});
let id = 0;
const pending = new Map();
ws.addEventListener("message", (ev) => {
  const msg = JSON.parse(ev.data);
  const p = pending.get(msg.id);
  if (!p) return;
  pending.delete(msg.id);
  msg.error ? p.reject(new Error(msg.error.message)) : p.resolve(msg.result);
});
const send = (method, params = {}) => new Promise((resolve, reject) => {
  const i = ++id;
  pending.set(i, { resolve, reject });
  ws.send(JSON.stringify({ id: i, method, params }));
});
const evaluate = async (expression) => {
  const r = await send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true });
  if (r.exceptionDetails) return "EVAL-EXC";
  return r.result?.value;
};
await send("Runtime.enable");
const rectOf = (sel) => evaluate(`(() => {
  const el = document.querySelector('${sel}');
  if (!el) return null;
  const r = el.getBoundingClientRect();
  return { x: r.x, y: r.y, w: r.width, h: r.height };
})()`);
const rect = () => evaluate(`window.__TAURI_INTERNALS__.invoke('debug_quick_launch_window_rect')`).catch(() => null);

for (let i = 0; i < 120; i++) {
  if (await evaluate(`!!document.querySelector('.qlw__bezel-grip')`).catch(() => false)) break;
  await sleep(1000);
}
const grip = await rectOf(".qlw__bezel-grip");
console.log("grip:", JSON.stringify(grip));
const gx = Math.round(grip.x + grip.w / 2), gy = Math.round(grip.y + grip.h / 2);
console.log("topmost-at-grip-center:", await evaluate(`document.elementFromPoint(${gx}, ${gy})?.className ?? 'none'`));
const y0 = await rect();
console.log("Y before:", JSON.stringify(y0));

// Synthetic PointerEvent drag (no CDP input pipeline involved).
const ySynth = await evaluate(`(async () => {
  const g = document.querySelector('.qlw__bezel-grip');
  const r = g.getBoundingClientRect();
  const cx = r.x + r.width / 2, cy = r.y + r.height / 2;
  const opts = (y) => ({ button: 0, buttons: 1, clientX: cx, clientY: y, bubbles: true, cancelable: true });
  g.dispatchEvent(new PointerEvent('pointerdown', { ...opts(cy), buttons: 1 }));
  for (let i = 1; i <= 5; i++) {
    window.dispatchEvent(new PointerEvent('pointermove', opts(cy + i * 10)));
    await new Promise((r) => setTimeout(r, 120));
  }
  window.dispatchEvent(new PointerEvent('pointerup', { ...opts(cy + 50), buttons: 0 }));
  return 'done';
})()`).catch((e) => `failed ${e.message}`);
await sleep(800);
console.log("synthetic drag:", ySynth, "Y after:", JSON.stringify(await rect()));
ws.close();
await sleep(500);
process.exit(0);
