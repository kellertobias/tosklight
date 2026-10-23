import { execFile, spawn } from "node:child_process";
import { mkdir, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { promisify } from "node:util";
import { artifactPaths, repositoryRoot } from "./artifact-paths.mjs";

const exec = promisify(execFile);

/** Signals only the subprocess created here, never the PID supplied to --attach. */
export function runOwnedCommand(file, args, {
  timeout = 15000, termGraceMs = 1000, killGraceMs = 1000,
  maxBuffer = 4 * 1024 * 1024, spawnChild = spawn,
} = {}) {
  return new Promise((resolve, reject) => {
    const child = spawnChild(file, args, { stdio: ["ignore", "pipe", "pipe"] });
    const output = { stdout: [], stderr: [] };
    const sizes = { stdout: 0, stderr: 0 };
    const timers = [];
    let finished = false;
    let failure;
    const later = (callback, delay) => timers.push(setTimeout(callback, delay));
    const finish = (code, signal) => {
      if (finished) return;
      finished = true;
      timers.forEach(clearTimeout);
      child.stdout?.destroy();
      child.stderr?.destroy();
      child.unref();
      const data = Object.fromEntries(Object.entries(output).map(([name, chunks]) => [name, Buffer.concat(chunks).toString("utf8")]));
      if (failure || code !== 0) {
        reject(Object.assign(failure ?? new Error(`${file} exited with ${code ?? signal}`), data, { signal: signal ?? null, exitCode: code }));
      } else resolve(data);
    };
    const stop = (error) => {
      if (failure || finished) return;
      failure = error;
      child.kill("SIGTERM");
      later(() => {
        if (finished) return;
        child.kill("SIGKILL");
        // Even missing close/exit events or inherited pipes cannot hold up result.json.
        later(() => finish(null, "SIGKILL"), killGraceMs);
      }, termGraceMs);
    };
    for (const name of ["stdout", "stderr"]) {
      child[name]?.on("data", (chunk) => {
        const bytes = Buffer.from(chunk);
        const available = Math.max(0, maxBuffer - sizes[name]);
        if (available > 0) output[name].push(bytes.subarray(0, available));
        sizes[name] += Math.min(bytes.length, available);
        if (bytes.length > available) stop(Object.assign(new Error(`${name} exceeded maxBuffer`), { code: "ERR_CHILD_PROCESS_STDIO_MAXBUFFER" }));
      });
    }
    child.on("error", (error) => { failure ??= error; finish(null, null); });
    child.on("close", finish);
    child.on("exit", (code, signal) => {
      // Do not wait forever for pipes inherited by descendants after the child exits.
      later(() => finish(code, signal), killGraceMs);
    });
    later(() => stop(Object.assign(new Error(`${file} timed out after ${timeout} ms`), { code: "ETIMEDOUT", killed: true })), timeout);
  });
}
const usage = `Usage: node tools/stage-gpu-trace.mjs --pid PID --executable /absolute/native/path
  --owned-isolated --build BUILD_ID --window WINDOW_DESCRIPTION [--duration 10]

Attach only to an already running, owned, isolated ToskLight native process.
No application is launched, restarted or stopped. DEVELOPER_DIR selects Xcode.
Raw trace, exported table of contents and result.json go under the canonical
performance/stage directory. The result deliberately contains no displayed FPS:
inspect the exported schemas and validate presentation semantics before analysis.`;

export function parseOptions(args) {
  const options = { duration: 10, ownedIsolated: false };
  const names = new Set(["pid", "executable", "build", "window", "duration"]);
  for (let i = 0; i < args.length; i++) {
    const name = args[i].replace(/^--/, "");
    if (args[i] === "--owned-isolated") options.ownedIsolated = true;
    else if (args[i].startsWith("--") && names.has(name) && args[i + 1] && !args[i + 1].startsWith("--")) options[name] = args[++i];
    else throw new Error(`Unknown or incomplete argument: ${args[i]}`);
  }
  options.pid = Number(options.pid);
  options.duration = Number(options.duration);
  if (!Number.isSafeInteger(options.pid) || options.pid <= 1) throw new Error("Explicit native PID greater than 1 is required");
  if (!options.executable || !path.isAbsolute(options.executable)) throw new Error("Absolute expected executable is required");
  if (!["ToskLight", "light-desktop", "viz-renderer"].includes(path.basename(options.executable))) throw new Error("Expected a ToskLight native desk or renderer executable");
  if (!options.ownedIsolated) throw new Error("Confirm ownership and isolation with --owned-isolated");
  if (!options.build?.trim() || !options.window?.trim()) throw new Error("Build and window identity are required");
  if (!Number.isInteger(options.duration) || options.duration < 1 || options.duration > 300) throw new Error("Duration must be 1–300 seconds");
  return options;
}

export async function inspectProcess(options, run, uid) {
  const query = async (field) => (await run("/bin/ps", ["-p", String(options.pid), "-o", `${field}=`])).stdout.trim();
  const owner = await query("uid");
  const executable = await query("comm");
  const started = await query("lstart");
  if (owner !== String(uid) || executable !== options.executable || !started) throw new Error("PID identity mismatch or process is not owned by current user");
  return { pid: options.pid, uid: Number(owner), executable, started };
}

export async function collectTrace(options, dependencies = {}) {
  const run = dependencies.run ?? ((file, args, extra = {}) =>
    ["record", "export"].includes(args[0])
      ? runOwnedCommand(file, args, extra)
      : exec(file, args, { timeout: 15000, maxBuffer: 4 * 1024 * 1024, ...extra }));
  const directory = await (dependencies.makeDirectory ?? (async () => {
    const { mkdtemp } = await import("node:fs/promises");
    const root = path.join(artifactPaths.performance, "stage");
    await mkdir(root, { recursive: true });
    return mkdtemp(path.join(root, "gpu-trace-"));
  }))();
  const save = dependencies.save ?? ((name, data) => writeFile(path.join(directory, name), data));
  const result = {
    schemaVersion: 1, status: "error", phase: "identity", startedAt: new Date().toISOString(),
    directory, requested: options, repository: repositoryRoot,
    artifacts: { rawTrace: "metal.trace", tableOfContents: "toc.xml", result: "result.json" },
    host: { platform: dependencies.platform ?? process.platform, release: os.release(), architecture: os.arch() },
    developerDir: process.env.DEVELOPER_DIR ?? null,
    displayedFrameIntervals: { status: "unavailable", reason: "Raw trace schemas require inspection; command submission and GPU completion are not displayed presentation." },
  };
  try {
    if (result.host.platform !== "darwin") {
      result.status = "unavailable";
      result.reason = "Xcode Metal System Trace requires macOS";
      return result;
    }
    const uid = dependencies.uid ?? process.getuid();
    result.process = await inspectProcess(options, run, uid);
    result.phase = "tool-discovery";
    const tool = (await run("/usr/bin/xcrun", ["--find", "xctrace"])).stdout.trim();
    if (!path.isAbsolute(tool)) throw new Error("xcrun did not return an absolute xctrace path");
    result.tool = { path: tool, version: (await run(tool, ["version"])).stdout.trim() };
    result.phase = "host-metadata";
    result.osVersion = (await run("/usr/bin/sw_vers", [])).stdout.trim();
    try { await save("gpu.json", (await run("/usr/sbin/system_profiler", ["SPDisplaysDataType", "-json"])).stdout); result.gpuMetadata = "gpu.json"; }
    catch (error) { result.gpuMetadata = { status: "unavailable", reason: error.message }; }
    result.phase = "record";
    // Revalidate immediately before attaching; lstart also guards against PID reuse.
    const before = await inspectProcess(options, run, uid);
    if (before.started !== result.process.started) throw new Error("PID was reused before trace attachment");
    const trace = path.join(directory, "metal.trace");
    const recording = await run(tool, ["record", "--no-prompt", "--template", "Metal System Trace", "--attach", String(options.pid), "--time-limit", `${options.duration}s`, "--output", trace], { timeout: (options.duration + 120) * 1000 });
    await save("record.log", `${recording.stdout ?? ""}\n${recording.stderr ?? ""}`);
    result.rawTrace = "metal.trace";
    result.phase = "export-toc";
    await run(tool, ["export", "--input", trace, "--toc", "--output", path.join(directory, "toc.xml")], { timeout: 30000 });
    result.tableOfContents = "toc.xml";
    result.phase = "identity-after";
    result.processAfter = await inspectProcess(options, run, uid);
    if (result.processAfter.started !== result.process.started) throw new Error("PID identity changed during trace");
    result.status = "ok";
    result.phase = "complete";
  } catch (error) {
    result.status = error.killed || error.code === "ETIMEDOUT" ? "timeout" : result.phase === "tool-discovery" ? "unavailable" : "error";
    result.reason = error.message;
    result.commandError = { code: error.code ?? null, signal: error.signal ?? null, stdout: error.stdout ?? "", stderr: error.stderr ?? "" };
  } finally {
    result.finishedAt = new Date().toISOString();
    await save("result.json", `${JSON.stringify(result, null, 2)}\n`);
  }
  return result;
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  if (process.argv.includes("--help")) console.log(usage);
  else {
    try {
      const result = await collectTrace(parseOptions(process.argv.slice(2)));
      console.log(JSON.stringify(result, null, 2));
      if (result.status !== "ok") process.exitCode = 1;
    } catch (error) { console.error(`${error.message}\n\n${usage}`); process.exitCode = 1; }
  }
}
