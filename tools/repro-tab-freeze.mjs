import { spawn, spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import fs from "node:fs";
import os from "node:os";
import net from "node:net";
import path from "node:path";
import { setTimeout as sleep } from "node:timers/promises";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

const USAGE = `Usage: node tools/repro-tab-freeze.mjs [options]

Drives the Sprout app via CDP and asserts every nav-tab click lands.
Verdict RED when a click never lands within budget or a main-thread
stall gap exceeds the threshold. Exit code: 0=green 1=red 2=infra.

Options:
  --mode exe|dev   app under test (default exe; dev spawns npm run tauri dev)
  --reps N         full nav sweeps (default 1)
  --budget ms      per-click landing budget (default 5000)
  --stall ms       stall threshold (default 1000)
  --delay ms       settle delay between clicks (default 400)
  --sample ms      stall sampler interval (default 100)
  --target path    click only this tab once (default: full sweep)
  --targets a,b,c  click these tabs in order, once each
  --ipc-probe      time the list IPC round trips on boot, then exit
  --port P         CDP port (default: first free of 9222,9333,9444,9555,9666)
  --keep           leave the app running after the verdict (debugging only)
  --large-plan N   seed N synthetic requirements across --large-presets presets,
                   then time compute_plan plus the nav sweep with them present
                   (default 0 = skip the seeded phase)
  --large-presets K  preset split for --large-plan (default 6)
  --large-backup N write N synthetic backup records to a temp file, then time
                   inspect_backup on it (default 0 = skip; the file is deleted
                   afterwards, the Library is never imported into)
  --seed-prefix S  id prefix for seeded rows (default "repro-large"); a clash
                   with existing rows aborts instead of touching user data
  --attach         drive the already-running app on --port (default 9222) via
                   its remote-debugging endpoint instead of spawning a second
                   app; never kills the app afterwards (for a live dev stack
                   whose vite port a fresh spawn could not bind)
  --cleanup-only   with --attach: delete every --seed-prefix row and exit
                   without measuring (leak recovery, no verdict)
  --help`;

function parseArgs(argv) {
  const args = { mode: "exe", reps: 1, budget: 5000, stall: 1000, delay: 400, sample: 100, port: null, target: null, targets: null, ipcProbe: false, keep: false, largePlan: 0, largePresets: 6, largeBackup: 0, seedPrefix: "repro-large", attach: false, cleanupOnly: false };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    switch (a) {
      case "--help": console.log(USAGE); process.exit(0);
      case "--keep": args.keep = true; break;
      case "--mode": args.mode = argv[++i]; break;
      case "--reps": args.reps = Number(argv[++i]); break;
      case "--budget": args.budget = Number(argv[++i]); break;
      case "--stall": args.stall = Number(argv[++i]); break;
      case "--delay": args.delay = Number(argv[++i]); break;
      case "--sample": args.sample = Number(argv[++i]); break;
      case "--target": args.target = argv[++i]; break;
      case "--targets": args.targets = argv[++i].split(","); break;
      case "--ipc-probe": args.ipcProbe = true; break;
      case "--port": args.port = Number(argv[++i]); break;
      case "--large-plan": args.largePlan = Number(argv[++i]); break;
      case "--large-presets": args.largePresets = Number(argv[++i]); break;
      case "--large-backup": args.largeBackup = Number(argv[++i]); break;
      case "--seed-prefix": args.seedPrefix = argv[++i]; break;
      case "--attach": args.attach = true; break;
      case "--cleanup-only": args.cleanupOnly = true; break;
      default: throw new Error(`unknown option: ${a}\n${USAGE}`);
    }
  }
  return args;
}

const SWEEP = ["/presets", "/plan", "/history", "/logs", "/settings", "/"];

