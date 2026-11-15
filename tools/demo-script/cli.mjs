#!/usr/bin/env node
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { access, mkdir, readFile, unlink, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { createInterface } from "node:readline/promises";
import { fileURLToPath } from "node:url";
import { artifactPaths } from "../artifact-paths.mjs";
import { executeScript, parseScript } from "./runtime.mjs";

const directory = path.dirname(fileURLToPath(import.meta.url));
const cache = path.join(artifactPaths.tmp, "demo-script");
const usage = `Usage:
  npm run demo:script -- SCRIPT.yaml [--countdown 10] [--step] [--dry-run]
  npm run demo:script -- SCRIPT.yaml --inspect APP_ALIAS
  npm run demo:script -- stop

Start recording during the countdown. Pause steps resume with Enter in this terminal.
Use a second terminal's stop command to cancel after the current interaction.
Native UI replay requires macOS Accessibility access for the launching terminal/app.`;

function command(program, args, input, timeout = 30_000) {
  return new Promise((resolve, reject) => {
    // Let the helper release held input before the parent handles terminal Ctrl+C.
    const child = spawn(program, args, { stdio: ["pipe", "pipe", "pipe"], detached: process.platform === "darwin" });
    let stdout = "", stderr = "", settled = false;
    const finish = (error, result) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      error ? reject(error) : resolve(result);
    };
    const timer = setTimeout(() => {
      child.kill("SIGTERM");
      finish(new Error(`${path.basename(program)} timed out after ${timeout / 1000}s`));
    }, timeout);
    child.on("error", (error) => finish(error));
    child.stdout.on("data", (bytes) => {
      stdout += bytes;
      if (stdout.length > 4 * 1024 * 1024) { child.kill(); finish(new Error("Native helper output exceeded 4 MiB")); }
    });
    child.stderr.on("data", (bytes) => { stderr = (stderr + bytes).slice(-32_000); });
    child.on("close", (code) => finish(code === 0 ? null : new Error(stderr.trim() || `${program} exited ${code}`), stdout));
    child.stdin.on("error", () => {});
    child.stdin.end(input ?? "");
  });
}

async function helper() {
  if (process.platform !== "darwin") throw new Error("Native demo replay currently supports macOS only. --dry-run works on every platform.");
  const source = path.join(directory, "NativeUI.swift");
  const hash = createHash("sha256").update(await readFile(source)).digest("hex").slice(0, 16);
  const binary = path.join(cache, `native-ui-${hash}`);
  await mkdir(cache, { recursive: true });
  try { await access(binary); }
  catch {
    console.error("Building the macOS UI helper...");
    await command("/usr/bin/swiftc", ["-parse-as-library", "-module-cache-path", path.join(cache, "module-cache"), source, "-o", binary], undefined, 120_000);
  }
  return async (request) => JSON.parse(await command(binary, [], JSON.stringify(request)));
}

const delay = (milliseconds, signal) => new Promise((resolve, reject) => {
  if (signal?.aborted) { reject(signal.reason); return; }
  const aborted = () => { clearTimeout(timer); reject(signal.reason); };
  const timer = setTimeout(() => { signal?.removeEventListener("abort", aborted); resolve(); }, milliseconds);
  signal?.addEventListener("abort", aborted, { once: true });
});

