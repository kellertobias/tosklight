import { execFile } from "node:child_process";
import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { promisify } from "node:util";

if (process.platform !== "darwin") throw new Error("desktop-smoke currently targets the macOS Tauri bundle");

const exec = promisify(execFile);
const root = path.resolve(import.meta.dirname, "../../..");
const app = path.join(root, "target/debug/bundle/macos/ToskLight.app/Contents/MacOS/light-control-ui");
const server = path.join(root, "target/debug/light-server");
const canonical = await fs.readFile(path.join(root, "tests/fixtures/default-stage.show"));

const requestedScenario = process.argv[2] ?? "all";
if (!new Set(["all", "DESKTOP-001", "DESKTOP-002"]).has(requestedScenario)) {
  throw new Error(`Unknown desktop smoke scenario: ${requestedScenario}`);
}
if (requestedScenario === "all" || requestedScenario === "DESKTOP-001") await ownedServerScenario();
if (requestedScenario === "all" || requestedScenario === "DESKTOP-002") await independentServerScenario();
console.log(
  requestedScenario === "all"
    ? "Desktop smoke passed: DESKTOP-001 app-owned and DESKTOP-002 independent-server process contracts are intact."
    : `Desktop smoke passed: ${requestedScenario}.`,
);

async function ownedServerScenario() {
  const dataDir = await fs.mkdtemp(path.join(os.tmpdir(), "light-desktop-owned-"));
  const marker = path.join(dataDir, "frontend-ready.json");
  const port = await freePort();
  let desktop;
  let seeder;
  try {
    seeder = await startSeededServer(dataDir, port, "desktop-001");
    await authenticated(seeder.token, port, "POST", "/api/v1/shutdown");
    await waitForExit(seeder.child, 5_000, "DESKTOP-001 seed server exit");
    await waitForPortClosed(port, 5_000, "DESKTOP-001 pre-launch port closure");

    desktop = spawnDesktop(dataDir, port, marker);
    const appPid = requirePid(desktop.child, "DESKTOP-001 app");
    const childPid = await waitForValue(async () => {
      const children = await lightServerChildren(appPid);
      return children.length === 1 ? children[0] : undefined;
    }, 15_000, "DESKTOP-001 exact child-server PID");
    await waitFor(async () => (await fetch(`http://127.0.0.1:${port}/api/v1/readiness`)).ok, 8_000, "DESKTOP-001 app-owned server readiness");
    const bootstrap = await fetchJson(port, "/api/v1/bootstrap");
    if (bootstrap.active_show?.name !== "desktop-001") throw new Error(`DESKTOP-001 active show mismatch: ${JSON.stringify(bootstrap.active_show)}`);
    if (!bootstrap.attribute_registry?.some((attribute) => attribute.id === "intensity")) {
      throw new Error("DESKTOP-001 packaged desk did not expose the canonical fixture attribute registry");
    }
    const fixtureProfiles = await authenticated(seeder.token, port, "GET", "/api/v1/fixture-profiles");
    if (!fixtureProfiles.length || fixtureProfiles.some((profile) => profile.schema_version !== 2 || profile.revision < 1)) {
      throw new Error("DESKTOP-001 packaged desk did not start with revisioned schema-v2 fixture profiles");
    }
    if (!fixtureProfiles.some((profile) => profile.reserved_source === "builtin:generic-catalog")) {
      throw new Error("DESKTOP-001 packaged desk did not seed reserved built-in Generic profiles");
    }
    const ready = await waitForValue(async () => JSON.parse(await fs.readFile(marker, "utf8")), 15_000, "DESKTOP-001 frontend-ready marker");
    if (ready.ready !== true || ready.server !== `127.0.0.1:${port}`) throw new Error(`DESKTOP-001 invalid frontend marker: ${JSON.stringify(ready)}`);

    await waitForExit(desktop.child, 8_000, "DESKTOP-001 app exit");
    await waitFor(async () => !pidAlive(childPid), 5_000, `DESKTOP-001 child PID ${childPid} exit`);
    await waitForPortClosed(port, 5_000, "DESKTOP-001 app-owned listener closure");
  } catch (error) {
    throw withLogs(error, desktop?.log, seeder?.log);
  } finally {
    kill(desktop?.child);
    kill(seeder?.child);
    await fs.rm(dataDir, { recursive: true, force: true });
  }
}

