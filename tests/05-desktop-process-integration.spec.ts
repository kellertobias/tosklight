import { execFile } from "node:child_process";
import path from "node:path";
import { promisify } from "node:util";
import playwright from "../apps/control-ui/node_modules/@playwright/test/index.js";

const { test } = playwright;

const exec = promisify(execFile);
const ROOT = path.resolve(import.meta.dirname, "..");
const DESKTOP_SMOKE = path.join(ROOT, "apps/control-ui/e2e/desktop-smoke.mjs");
const enabled = process.env.LIGHT_DESKTOP_SMOKE === "1";

test.describe("docs/testing/05-virtual-time-persistence-and-recovery.md · packaged desktop", () => {
  test.describe.configure({ mode: "serial" });
  test.skip(!enabled, "Run through npm run test:desktop-smoke after building the macOS app bundle.");
  test.setTimeout(120_000);

  test("DESKTOP-001 @desktop › packaged app owns and terminates its exact child server", async () => {
    await runDesktopScenario("DESKTOP-001");
  });

  test("DESKTOP-002 @desktop › packaged app never adopts or terminates an independent server", async () => {
    await runDesktopScenario("DESKTOP-002");
  });
});

async function runDesktopScenario(scenario: "DESKTOP-001" | "DESKTOP-002"): Promise<void> {
  const { stdout, stderr } = await exec(process.execPath, [DESKTOP_SMOKE, scenario], {
    cwd: ROOT,
    env: process.env,
    timeout: 110_000,
    maxBuffer: 8 * 1024 * 1024,
  });
  await test.info().attach(`${scenario}-process-output`, {
    body: Buffer.from(`${stdout}${stderr}`),
    contentType: "text/plain",
  });
}
