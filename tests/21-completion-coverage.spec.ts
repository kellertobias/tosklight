import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import net from "node:net";
import type { Locator, Page } from "../apps/control-ui/node_modules/@playwright/test/index.js";
import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";

let hardwareServer: ChildProcessWithoutNullStreams | undefined;
let hardwareUrl = "";

test.describe("docs/plans/Done/21-completion-coverage-and-release-verification.DONE.md", () => {
  test.describe.configure({ mode: "serial" });

  test.beforeAll(async () => {
    const port = await freePort();
    hardwareUrl = `http://127.0.0.1:${port}`;
    hardwareServer = spawn(
      "npm",
      ["run", "dev", "--", "--port", String(port), "--strictPort"],
      {
        cwd: new URL("../apps/hardware-controls", import.meta.url).pathname,
        env: { ...process.env, NO_COLOR: "1", FORCE_COLOR: "0" },
        stdio: "pipe",
      },
    );
    await waitForServer(hardwareUrl, hardwareServer);
  });

  test.afterAll(async () => {
    if (!hardwareServer || hardwareServer.exitCode !== null) return;
    hardwareServer.kill("SIGTERM");
    await new Promise<void>((resolve) => {
      const timer = setTimeout(resolve, 2_000);
      hardwareServer!.once("exit", () => {
        clearTimeout(timer);
        resolve();
      });
    });
  });

  test("HIGHLIGHT-006 @ui › the production hardware simulator preserves geometry and sends independent full-height faders", async ({ page }) => {
    await page.setViewportSize({ width: 1600, height: 1100 });
    await installTauriHarness(page);
    await page.goto(hardwareUrl);

    await expect(page.locator(".hardware-number-block")).toBeVisible();
    await expect(page.locator(".hardware-highlight-feedback,.highlight-hardware,[aria-label='Highlight status']")).toHaveCount(0);

    const record = await requiredBox(page.locator('[data-keypad-key="RECORD"]'));
    const preload = await requiredBox(page.locator('[data-keypad-key="PRELOAD GO"]'));
    expect(Math.abs(record.y - preload.y)).toBeLessThanOrEqual(1.5);
    expect(Math.abs(record.width - preload.width)).toBeLessThanOrEqual(1.5);
    expect(Math.abs(record.height - preload.height)).toBeLessThanOrEqual(1.5);
    expect(record.x + record.width).toBeLessThanOrEqual(preload.x);

    const upper = await Promise.all(["HIGH", "PREV", "NEXT", "ALL"].map((key) => requiredBox(page.locator(`[data-keypad-key="${key}"]`))));
    const lower = await Promise.all(["GRP", "CUE", "TIME", "DIV"].map((key) => requiredBox(page.locator(`[data-keypad-key="${key}"]`))));
    for (let index = 0; index < upper.length; index += 1) {
      expect(Math.abs(centerX(upper[index]) - centerX(lower[index]))).toBeLessThanOrEqual(1.5);
      expect(Math.abs(upper[index].width - lower[index].width)).toBeLessThanOrEqual(1.5);
      expect(Math.abs(upper[index].height - lower[index].height)).toBeLessThanOrEqual(1.5);
    }

    const programmerFade = page.locator(".fade-times .time-fader").filter({ hasText: "Prog Fade" });
    const cueFade = page.locator(".fade-times .time-fader").filter({ hasText: "Cue Fade" });
    const programmerBox = await requiredBox(programmerFade);
    const cueBox = await requiredBox(cueFade);
    expect(Math.abs(programmerBox.y - cueBox.y)).toBeLessThanOrEqual(1.5);
    expect(Math.abs(programmerBox.width - cueBox.width)).toBeLessThanOrEqual(1.5);
    expect(Math.abs(programmerBox.height - cueBox.height)).toBeLessThanOrEqual(1.5);
    const fadeArea = await requiredBox(page.locator(".fade-times"));
    expect(programmerBox.y).toBeGreaterThanOrEqual(fadeArea.y);
    expect(cueBox.y + cueBox.height).toBeLessThanOrEqual(fadeArea.y + fadeArea.height + 1.5);

    await clearInvocations(page);
    await setRange(programmerFade.locator('input[type="range"]'), 0.7);
    await expect.poll(() => controlWrites(page, "programmer/prog-fade")).toEqual([0.7]);
    expect(await controlWrites(page, "programmer/cue-fade")).toEqual([]);

    await clearInvocations(page);
    await setRange(cueFade.locator('input[type="range"]'), 0.35);
    await expect.poll(() => controlWrites(page, "programmer/cue-fade")).toEqual([0.35]);
    expect(await controlWrites(page, "programmer/prog-fade")).toEqual([]);

    await clearInvocations(page);
    await setRange(programmerFade.locator('input[type="range"]'), 0);
    await setRange(programmerFade.locator('input[type="range"]'), 1);
    await setRange(cueFade.locator('input[type="range"]'), 0);
    await setRange(cueFade.locator('input[type="range"]'), 1);
    expect(await controlWrites(page, "programmer/prog-fade")).toEqual([0, 1]);
    expect(await controlWrites(page, "programmer/cue-fade")).toEqual([0, 1]);
  });

  test("UPDATE-002 @ui › actual simulator pointer gestures emit complete, mutually exclusive Shift and Record sequences", async ({ page }) => {
    await page.setViewportSize({ width: 1600, height: 1100 });
    await installTauriHarness(page);
    await page.goto(hardwareUrl);
    const shift = page.locator('[data-keypad-key="SHIFT"]');
    const record = page.locator('[data-keypad-key="RECORD"]');

    await clearInvocations(page);
    await pointerDown(shift, 1);
    await pointerPress(record, 2);
    await pointerUp(shift, 1);
    expect(await programmerButtonWrites(page)).toEqual([
      ["programmer/shift", true],
      ["programmer/record", true],
      ["programmer/record", false],
      ["programmer/shift", false],
    ]);

    await clearInvocations(page);
    await pointerDown(shift, 1);
    await pointerPress(record, 2);
    await pointerPress(record, 2);
    await pointerUp(shift, 1);
    expect(await programmerButtonWrites(page)).toEqual([
      ["programmer/shift", true],
      ["programmer/record", true],
      ["programmer/record", false],
      ["programmer/record", true],
      ["programmer/record", false],
      ["programmer/shift", false],
    ]);

    await clearInvocations(page);
    await pointerDown(shift, 1);
    await pointerDown(record, 2);
    await expect(record.getByText("LONG", { exact: true })).toBeVisible({ timeout: 1_000 });
    await pointerUp(record, 2);
    await pointerUp(shift, 1);
    expect(await programmerButtonWrites(page)).toEqual([
      ["programmer/shift", true],
      ["programmer/record", true],
      ["programmer/record", false],
      ["programmer/shift", false],
    ]);
  });
});