async function independentServerScenario() {
  const dataDir = await fs.mkdtemp(path.join(os.tmpdir(), "light-desktop-independent-"));
  const marker = path.join(dataDir, "frontend-ready.json");
  const port = await freePort();
  let desktop;
  let independent;
  try {
    independent = await startSeededServer(dataDir, port, "desktop-002");
    const independentPid = requirePid(independent.child, "DESKTOP-002 independent server");
    desktop = spawnDesktop(dataDir, port, marker);
    const appPid = requirePid(desktop.child, "DESKTOP-002 app");
    const ready = await waitForValue(async () => JSON.parse(await fs.readFile(marker, "utf8")), 15_000, "DESKTOP-002 frontend-ready marker");
    if (ready.ready !== true) throw new Error(`DESKTOP-002 invalid frontend marker: ${JSON.stringify(ready)}`);
    const appServerChildren = await lightServerChildren(appPid);
    if (appServerChildren.length !== 0) throw new Error(`DESKTOP-002 created an unexpected child server: ${appServerChildren.join(", ")}`);
    const bootstrap = await fetchJson(port, "/api/v1/bootstrap");
    if (bootstrap.active_show?.name !== "desktop-002") throw new Error(`DESKTOP-002 active show mismatch: ${JSON.stringify(bootstrap.active_show)}`);

    await waitForExit(desktop.child, 8_000, "DESKTOP-002 app exit");
    if (!pidAlive(independentPid)) throw new Error(`DESKTOP-002 killed independent server PID ${independentPid}`);
    if (!(await fetch(`http://127.0.0.1:${port}/api/v1/readiness`)).ok) throw new Error("DESKTOP-002 independent readiness failed after app exit");
    const created = await authenticated(independent.token, port, "POST", "/api/v1/shows", {
      name: `desktop-002-post-exit-${crypto.randomUUID()}`,
      data_base64: null,
      overwrite: false,
    });
    if (!created.id) throw new Error(`DESKTOP-002 authenticated post-exit write failed: ${JSON.stringify(created)}`);
    if (!created.path?.startsWith(dataDir)) throw new Error(`DESKTOP-002 write escaped its data directory: ${JSON.stringify(created)}`);
    await fs.access(created.path);
    await fs.access(path.join(dataDir, "desk.sqlite"));

    await authenticated(independent.token, port, "POST", "/api/v1/shutdown");
    await waitForExit(independent.child, 5_000, "DESKTOP-002 independent cleanup exit");
  } catch (error) {
    throw withLogs(error, desktop?.log, independent?.log);
  } finally {
    kill(desktop?.child);
    kill(independent?.child);
    await fs.rm(dataDir, { recursive: true, force: true });
  }
}

async function startSeededServer(dataDir, port, showName) {
  const running = spawnLogged(server, [
    "--data-dir", dataDir,
    "--bind", `127.0.0.1:${port}`,
    "--osc-bind", "127.0.0.1:0",
    "--output-bind-ip", "127.0.0.1",
  ]);
  await waitFor(async () => (await fetch(`http://127.0.0.1:${port}/api/v1/readiness`)).ok, 15_000, `${showName} seed readiness`);
  const session = await request(port, "POST", "/api/v1/sessions", { username: "Operator" });
  const show = await authenticated(session.token, port, "POST", "/api/v1/shows", {
    name: showName,
    data_base64: canonical.toString("base64"),
    overwrite: false,
  });
  await authenticated(session.token, port, "POST", `/api/v1/shows/${show.id}/open`, { transition: "hold_current" });
  return { ...running, token: session.token, showId: show.id };
}