async function pickFreePort(preferred) {
  const candidates = preferred ? [preferred, 9333, 9444, 9555, 9666] : [9222, 9333, 9444, 9555, 9666];
  for (const port of candidates) {
    const free = await new Promise((resolve) => {
      const srv = net.createServer();
      srv.once("error", () => resolve(false));
      srv.once("listening", () => srv.close(() => resolve(true)));
      srv.listen(port, "127.0.0.1");
    });
    if (free) return port;
  }
  throw new Error("no free CDP port among " + candidates.join(","));
}

async function fetchJson(url) {
  const res = await fetch(url, { signal: AbortSignal.timeout(2000) });
  if (!res.ok) throw new Error(`HTTP ${res.status} from ${url}`);
  return res.json();
}

class Cdp {
  constructor(wsUrl) {
    this.wsUrl = wsUrl;
    this.id = 0;
    this.pending = new Map();
    this.events = [];
  }

  async connect() {
    this.ws = new WebSocket(this.wsUrl);
    this.ws.addEventListener("message", (ev) => {
      const msg = JSON.parse(ev.data);
      if (msg.id) {
        const p = this.pending.get(msg.id);
        if (!p) return;
        this.pending.delete(msg.id);
        if (msg.error) p.reject(new Error(msg.error.message));
        else p.resolve(msg.result);
      } else {
        this.events.push(msg);
      }
    });
    await new Promise((resolve, reject) => {
      this.ws.addEventListener("open", resolve, { once: true });
      this.ws.addEventListener("error", () => reject(new Error("CDP websocket connect failed")), { once: true });
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
    const r = await this.send("Runtime.evaluate", {
      expression,
      returnByValue: true,
      awaitPromise: true,
    });
    if (r.exceptionDetails) {
      throw new Error(
        "eval threw: " +
          (r.exceptionDetails.exception?.description ?? r.exceptionDetails.text),
      );
    }
    return r.result?.value;
  }

  close() {
    try {
      this.ws.close();
    } catch {}
  }
}

function launch(mode, port) {
  const env = {
    ...process.env,
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port}`,
  };
  if (mode === "dev") {
    const child = spawn("cmd.exe", ["/c", "npm.cmd run tauri dev"], {
      cwd: ROOT,
      env,
      stdio: ["ignore", "ignore", "pipe"],
    });
    const tail = [];
    child.stderr.on("data", (d) => {
      tail.push(d.toString());
      if (tail.length > 20) tail.shift();
    });
    child.tail = tail;
    return child;
  }
  return spawn(path.join(ROOT, "dist", "Sprout.exe"), [], {
    cwd: ROOT,
    env,
    stdio: "ignore",
  });
}

function killTree(child) {
  try {
    spawnSync("taskkill", ["/PID", String(child.pid), "/T", "/F"], { stdio: "ignore" });
  } catch {}
}

async function waitForPageTarget(port, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let lastErr = null;
  while (Date.now() < deadline) {
    try {
      const list = await fetchJson(`http://127.0.0.1:${port}/json`);
      const target = list.find(
        (t) =>
          t.type === "page" &&
          t.url !== "about:blank" &&
          !t.url.startsWith("devtools://"),
      );
      if (target) return target;
    } catch (e) {
      lastErr = e;
    }
    await sleep(250);
  }
  throw new Error(`no CDP page target on :${port} within ${timeoutMs}ms (${lastErr ?? "no endpoint"})`);
}

function startSampler(cdp, intervalMs) {
  const samples = [];
  const timer = setInterval(() => {
    const wallSend = Date.now();
    cdp
      .eval("performance.now()")
      .then((perf) => samples.push({ wallSend, wallRecv: Date.now(), perf }))
      .catch(() => {});
  }, intervalMs);
  timer.unref();
  return {
    samples,
    stop() {
      clearInterval(timer);
    },
  };
}

function stallsFrom(samples, thresholdMs) {
  const stalls = [];
  let episode = null;
  for (const s of samples) {
    const latency = s.wallRecv - s.wallSend;
    if (latency > thresholdMs) {
      if (!episode) episode = { from: s.wallSend, to: s.wallRecv, max: latency };
      else {
        episode.to = s.wallRecv;
        episode.max = Math.max(episode.max, latency);
      }
    } else if (episode) {
      stalls.push(episode);
      episode = null;
    }
  }
  if (episode) stalls.push(episode);
  return stalls;
}

