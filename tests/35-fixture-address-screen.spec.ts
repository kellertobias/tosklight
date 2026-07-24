import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import { loadCanonicalCopy } from "./support/catalog";
import { readPatchSnapshot } from "./support/operator/patch";

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

test("PATCH-PLACEMENT-001 @supplemental-ui › the server commits the independently arranged placement preview", async ({
  api,
  bench,
  desk,
  page,
}) => {
  const show = await loadCanonicalCopy(api, bench, "patch-placement-001", "default-stage");
  await desk.open(api.baseUrl);
  await page.getByRole("button", { name: /Open show menu/ }).click();
  await page.getByRole("button", { name: "Show Patch", exact: true }).click();

  await desk.click(page.getByRole("button", { name: "+ Add fixture", exact: true }));
  const browser = page.locator(".fixture-browser-modal");
  await browser.getByRole("textbox", { name: "Search", exact: true }).fill("Dimmer");
  const family = browser
    .locator(".fixture-picker-columns > section")
    .nth(1)
    .getByRole("button")
    .filter({ has: page.getByText("Dimmer", { exact: true }) })
    .first();
  await desk.click(family);
  const mode = browser.locator(".fixture-mode-detail select");
  const eightBit = await mode.locator("option").evaluateAll((options) =>
    options.find((option) => option.textContent?.startsWith("8-bit"))?.getAttribute("value"),
  );
  expect(eightBit).toBeTruthy();
  await mode.selectOption(eightBit!);
  await desk.click(
    browser.locator(".fixture-mode-detail").getByRole("button", {
      name: "Add fixture",
      exact: true,
    }),
  );

  const placement = page.locator(".fixture-placement-modal");
  await placement.getByRole("textbox", { name: /^Fixture name\b/ }).fill("Placement");
  await placement.getByRole("textbox", { name: "Start fixture ID", exact: true }).fill("9000");
  await placement.getByRole("textbox", { name: "Count", exact: true }).fill("3");
  await placement
    .getByRole("textbox", { name: /^Address \(universe\.address\)/ })
    .fill("32.1");

  const grid = placement.getByRole("grid", { name: "DMX universe 32" });
  const second = grid.getByRole("gridcell", { name: /Fixture 9001\b/ });
  const destination = grid.locator('[data-dmx-address="50"]');
  await desk.click(second);
  await desk.click(destination);

  const preview = await Promise.all(
    [9000, 9001, 9002].map(async (fixtureNumber) =>
      Number(
        await grid
          .getByRole("gridcell", { name: new RegExp(`Fixture ${fixtureNumber}\\b`) })
          .getAttribute("data-dmx-address"),
      ),
    ),
  );
  expect(preview).toEqual([1, 50, 3]);

  await desk.click(
    placement.getByRole("button", { name: "Add 3 fixtures", exact: true }),
  );
  await expect(placement).toBeHidden();

  const snapshot = await readPatchSnapshot(api, show.id);
  const authoritative = snapshot.fixtures
    .filter(
      (fixture) =>
        fixture.fixture_number != null &&
        fixture.fixture_number >= 9000 &&
        fixture.fixture_number <= 9002,
    )
    .sort((left, right) => left.fixture_number! - right.fixture_number!)
    .map((fixture) => fixture.split_patches[0]?.address);
  expect(authoritative).toEqual(preview);
});