function spawnDesktop(dataDir, port, marker) {
  return spawnLogged(app, [], {
    ...process.env,
    LIGHT_DESKTOP_TEST_DATA_DIR: dataDir,
    LIGHT_DESKTOP_TEST_BIND: `127.0.0.1:${port}`,
    LIGHT_DESKTOP_TEST_READY_FILE: marker,
    LIGHT_DESKTOP_TEST_AUTO_EXIT: "1500",
  });
}

function spawnLogged(command, args, env = process.env) {
  const log = [];
  const child = spawn(command, args, { env, cwd: root, stdio: ["ignore", "pipe", "pipe"] });
  const collect = (chunk) => {
    log.push(chunk.toString());
    if (log.length > 500) log.shift();
  };
  child.stdout.on("data", collect);
  child.stderr.on("data", collect);
  return { child, log };
}

async function lightServerChildren(parentPid) {
  const { stdout } = await exec("ps", ["-axo", "pid=,ppid=,command="]);
  return stdout.split("\n").flatMap((line) => {
    const match = line.match(/^\s*(\d+)\s+(\d+)\s+(.+)$/);
    return match && Number(match[2]) === parentPid && /(?:^|\/)light-server(?:\s|$)/.test(match[3]) ? [Number(match[1])] : [];
  });
}

async function request(port, method, pathname, body) {
  const response = await fetch(`http://127.0.0.1:${port}${pathname}`, {
    method,
    headers: body === undefined ? {} : { "content-type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (!response.ok) throw new Error(`${method} ${pathname} returned ${response.status}: ${await response.text()}`);
  return response.status === 204 ? undefined : response.json();
}

async function authenticated(token, port, method, pathname, body) {
  const headers = { authorization: `Bearer ${token}` };
  if (body !== undefined) headers["content-type"] = "application/json";
  const response = await fetch(`http://127.0.0.1:${port}${pathname}`, {
    method,
    headers,
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (!response.ok) throw new Error(`${method} ${pathname} returned ${response.status}: ${await response.text()}`);
  return response.status === 204 ? undefined : response.json();
}

async function fetchJson(port, pathname) {
  const response = await fetch(`http://127.0.0.1:${port}${pathname}`);
  if (!response.ok) throw new Error(`GET ${pathname} returned ${response.status}: ${await response.text()}`);
  return response.json();
}

async function freePort() {
  const listener = net.createServer();
  await new Promise((resolve, reject) => { listener.once("error", reject); listener.listen(0, "127.0.0.1", resolve); });
  const port = listener.address().port;
  await new Promise((resolve) => listener.close(resolve));
  return port;
}

async function waitFor(check, timeout, label) {
  await waitForValue(async () => (await check()) ? true : undefined, timeout, label);
}

async function waitForValue(check, timeout, label) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    try {
      const value = await check();
      if (value !== undefined && value !== false) return value;
    } catch {}
    await new Promise((resolve) => setTimeout(resolve, 25));
  }
  throw new Error(`Timed out waiting for ${label}`);
}

async function waitForPortClosed(port, timeout, label) {
  await waitFor(async () => {
    try { await fetch(`http://127.0.0.1:${port}/api/v1/readiness`); return false; }
    catch { return true; }
  }, timeout, label);
}

async function waitForExit(child, timeout, label) {
  if (child.exitCode !== null || child.signalCode !== null) return;
  await Promise.race([
    new Promise((resolve) => child.once("exit", resolve)),
    new Promise((_, reject) => setTimeout(() => reject(new Error(`Timed out waiting for ${label}`)), timeout)),
  ]);
}

function requirePid(child, label) {
  if (!child.pid) throw new Error(`${label} has no PID`);
  return child.pid;
}

function pidAlive(pid) {
  try { process.kill(pid, 0); return true; }
  catch { return false; }
}

function kill(child) {
  if (child && child.exitCode === null && child.signalCode === null) child.kill("SIGKILL");
}

function withLogs(error, ...logs) {
  const detail = logs.filter(Boolean).map((log) => log.slice(-80).join("")).join("\n");
  const message = error instanceof Error ? error.message : String(error);
  return new Error(`${message}${detail ? `\nRecent process output:\n${detail}` : ""}`);
}