async function clickTab(cdp, href, budgetMs, stallMs, sampleMs, delayMs, report) {
  const rect = await cdp.eval(`(() => {
    const a = document.querySelector('a.rail__item[href=${JSON.stringify(href)}]');
    if (!a) return null;
    const r = a.getBoundingClientRect();
    return { x: r.x + r.width / 2, y: r.y + r.height / 2 };
  })()`);
  if (!rect) throw new Error(`nav anchor not found for ${href}`);

  const t0 = Date.now();
  const sampler = startSampler(cdp, sampleMs);
  try {
    await cdp.send("Input.dispatchMouseEvent", {
      type: "mouseMoved",
      x: rect.x,
      y: rect.y,
    });
    await sleep(30);
    await cdp.send("Input.dispatchMouseEvent", {
      type: "mousePressed",
      x: rect.x,
      y: rect.y,
      button: "left",
      buttons: 1,
      clickCount: 1,
    });
    await sleep(30);
    await cdp.send("Input.dispatchMouseEvent", {
      type: "mouseReleased",
      x: rect.x,
      y: rect.y,
      button: "left",
      buttons: 0,
      clickCount: 1,
    });

    let last = null;
    while (Date.now() - t0 < budgetMs) {
      try {
        last = await cdp.eval(`(() => ({
          path: location.pathname,
          active: document.querySelector('.rail__item.active')?.getAttribute('href') ?? null
        }))()`);
        if (last && last.path === href && last.active === href) {
          const landMs = Date.now() - t0;
          const stalls = stallsFrom(sampler.samples, stallMs);
          report.clicks.push({ href, start: t0, landMs, ok: true, stalls, state: last });
          return;
        }
      } catch {}
      await sleep(40);
    }
    const landMs = Date.now() - t0;
    const stalls = stallsFrom(sampler.samples, stallMs);
    report.clicks.push({ href, start: t0, landMs, ok: false, stalls, state: last });
  } finally {
    sampler.stop();
  }
  await sleep(delayMs);
}

function extractLog(cdp) {
  const out = [];
  for (const ev of cdp.events) {
    if (ev.method === "Runtime.exceptionThrown") {
      const d = ev.params.exceptionDetails;
      out.push({
        kind: "exception",
        text: d.exception?.description ?? d.text,
        url: d.url,
        line: d.lineNumber,
      });
    } else if (ev.method === "Runtime.consoleAPICalled") {
      const text = ev.params.args
        .map((a) => a.value ?? a.description ?? a.type)
        .join(" ");
      out.push({ kind: "console", type: ev.params.type, text });
    } else if (ev.method === "Log.entryAdded") {
      const e = ev.params.entry;
      out.push({ kind: "log", level: e.level, text: e.text, url: e.url });
    }
  }
  return out;
}

function verdict(report, stallMs, budgetMs) {
  const bad = report.clicks.filter(
    (c) => !c.ok || c.landMs > budgetMs || c.stalls.length > 0,
  );
  const heavyBad = (report.heavy ?? []).filter((h) => !h.ok || h.stalls.length > 0);
  return { red: bad.length > 0 || heavyBad.length > 0, bad, heavyBad };
}

