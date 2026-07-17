import type { Page, TestInfo } from "@playwright/test";
import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import { loadCanonicalCopy } from "./support/catalog";

const states = [
  ["dark", "rgb(0, 8, 24)"],
  ["bright", "rgb(255, 255, 255)"],
  ["absent", "transparent"],
  ["mixed", "linear-gradient(90deg, rgb(255, 0, 0) 50%, rgb(0, 0, 255) 50%)"],
] as const;

test("FIXTURE-SHEET-001 @bench › resolved-color dots retain their fill and geometry in software and hardware layouts", async ({ api, bench, desk, page }, testInfo) => {
  await loadCanonicalCopy(api, bench, "fixture-sheet-001", "default-stage");
  await page.setViewportSize({ width: 1600, height: 1100 });
  await desk.open(api.baseUrl);
  await openFixtures(page);
  await assertColorDots(page, "software-1600x1100", testInfo);

  const hardware = await bench.osc();
  const clientId = `fixture-sheet-001-${crypto.randomUUID()}`;
  try {
    await hardware.subscribe(clientId, api.session!.desk.osc_alias);
    await expect.poll(async () => (await api.request<any>("GET", "/api/v1/bootstrap", undefined, false)).hardware_connected).toBe(true);
    await expect(page.locator(".control-section.hardware-connected")).toBeVisible();
    await page.setViewportSize({ width: 1280, height: 720 });
    await assertColorDots(page, "hardware-1280x720", testInfo);
  } finally {
    await hardware.send("/light/unsubscribe", [clientId]).catch(() => undefined);
    await hardware.close();
  }
});

async function openFixtures(page: Page): Promise<void> {
  const entry = page.locator(".dock-entry").filter({ hasText: "Fixtures" }).first();
  if (!await entry.isVisible()) await page.getByRole("button", { name: "BUILT-INS", exact: true }).click();
  await entry.click();
  await expect(page.locator(".fixture-window")).toBeVisible();
  await expect(page.locator(".fixture-window .color-dot").first()).toBeVisible();
}

async function assertColorDots(page: Page, mode: string, testInfo: TestInfo): Promise<void> {
  const rows = page.locator(".fixture-window .ui-data-table-row:not(.header)").filter({ has: page.locator(".color-dot") });
  expect(await rows.count()).toBeGreaterThanOrEqual(states.length);
  await rows.first().click();
  await expect(rows.first()).toHaveClass(/selected/);
  const rowBoundsBefore = await Promise.all(states.map((_, index) => rows.nth(index).boundingBox()));
  const selectionClassesBefore = await Promise.all(states.map((_, index) => rows.nth(index).getAttribute("class")));
  const dots = rows.locator(".color-dot");

  for (const [index, [state, fill]] of states.entries()) {
    await dots.nth(index).evaluate((element, value) => {
      element.setAttribute("data-fixture-color-state", value.state);
      (element as HTMLElement).style.background = value.fill;
    }, { state, fill });
  }

  const presentations = await Promise.all(states.map((_, index) => dots.nth(index).evaluate((element) => {
    const style = getComputedStyle(element);
    const bounds = element.getBoundingClientRect();
    return {
      state: element.getAttribute("data-fixture-color-state"),
      background: style.background,
      borderColor: style.borderColor,
      borderStyle: style.borderStyle,
      borderWidth: style.borderWidth,
      boxShadow: style.boxShadow,
      width: bounds.width,
      height: bounds.height,
    };
  })));

  expect(presentations.map(({ state }) => state)).toEqual(states.map(([state]) => state));
  for (const [index, presentation] of presentations.entries()) {
    const fill = states[index][1];
    expect(presentation.background).toContain(fill.startsWith("linear-gradient") ? "linear-gradient" : fill === "transparent" ? "rgba(0, 0, 0, 0)" : fill);
    expect(presentation).toMatchObject({
      borderColor: "rgb(165, 175, 182)",
      borderStyle: "solid",
      borderWidth: "1px",
      boxShadow: "none",
      width: 16,
      height: 16,
    });
  }
  expect(await Promise.all(states.map((_, index) => rows.nth(index).boundingBox()))).toEqual(rowBoundsBefore);
  expect(await Promise.all(states.map((_, index) => rows.nth(index).getAttribute("class")))).toEqual(selectionClassesBefore);
  await testInfo.attach(`fixture-sheet-color-dots-${mode}`, {
    body: await page.locator(".fixture-window").screenshot(),
    contentType: "image/png",
  });
}
