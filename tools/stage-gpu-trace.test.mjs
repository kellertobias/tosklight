import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import { PassThrough } from "node:stream";
import test from "node:test";
import { collectTrace, parseOptions, runOwnedCommand } from "./stage-gpu-trace.mjs";

const args = ["--pid", "4321", "--executable", "/isolated/ToskLight", "--owned-isolated", "--build", "abc123", "--window", "Live 1920x1080 @60Hz"];
const options = parseOptions(args);

function harness(overrides = {}) {
  const files = new Map();
  const calls = [];
  let starts = 0;
  const run = async (file, argv, extra) => {
    calls.push({ file, argv, extra });
    if (overrides.run) {
      const answer = overrides.run(file, argv, extra);
      if (answer !== undefined) return answer;
    }
    let stdout = "";
    if (file === "/bin/ps") {
      const field = argv.at(-1);
      stdout = field === "uid=" ? "501" : field === "comm=" ? options.executable : `Fri Oct 23 02:00:0${overrides.reuse && ++starts > 1 ? 1 : 0} 2026`;
    } else if (file === "/usr/bin/xcrun") stdout = "/Xcode/xctrace";
    else if (argv[0] === "version") stdout = "xctrace version fixture";
    return { stdout, stderr: "" };
  };
  return {
    files, calls,
    dependencies: { run, platform: "darwin", uid: 501, makeDirectory: async () => "/canonical/performance/stage/test", save: async (name, data) => files.set(name, data) },
  };
}

test("requires explicit ownership, PID, build and window identity before collection", () => {
  for (const bad of [[], args.filter((arg) => arg !== "--owned-isolated"), [...args, "--duration", "0"], [...args, "--pid", "1"], [...args, "--executable", "relative"], [...args, "--executable", "/Applications/OtherApp"], [...args, "--window", " "]]) assert.throws(() => parseOptions(bad));
});

test("records only explicit PID and exports raw TOC, with no derived displayed FPS", async () => {
  const h = harness();
  const result = await collectTrace(options, h.dependencies);
  assert.equal(result.status, "ok");
  assert.equal(result.displayedFrameIntervals.status, "unavailable");
  assert.ok(h.files.has("gpu.json"));
  assert.ok(h.files.has("result.json"));
  const record = h.calls.find((call) => call.argv[0] === "record");
  assert.deepEqual(record.argv.slice(0, 8), ["record", "--no-prompt", "--template", "Metal System Trace", "--attach", "4321", "--time-limit", "10s"]);
  assert.equal(record.extra.timeout, 130000);
  assert.ok(h.calls.some((call) => call.argv.includes("--toc")));
  assert.ok(h.calls.every((call) => !call.argv.includes("--launch") && !call.argv.includes("--all-processes")));
});

test("rejects an unrelated executable before invoking xctrace", async () => {
  const h = harness({ run: (file, argv) => file === "/bin/ps" && argv.at(-1) === "comm=" ? { stdout: "/Applications/OtherApp" } : undefined });
  const result = await collectTrace(options, h.dependencies);
  assert.equal(result.status, "error");
  assert.equal(h.calls.some((call) => call.file === "/usr/bin/xcrun"), false);
});

test("rejects PID reuse before attachment", async () => {
  const h = harness({ reuse: true });
  const result = await collectTrace(options, h.dependencies);
  assert.equal(result.status, "error");
  assert.match(result.reason, /PID was reused/);
  assert.equal(h.calls.some((call) => call.argv[0] === "record"), false);
});

test("rejects a process owned by another user before invoking xctrace", async () => {
  const h = harness();
  const result = await collectTrace(options, { ...h.dependencies, uid: 502 });
  assert.equal(result.status, "error");
  assert.equal(h.calls.some((call) => call.file === "/usr/bin/xcrun"), false);
});

test("retains timeout and export failure evidence without reporting success", async () => {
  for (const [phase, expected] of [["record", "timeout"], ["export", "error"]]) {
    const h = harness({ run: (_file, argv) => {
      if (argv[0] === phase) throw Object.assign(new Error(`${phase} failed`), { killed: phase === "record", stderr: "fixture diagnostic" });
    } });
    const result = await collectTrace(options, h.dependencies);
    assert.equal(result.status, expected);
    assert.equal(result.commandError.stderr, "fixture diagnostic");
    assert.equal(JSON.parse(h.files.get("result.json")).status, expected);
    assert.equal(result.displayedFrameIntervals.status, "unavailable");
  }
});

test("reports unsupported platforms and missing tools explicitly", async () => {
  const h = harness();
  const unsupported = await collectTrace(options, { ...h.dependencies, platform: "linux" });
  assert.equal(unsupported.status, "unavailable");
  assert.equal(h.calls.length, 0);
  const absent = harness({ run: (file) => { if (file === "/usr/bin/xcrun") throw new Error("Xcode missing"); } });
  assert.equal((await collectTrace(options, absent.dependencies)).status, "unavailable");
});

test("kills its owned TERM-ignoring child and settles within the bounded grace period", { timeout: 5000 }, async () => {
  const began = Date.now();
  const script = `process.on('SIGTERM', () => process.stdout.write('ignored-term\\n')); process.stdout.write('pid=' + process.pid + '\\n'); setInterval(() => {}, 100);`;
  await assert.rejects(runOwnedCommand(process.execPath, ["-e", script], { timeout: 500, termGraceMs: 100, killGraceMs: 100 }), (error) => {
    assert.equal(error.code, "ETIMEDOUT");
    assert.match(error.stdout, /ignored-term/);
    assert.equal(error.signal, "SIGKILL");
    const pid = Number(error.stdout.match(/pid=(\d+)/)?.[1]);
    assert.ok(pid > 1);
    assert.throws(() => process.kill(pid, 0), { code: "ESRCH" });
    assert.ok(Date.now() - began < 3000);
    return true;
  });
});

test("missing child exit events cannot prevent timeout evidence from being saved", { timeout: 3000 }, async () => {
  const signals = [];
  const h = harness({ run: (_file, argv) => {
    if (argv[0] !== "export") return undefined;
    return runOwnedCommand("/fixture/xctrace", argv, {
      timeout: 10, termGraceMs: 10, killGraceMs: 10,
      spawnChild: () => Object.assign(new EventEmitter(), {
        stdout: new PassThrough(), stderr: new PassThrough(),
        kill: (signal) => { signals.push(signal); return true; }, unref: () => {},
      }),
    });
  } });
  const result = await collectTrace(options, h.dependencies);
  assert.equal(result.status, "timeout");
  assert.equal(result.phase, "export-toc");
  assert.deepEqual(signals, ["SIGTERM", "SIGKILL"]);
  assert.equal(JSON.parse(h.files.get("result.json")).status, "timeout");
});

test("owned command retains successful stdout and failing stderr", async () => {
  assert.equal((await runOwnedCommand(process.execPath, ["-e", "process.stdout.write('ok')"])).stdout, "ok");
  await assert.rejects(runOwnedCommand(process.execPath, ["-e", "process.stderr.write('bad'); process.exit(2)"]), (error) => error.exitCode === 2 && error.stderr === "bad");
});