// WHY fixtures go through the UI-owned create/delete commands: the empty
// first-run decision (No first-run seed) stays intact — seeded rows are
// deliberate data like any user entry, and cleanup below removes them, so a
// run never leaves a warm Library behind.
function planFixture(prefix, stamp, totalReqs, presetCount) {
  const policies = [{ kind: "latest" }, { kind: "pinned", version: "9.9.9" }, { kind: "present" }];
  const products = [];
  for (let i = 0; i < totalReqs; i++) {
    products.push({
      id: `${prefix}-prod-${i}`,
      name: `Repro Large Product ${stamp} ${i}`,
      winget_id: `ReproNonexistent.Vendor${i}`,
      install_location_hint: null,
      install_dir: null,
      default_env: [],
    });
  }
  const per = Math.ceil(totalReqs / presetCount);
  const presets = [];
  for (let k = 0; k < presetCount; k++) {
    const reqs = [];
    for (let j = k * per; j < Math.min((k + 1) * per, totalReqs); j++) {
      const p = products[j];
      reqs.push({
        product: { ...p },
        step: { type: "winget", id: p.winget_id, scope: "machine" },
        version_policy: policies[j % policies.length],
        depends_on: [],
        timeout_minutes: 10,
        env: [],
        verify: [],
      });
    }
    if (reqs.length === 0) break;
    presets.push({
      id: `${prefix}-preset-${k}`,
      schema_version: 1,
      platform: "windows",
      name: `Repro Large Preset ${stamp} ${k}`,
      description: "Synthetic repro fixture, removed after the run.",
      author: "repro-tab-freeze",
      version: "1",
      requirements: reqs,
      imported: false,
    });
  }
  return { products, presets };
}

// WHY the version-2 kind-tagged envelope: the single backup document format
// (One backup document format) — inspect_backup covers the read/parse/validate
// half on the blocking pool, which is the stall-relevant half; the merge half
// is never exercised here so the Library is untouched.
function backupFixture(prefix, stamp, total) {
  const nProducts = Math.floor(total * 0.4);
  const nClips = Math.floor(total * 0.25);
  const nLaunch = Math.floor(total * 0.15);
  const nActions = Math.floor(total * 0.15);
  const nPresets = Math.max(0, total - nProducts - nClips - nLaunch - nActions);
  const products = [];
  for (let i = 0; i < nProducts; i++) {
    products.push({
      id: `${prefix}-bprod-${i}`,
      name: `Repro Large Backup Product ${stamp} ${i}`,
      winget_id: `ReproNonexistent.Backup${i}`,
      install_location_hint: null,
      install_dir: null,
      default_env: [],
    });
  }
  const presets = [];
  for (let k = 0; k < nPresets; k++) {
    const pid = `${prefix}-bprod-${k % Math.max(1, nProducts)}`;
    presets.push({
      id: `${prefix}-bpreset-${k}`,
      schema_version: 1,
      platform: "windows",
      name: `Repro Large Backup Preset ${stamp} ${k}`,
      description: "Synthetic repro fixture, never imported.",
      author: "repro-tab-freeze",
      version: "1",
      requirements: [
        {
          product: {
            id: pid,
            name: `Repro Large Backup Product ${stamp} ${k % Math.max(1, nProducts)}`,
            winget_id: `ReproNonexistent.Backup${k % Math.max(1, nProducts)}`,
            install_location_hint: null,
            install_dir: null,
            default_env: [],
          },
          step: { type: "winget", id: `ReproNonexistent.Backup${k % Math.max(1, nProducts)}`, scope: "machine" },
          version_policy: { kind: "latest" },
          depends_on: [],
          timeout_minutes: 10,
          env: [],
          verify: [],
        },
      ],
      imported: false,
    });
  }
  const launch_entries = [];
  for (let i = 0; i < nLaunch; i++) {
    launch_entries.push({
      name: `Repro Large Entry ${stamp} ${i}`,
      kind: "command",
      target: `cmd.exe /c echo repro-large-${i}`,
      shell: "none",
      show_window: false,
      desktop_id: null,
      show_in_dock: true,
    });
  }
  const quick_actions = [];
  for (let i = 0; i < nActions; i++) {
    quick_actions.push({
      name: `Repro Large Action ${stamp} ${i}`,
      shell: "powershell",
      command: `Write-Host repro-large-${i}`,
      cwd: null,
      stoppable: false,
      stop_command: null,
      auto_run: false,
      show_in_dock: true,
    });
  }
  const clips = [];
  for (let i = 0; i < nClips; i++) {
    clips.push({
      name: `repro large clip ${stamp} ${i}`,
      content: `repro-large clip body ${stamp} ${i} — synthetic fixture text`,
      show_in_dock: true,
    });
  }
  return {
    kind: "sprout-backup",
    version: 2,
    exported_at: Math.floor(Date.now() / 1000),
    products,
    presets,
    launch_entries,
    quick_actions,
    quick_action_files: [],
    clips,
  };
}

