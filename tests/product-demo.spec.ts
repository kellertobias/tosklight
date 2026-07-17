import fs from "node:fs/promises";
import path from "node:path";
import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import { loadCanonicalCopy } from "./support/catalog";

const ROOT = path.resolve(import.meta.dirname, "..");
const VIDEO = path.join(ROOT, "artifacts", "product-demo", "tosklight-product-demo.webm");
const SCREENSHOT = path.join(ROOT, "artifacts", "product-demo", "tosklight-product-demo-1920x1080.png");

test("@ui records the Full HD product demo surface", async ({ api, bench, desk, page }, testInfo) => {
  test.setTimeout(90_000);
  await loadCanonicalCopy(api, bench, "product-demo", "default-stage");
  const video = page.video();
  try {
    await desk.open(`${bench.baseUrl}/?demo=product`);
    const demo = page.getByTestId("product-demo");
    const keypad = demo.locator(".demo-number-block");
    await expect(demo).toBeVisible();
    await expect(demo.locator(".product-demo-application .control-section.hardware-connected")).toBeVisible();
    await expect(demo.locator(".stage-3d-canvas canvas")).toBeVisible();
    for (const universe of [1, 2, 3, 4])
      await expect(demo.getByLabel(`Live DMX universe ${universe}`).locator(".product-demo-dmx-cell")).toHaveCount(512);
    await expect(demo.locator(".product-demo-dmx-universe-label")).toHaveText(["UNIVERSE 1", "UNIVERSE 2", "UNIVERSE 3", "UNIVERSE 4"]);
    await expect(demo.locator(".product-demo-visual-divider")).toHaveCount(2);
    await expect(demo.getByLabel("Live DMX output above, simulated hardware controls below")).toContainText("SIMULATED HARDWARE CONTROLS");
    await expect(demo.locator(".product-demo-playback-strip")).toHaveCount(4);
    await expect(demo.locator(".product-demo-playback-top-row .product-demo-playback-button")).toHaveCount(4);
    await expect(keypad.getByRole("button", { name: "ESCAPE", exact: true })).toBeVisible();

    for (const label of ["GRP", "1", "AT", "7", "5", "ENT"])
      await keypad.getByRole("button", { name: label, exact: true }).click();
    await api.command("programmer.execute", { value: "1 THRU 6 AT 75" });
    await api.command("programmer.execute", { value: "7 THRU 12 AT 35" });
    await bench.tick(1_000);
    await expect.poll(async () => api.request<any>("GET", "/api/v1/dmx", undefined, false).then((snapshot) =>
      snapshot.universes.find((frame: any) => frame.universe === 1)?.slots.some((value: number) => value > 0) ?? false,
    )).toBe(true);
    await expect.poll(async () => demo.locator(".product-demo-dmx-cell[data-value]:not([data-value='0'])").count()).toBeGreaterThan(0);
    await page.waitForTimeout(1_500);
    if (process.env.LIGHT_VISUAL_RECORDING === "1") {
      await fs.mkdir(path.dirname(SCREENSHOT), { recursive: true });
      await page.screenshot({ path: SCREENSHOT });
      await testInfo.attach("product-demo-full-hd-screenshot", { path: SCREENSHOT, contentType: "image/png" });
    }

    await keypad.getByRole("button", { name: "GRP", exact: true }).click();
    await keypad.getByRole("button", { name: "ESCAPE", exact: true }).click();
    await expect(page.getByLabel("Command line")).not.toHaveValue(/G$/);
    await page.waitForTimeout(900);
  } finally {
    if (video) {
      await fs.mkdir(path.dirname(VIDEO), { recursive: true });
      await page.context().close();
      await video.saveAs(VIDEO);
      await testInfo.attach("product-demo-video", { path: VIDEO, contentType: "video/webm" });
    }
  }
});
