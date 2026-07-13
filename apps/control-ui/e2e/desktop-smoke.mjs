import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import net from "node:net";
import os from "node:os";
import path from "node:path";

if (process.platform !== "darwin") throw new Error("desktop-smoke currently targets the macOS Tauri bundle");
const root = path.resolve(import.meta.dirname, "../../..");
const app = path.join(root, "target/debug/bundle/macos/ToskLight.app/Contents/MacOS/light-control-ui");
const dataDir = await fs.mkdtemp(path.join(os.tmpdir(), "light-desktop-smoke-"));
const marker = path.join(dataDir, "frontend-ready.json");
const port = await freePort();
const child = spawn(app, [], { env: { ...process.env, LIGHT_DESKTOP_TEST_DATA_DIR: dataDir, LIGHT_DESKTOP_TEST_BIND: `127.0.0.1:${port}`, LIGHT_DESKTOP_TEST_READY_FILE: marker, LIGHT_DESKTOP_TEST_AUTO_EXIT: "1" }, stdio: "inherit" });
try {
  await waitFor(async () => JSON.parse(await fs.readFile(marker, "utf8")).ready === true, 15_000, "desktop frontend ready marker");
  await waitFor(async () => (await fetch(`http://127.0.0.1:${port}/api/v1/readiness`)).ok, 5_000, "app-owned server readiness");
  await Promise.race([new Promise((resolve) => child.once("exit", resolve)), rejectAfter(5_000, "desktop app exit")]);
  await waitFor(async () => { try { await fetch(`http://127.0.0.1:${port}/api/v1/readiness`); return false; } catch { return true; } }, 5_000, "app-owned server shutdown");
  console.log("Desktop smoke passed: WebView bootstrapped and the app-owned server shut down with the app.");
} finally {
  if (child.exitCode === null) child.kill("SIGKILL");
  await fs.rm(dataDir, { recursive: true, force: true });
}

async function freePort() {
  const server = net.createServer();
  await new Promise((resolve, reject) => { server.once("error", reject); server.listen(0, "127.0.0.1", resolve); });
  const port = server.address().port;
  await new Promise((resolve) => server.close(resolve));
  return port;
}
async function waitFor(check, timeout, label) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) { try { if (await check()) return; } catch {} await new Promise((resolve) => setTimeout(resolve, 25)); }
  throw new Error(`Timed out waiting for ${label}`);
}
function rejectAfter(timeout, label) { return new Promise((_, reject) => setTimeout(() => reject(new Error(`Timed out waiting for ${label}`)), timeout)); }