async function invokeInPage(cdp, cmd, payload) {
  return cdp.eval(`(async () => {
    const invoke = window.__TAURI_INTERNALS__.invoke;
    return await invoke(${JSON.stringify(cmd)}, ${JSON.stringify(payload ?? {})});
  })()`);
}

async function librarySnapshot(cdp, prefix) {
  return cdp.eval(`(async () => {
    const invoke = window.__TAURI_INTERNALS__.invoke;
    const products = await invoke("list_products", { query: null });
    const presets = await invoke("list_presets");
    const clashes = [
      ...products.map((p) => p.id).filter((id) => id.startsWith(${JSON.stringify(prefix)})),
      ...presets.map((p) => p.id).filter((id) => id.startsWith(${JSON.stringify(prefix)})),
    ];
    return { products: products.length, presets: presets.length, clashes };
  })()`);
}

async function timeHeavy(cdp, label, sampleMs, stallMs, probe) {
  const sampler = startSampler(cdp, sampleMs);
  const t0 = Date.now();
  try {
    const summary = await probe();
    const stalls = stallsFrom(sampler.samples, stallMs);
    return { label, ok: true, ms: Date.now() - t0, stalls, summary };
  } catch (e) {
    const stalls = stallsFrom(sampler.samples, stallMs);
    return { label, ok: false, ms: Date.now() - t0, stalls, error: String(e && e.message ? e.message : e) };
  } finally {
    sampler.stop();
  }
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  // WHY no free-port probe in attach mode: the port names someone else's live
  // endpoint — probing "free" would pick the wrong one and drive a stranger.
  const port = args.attach ? (args.port ?? 9222) : await pickFreePort(args.port);
  const bootTimeout = args.attach ? 30_000 : args.mode === "dev" ? 600_000 : 30_000;

  console.log(
    `repro-tab-freeze mode=${args.attach ? "attach" : args.mode} port=${port} reps=${args.reps} budget=${args.budget}ms stall=${args.stall}ms delay=${args.delay}ms sample=${args.sample}ms largePlan=${args.largePlan} largePresets=${args.largePresets} largeBackup=${args.largeBackup}`,
  );

  const child = args.attach ? null : launch(args.mode, port);
  const report = { clicks: [], heavy: [] };
  let cdp = null;
  let infraError = null;

  try {
    const tBoot = Date.now();
    const target = await waitForPageTarget(port, bootTimeout);
    console.log(`boot: ${((Date.now() - tBoot) / 1000).toFixed(1)}s target=${target.url}`);

    cdp = new Cdp(target.webSocketDebuggerUrl);
    await cdp.connect();
    await cdp.send("Runtime.enable");
    await cdp.send("Page.enable");
    await cdp.send("Log.enable");

    const tRail = Date.now();
    while (Date.now() - tRail < 20_000) {
      try {
        if (await cdp.eval(`!!document.querySelector('.rail')`)) break;
      } catch {}
      await sleep(100);
    }
    if (!(await cdp.eval(`!!document.querySelector('.rail')`))) {
      throw new Error("nav rail never appeared");
    }
    const settled = await cdp.eval(
      `new Promise(r => setTimeout(() => r(document.querySelector('.rail__item.active')?.getAttribute('href') ?? null), 1500))`,
    );
    console.log(`app ready, active tab=${settled}`);

    if (args.cleanupOnly) {
      if (!args.attach) throw new Error("--cleanup-only needs --attach (it deletes rows in a live app)");
      const rows = await cdp.eval(`(async () => {
        const invoke = window.__TAURI_INTERNALS__.invoke;
        const products = await invoke("list_products", { query: null });
        const presets = await invoke("list_presets");
        return {
          productIds: products.map((p) => p.id).filter((id) => id.startsWith(${JSON.stringify(args.seedPrefix)})),
          presetIds: presets.map((p) => p.id).filter((id) => id.startsWith(${JSON.stringify(args.seedPrefix)})),
        };
      })()`);
      for (const id of rows.presetIds) {
        await invokeInPage(cdp, "delete_preset", { id });
      }
      for (const id of rows.productIds) {
        await invokeInPage(cdp, "delete_product", { id });
      }
      console.log(`cleanup-only: removed ${rows.presetIds.length} presets, ${rows.productIds.length} products with prefix ${JSON.stringify(args.seedPrefix)}`);
      process.exitCode = 0;
      cdp.close();
      await sleep(100);
      process.exit(0);
    }

    if (args.ipcProbe) {
      const names = ["list_products", "list_presets", "list_runs", "get_settings", "list_logs"];
      for (const name of names) {
        const ms = await cdp.eval(`(async () => {
          const t0 = performance.now();
          await window.__TAURI_INTERNALS__.invoke(${JSON.stringify(name)});
          return Math.round(performance.now() - t0);
        })()`);
        console.log(`ipc ${name.padEnd(16)} ${ms}ms`);
      }
    } else {
      const seed = { presetIds: [], productIds: [], backupPath: null, before: null };
      try {
        if (args.largePlan > 0 || args.largeBackup > 0) {
          const stamp = Date.now().toString(36);
          seed.before = await librarySnapshot(cdp, args.seedPrefix);
          if (seed.before.clashes.length > 0) {
            throw new Error(
              `seed prefix ${JSON.stringify(args.seedPrefix)} already present (${seed.before.clashes.length} rows) — refusing to touch existing data; pick --seed-prefix`,
            );
          }
          console.log(`library before seed: ${seed.before.products} products, ${seed.before.presets} presets`);
          if (args.largePlan > 0) {
            const fix = planFixture(args.seedPrefix, stamp, args.largePlan, Math.max(1, args.largePresets));
            for (let i = 0; i < fix.products.length; i++) {
              await invokeInPage(cdp, "create_product", { product: fix.products[i] });
              if ((i + 1) % 200 === 0) console.log(`seed products ${i + 1}/${fix.products.length}`);
            }
            for (const preset of fix.presets) {
              await invokeInPage(cdp, "create_preset", { preset });
            }
            seed.presetIds = fix.presets.map((p) => p.id);
            seed.productIds = fix.products.map((p) => p.id);
            console.log(`seeded plan: ${fix.products.length} products in ${fix.presets.length} presets`);
          }
          if (args.largeBackup > 0) {
            const doc = backupFixture(args.seedPrefix, stamp, args.largeBackup);
            seed.backupPath = path.join(fs.realpathSync(os.tmpdir()), `sprout-repro-backup-${stamp}.json`);
            fs.writeFileSync(seed.backupPath, JSON.stringify(doc));
            const bytes = fs.statSync(seed.backupPath).size;
            const records = doc.products.length + doc.presets.length + doc.launch_entries.length + doc.quick_actions.length + doc.clips.length;
            console.log(`backup fixture: ${records} records, ${(bytes / 1024).toFixed(0)} KiB at ${seed.backupPath}`);
          }
          if (seed.presetIds.length > 0) {
            const ids = seed.presetIds;
            report.heavy.push(await timeHeavy(cdp, `compute_plan ${args.largePlan}reqs`, args.sample, args.stall, () =>
              cdp.eval(`(async () => {
                const t0 = performance.now();
                const c = await window.__TAURI_INTERNALS__.invoke("compute_plan", ${JSON.stringify({ presetIds: ids })});
                return { ms: Math.round(performance.now() - t0), entries: c.entries.length };
              })()`),
            ));
          }
          if (seed.backupPath) {
            const backupPath = seed.backupPath;
            report.heavy.push(await timeHeavy(cdp, `inspect_backup`, args.sample, args.stall, () =>
              cdp.eval(`(async () => {
                const t0 = performance.now();
                const counts = await window.__TAURI_INTERNALS__.invoke("inspect_backup", ${JSON.stringify({ path: backupPath })});
                return { ms: Math.round(performance.now() - t0), counts };
              })()`),
            ));
          }
        }
        const sweep = args.target ? [args.target] : args.targets ? args.targets : SWEEP;
        for (let rep = 0; rep < args.reps; rep++) {
          for (const href of sweep) {
            await clickTab(cdp, href, args.budget, args.stall, args.sample, args.delay, report);
          }
        }
      } finally {
        // WHY best-effort reverse-order removal with a targeted leftover check:
        // a leaked fixture row would warm every future run, so leftovers are
        // an infra failure, never a silent pass.
        const leftovers = [];
        for (const id of seed.presetIds) {
          try {
            await invokeInPage(cdp, "delete_preset", { id });
          } catch {
            leftovers.push(id);
          }
        }
        for (const id of seed.productIds) {
          try {
            await invokeInPage(cdp, "delete_product", { id });
          } catch {
            leftovers.push(id);
          }
        }
        if (seed.backupPath) {
          try {
            fs.unlinkSync(seed.backupPath);
          } catch {}
        }
        if (seed.before) {
          const after = await librarySnapshot(cdp, args.seedPrefix);
          const prefixRows = after.clashes.length;
          console.log(`library after cleanup: ${after.products} products, ${after.presets} presets (was ${seed.before.products}/${seed.before.presets})`);
          if (leftovers.length > 0 || prefixRows > 0) {
            throw new Error(`seed cleanup incomplete: ${leftovers.length} delete failures, ${prefixRows} prefixed rows remain`);
          }
        }
      }
    }
  } catch (e) {
    infraError = e;
  } finally {
    const { red, bad, heavyBad } = verdict(report, args.stall, args.budget);
    for (const h of report.heavy) {
      const stallNote = h.stalls.length
        ? ` STALLS: ${h.stalls.map((s) => `${s.max}ms`).join(", ")}`
        : "";
      const detail = h.ok ? JSON.stringify(h.summary) : `ERROR ${h.error}`;
      console.log(`heavy ${h.label.padEnd(24)} ${h.ok ? "ok" : "FAILED"} ${h.ms}ms ${detail}${stallNote}`);
    }
    for (const c of report.clicks) {
      const stallNote = c.stalls.length
        ? ` STALLS: ${c.stalls.map((s) => `${s.max}ms@+${s.from - c.start}ms`).join(", ")}`
        : "";
      console.log(
        `click ${c.href.padEnd(12)} ${c.ok ? "landed" : "NEVER LANDED"} ${c.landMs}ms${stallNote}`,
      );
    }

    if (infraError) {
      console.error(`INFRA ERROR: ${infraError.message}`);
      if (child && child.tail) console.error(child.tail.slice(-10).join(""));
      process.exitCode = 2;
    } else {
      const log = extractLog(cdp);
      if (log.length) {
        console.log("--- captured console/errors ---");
        for (const l of log) {
          console.log(`[${l.kind}/${l.type ?? l.level ?? ""}] ${l.text}${l.url ? ` (${l.url}:${l.line ?? ""})` : ""}`);
        }
      }
      console.log(red ? "VERDICT: RED" : "VERDICT: GREEN");
      process.exitCode = red ? 1 : 0;
    }

    cdp?.close();
    if (!args.keep && !args.attach && child) {
      killTree(child);
      await sleep(500);
    }
    await sleep(100);
    process.exit(process.exitCode ?? 0);
  }
}

main().catch((e) => {
  console.error(e);
  process.exit(2);
});