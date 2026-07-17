import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import { loadCanonicalCopy } from "./support/catalog";

test("FIXTURE-ADDRESS-001 @supplemental-ui › integrated address screen keeps the complete map and number block reachable", async ({ api, bench, desk, page }) => {
  await loadCanonicalCopy(api, bench, "fixture-address-001", "default-stage");
  await desk.open(api.baseUrl);
  await page.getByRole("button", { name: /Open show menu/ }).click();
  await page.getByRole("button", { name: "Show Patch", exact: true }).click();

  const patchAddress = page.locator(".patch-table .patch-address").first();
  const originalAddress = (await patchAddress.textContent())?.trim();
  await page.getByRole("button", { name: "SET", exact: true }).click();
  await patchAddress.click();

  const dialog = page.getByRole("dialog", { name: "Fixture Address" });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByText("Complete footprint")).toBeVisible();
  await expect(dialog.getByRole("gridcell")).toHaveCount(512);
  await expect(dialog.getByLabel("Fixture address number block")).toBeVisible();
  await expect(dialog.getByRole("button", { name: "Clear address · Unpatch" })).toBeVisible();
  await expect(dialog.getByRole("button", { name: "Set Address" })).toBeVisible();

  const bounds = await dialog.boundingBox();
  expect(bounds).not.toBeNull();
  expect(bounds!.x).toBeGreaterThanOrEqual(0);
  expect(bounds!.y).toBeGreaterThanOrEqual(0);
  expect(bounds!.x + bounds!.width).toBeLessThanOrEqual(page.viewportSize()!.width);
  expect(bounds!.y + bounds!.height).toBeLessThanOrEqual(page.viewportSize()!.height);

  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
  await expect(patchAddress).toHaveText(originalAddress ?? "");
});