async function freePort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      const port = typeof address === "object" && address ? address.port : 0;
      server.close((error) => error ? reject(error) : resolve(port));
    });
  });
}

async function waitForServer(url: string, process: ChildProcessWithoutNullStreams): Promise<void> {
  let output = "";
  process.stdout.on("data", (chunk) => { output += String(chunk); });
  process.stderr.on("data", (chunk) => { output += String(chunk); });
  for (let attempt = 0; attempt < 100; attempt += 1) {
    if (process.exitCode !== null) throw new Error(`Hardware-controls Vite server exited early.\n${output}`);
    try {
      const response = await fetch(url);
      if (response.ok) return;
    } catch {
      // The development server has not opened its socket yet.
    }
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw new Error(`Hardware-controls Vite server did not become ready.\n${output}`);
}

async function installTauriHarness(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const callbacks = new Map<number, (...args: unknown[]) => void>();
    let callbackId = 0;
    // Synthetic multi-touch PointerEvents are not registered as active browser pointers, so the
    // native capture call would reject before React reaches the production onDown handler.
    Element.prototype.setPointerCapture = () => undefined;
    (window as any).__hardwareInvocations = [];
    (window as any).__TAURI_INTERNALS__ = {
      invoke: async (cmd: string, args: unknown) => {
        (window as any).__hardwareInvocations.push({ cmd, args });
        if (cmd === "plugin:event|listen") return 1;
        return null;
      },
      transformCallback: (callback: (...args: unknown[]) => void) => {
        const id = ++callbackId;
        callbacks.set(id, callback);
        return id;
      },
      unregisterCallback: (id: number) => callbacks.delete(id),
    };
    (window as any).__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => undefined };
  });
}

async function requiredBox(locator: Locator) {
  await expect(locator).toBeVisible();
  const box = await locator.boundingBox();
  expect(box).toBeTruthy();
  return box!;
}

function centerX(box: { x: number; width: number }) {
  return box.x + box.width / 2;
}

async function setRange(locator: Locator, value: number) {
  await locator.fill(String(value));
}

async function clearInvocations(page: Page) {
  await page.evaluate(() => { (window as any).__hardwareInvocations = []; });
}

async function controlWrites(page: Page, path: string): Promise<number[]> {
  return page.evaluate((expectedPath: string) => (window as any).__hardwareInvocations
    .filter((entry: any) => entry.cmd === "send_control" && entry.args.path === expectedPath)
    .map((entry: any) => entry.args.args[0]), path);
}

async function programmerButtonWrites(page: Page): Promise<Array<[string, boolean]>> {
  return page.evaluate(() => (window as any).__hardwareInvocations
    .filter((entry: any) => entry.cmd === "send_control" && /^programmer\/(?:shift|record)$/.test(entry.args.path))
    .map((entry: any) => [entry.args.path, entry.args.args[0]]));
}

async function pointerDown(locator: Locator, pointerId: number) {
  await requiredBox(locator);
  await locator.dispatchEvent("pointerdown", {
    pointerId,
    pointerType: "touch",
    isPrimary: pointerId === 1,
    buttons: 1,
  });
}

async function pointerUp(locator: Locator, pointerId: number) {
  await requiredBox(locator);
  await locator.dispatchEvent("pointerup", {
    pointerId,
    pointerType: "touch",
    isPrimary: pointerId === 1,
    buttons: 0,
  });
}

async function pointerPress(locator: Locator, pointerId: number) {
  await pointerDown(locator, pointerId);
  await pointerUp(locator, pointerId);
}
