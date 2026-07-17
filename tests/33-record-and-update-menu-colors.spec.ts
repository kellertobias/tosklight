import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import { loadCanonicalCopy } from "./support/catalog";

test("WORKFLOW-COLOR-001 @supplemental-ui › Record red and Update amber remain distinct with and without hardware", async ({ api, bench, desk, page }) => {
  await loadCanonicalCopy(api, bench, "workflow-color-001", "default-stage");
  await desk.open(api.baseUrl);
  await assertWorkflowThemes(page);

  const hardware = await bench.osc();
  await hardware.subscribe(`workflow-colors-${crypto.randomUUID()}`, api.session!.desk.osc_alias);
  try {
    await expect.poll(async () => (await api.request<any>("GET", "/api/v1/bootstrap", undefined, false)).hardware_connected).toBe(true);
    await assertWorkflowThemes(page);
  } finally {
    await hardware.close();
  }
});

async function assertWorkflowThemes(page: any) {
  const rec = page.locator(".global-store-button");
  await rec.dispatchEvent("pointerdown", { pointerId: 1, pointerType: "mouse", button: 0 });
  await page.waitForTimeout(700);
  await rec.dispatchEvent("pointerup", { pointerId: 1, pointerType: "mouse", button: 0 });
  const record = page.getByRole("dialog", { name: "Record Settings" });
  await expect(record).toContainText("RECORD");
  expect(await colors(record)).toMatchObject({ border: "rgb(255, 78, 85)", theme: "#ff4e55" });
  await record.getByRole("button", { name: "Close Record Settings" }).click();

  await page.evaluate(() => window.dispatchEvent(new Event("light:update-settings")));
  const update = page.getByRole("dialog", { name: "Update Settings" });
  await expect(update).toContainText("UPDATE");
  expect(await colors(update)).toMatchObject({ border: "rgb(244, 185, 66)", theme: "#f4b942" });
  await update.getByRole("button", { name: "Close Update Settings" }).click();

  expect((await colors(rec)).border).toBe("rgb(255, 78, 85)");
  await page.evaluate(() => window.dispatchEvent(new CustomEvent("light:update-armed", { detail: true })));
  await expect(rec).toContainText("UPDATE ARMED");
  await expect.poll(async () => (await colors(rec)).border).toBe("rgb(244, 185, 66)");
  await page.evaluate(() => window.dispatchEvent(new CustomEvent("light:update-armed", { detail: false })));
}

async function colors(locator: any) {
  return locator.evaluate((element: HTMLElement) => {
    const style = getComputedStyle(element);
    return { border: style.borderTopColor, theme: style.getPropertyValue("--workflow-theme").trim() };
  });
}