async function main() {
  const args = process.argv.slice(2);
  if (!args.length || args.includes("--help")) { console.log(usage); return; }
  const activeFile = path.join(cache, "active.json");
  if (args[0] === "stop") {
    const active = JSON.parse(await readFile(activeFile, "utf8").catch(() => { throw new Error("No demo script run is registered."); }));
    const expectedBase = path.join(artifactPaths.visual, "demo-scripts") + path.sep;
    if (!path.resolve(active.stopFile).startsWith(expectedBase) || !(active.pid > 0)) throw new Error("Invalid active-run record.");
    try { process.kill(active.pid, 0); } catch { throw new Error("The registered demo script is no longer running."); }
    await writeFile(active.stopFile, "stop\n");
    console.log("Stop requested. The current interaction will finish before replay stops.");
    return;
  }
  const scriptPath = path.resolve(args.shift());
  let countdown = 10, stepMode = false, dryRun = false, inspect;
  while (args.length) {
    const flag = args.shift();
    if (flag === "--countdown") countdown = Number(args.shift());
    else if (flag === "--inspect") {
      inspect = args.shift();
      if (!inspect || inspect.startsWith("--")) throw new Error("--inspect requires an app alias.");
    }
    else if (flag === "--step") stepMode = true;
    else if (flag === "--dry-run") dryRun = true;
    else throw new Error(`Unknown option: ${flag}\n${usage}`);
  }
  if (!Number.isFinite(countdown) || countdown < 0 || countdown > 60) throw new Error("Countdown must be between 0 and 60 seconds.");
  const source = await readFile(scriptPath, "utf8");
  const script = parseScript(source, { filename: scriptPath });
  if (dryRun) { console.log(JSON.stringify(script, null, 2)); return; }
  const native = await helper();
  if (!(await native({ operation: "check" })).trusted) throw new Error("Enable the launching terminal/app in System Settings > Privacy & Security > Accessibility, then rerun. No actions were sent.");
  if (inspect !== undefined) {
    if (!script.apps[inspect]) throw new Error(`Unknown app alias: ${inspect}`);
    console.log(JSON.stringify(await native({ operation: "inspect", app: script.apps[inspect] }), null, 2));
    return;
  }
  if ((stepMode || script.steps.some((s) => "pause" in s)) && !process.stdin.isTTY) throw new Error("Pause and --step need an interactive terminal.");
  const runDirectory = path.join(artifactPaths.visual, "demo-scripts", `${Date.now()}-${process.pid}`);
  await mkdir(runDirectory, { recursive: true });
  await mkdir(cache, { recursive: true });
  const stopFile = path.join(runDirectory, "STOP");
  // A live registered run owns focus; never start a competing replay.
  try {
    const active = JSON.parse(await readFile(activeFile, "utf8"));
    if (!(active.pid > 0)) throw new Error("Invalid active-run record; inspect the demo-script cache.");
    try { process.kill(active.pid, 0); throw new Error(`Another demo script is running (PID ${active.pid}). Stop it first.`); }
    catch (error) { if (error.code !== "ESRCH") throw error; }
    await unlink(activeFile);
  } catch (error) { if (error.code !== "ENOENT") throw error; }
  await writeFile(activeFile, JSON.stringify({ pid: process.pid, stopFile }), { flag: "wx" });
  const controller = new AbortController();
  const abort = () => controller.abort(new Error("Replay stopped by operator"));
  process.once("SIGINT", abort);
  process.once("SIGTERM", abort);
  const stopCheck = setInterval(async () => {
    try { await access(stopFile); abort(); } catch {}
  }, 100);
  const terminal = process.stdin.isTTY ? createInterface({ input: process.stdin, output: process.stdout }) : null;
  const pause = async (message) => {
    await terminal.question(`${message}\nPress Enter in this terminal to continue. `, { signal: controller.signal });
  };
  const pids = new Map();
  const invoke = (operation, app, extra = {}) => native({ operation, app: { ...app, ...(pids.has(app.alias) ? { pid: pids.get(app.alias) } : {}) }, ...extra });
  const ready = async (app, selector) => {
    const until = performance.now() + 10_000;
    while (true) {
      controller.signal.throwIfAborted();
      try { return await invoke("waitFor", app, { selector }); }
      catch (error) {
        if (!/found 0|Control is disabled/.test(error.message) || performance.now() >= until) throw error;
        await delay(200, controller.signal);
      }
    }
  };
  const adapter = {
    async focus(app) {
      const result = await native({ operation: "focus", app });
      pids.set(app.alias, result.pid);
      return result;
    },
    async click(app, selector) {
      selector = { role: "button", ...selector };
      await ready(app, selector);
      return invoke("click", app, { selector });
    },
    async fill(app, spec) {
      const selector = { ...spec, role: "field" };
      delete selector.text;
      await ready(app, selector);
      return invoke("fill", app, { selector, text: spec.text });
    },
    key: (app, chord) => invoke("key", app, { key: chord }),
    waitFor: ready,
    clickAt: (app, point) => invoke("clickAt", app, { point }),
    drag: (app, spec) => invoke("drag", app, spec),
  };
  let report;
  try {
    console.log(`${script.name}\nRun log: ${runDirectory}\nStart your recorder. Replay begins in ${countdown}s.`);
    await delay(countdown * 1000, controller.signal);
    report = await executeScript(script, adapter, {
      signal: controller.signal,
      sleep: (milliseconds) => delay(milliseconds, controller.signal),
      pause,
      beforeStep: stepMode ? (_step, index) => pause(`Ready for step ${index + 1}.`) : undefined,
      log: (entry) => console.log(JSON.stringify(entry)),
    });
    console.log("Replay completed.");
  } catch (error) {
    report = error.report ?? { status: controller.signal.aborted ? "aborted" : "failed", error: error.message };
    throw error;
  } finally {
    clearInterval(stopCheck);
    terminal?.close();
    process.removeListener("SIGINT", abort);
    process.removeListener("SIGTERM", abort);
    await writeFile(path.join(runDirectory, "timeline.json"), JSON.stringify(report ?? { status: "stopped" }, null, 2));
    await writeFile(path.join(runDirectory, "script.yaml"), source);
    await unlink(activeFile);
  }
}

main().catch((error) => { console.error(error.message); process.exitCode = 1; });
