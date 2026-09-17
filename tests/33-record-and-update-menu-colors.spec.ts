import { expect, test } from "./bench/core/fixtures";
import { loadCanonicalCopy } from "./support/catalog";

test("WORKFLOW-COLOR-001 @ui › Record red and Update amber remain distinct with and without hardware", async ({ api, bench, desk, page }) => {
  await loadCanonicalCopy(api, bench, "workflow-color-001", "default-stage");
  await desk.open(api.baseUrl);
  await assertWorkflowThemes(page);

  const hardware = await bench.osc();
  await hardware.subscribe(`workflow-colors-${crypto.randomUUID()}`, "desk");
  try {
    await expect.poll(async () => (await api.request<any>("GET", "/api/v2/bootstrap", undefined, false)).hardware_connected).toBe(true);
    await assertWorkflowThemes(page);
  } finally {
    await hardware.close();
  }
});

async function assertWorkflowThemes(page: any) {
  const rec = page.locator(".global-store-button");
  await rec.dispatchEvent("pointerdown", { pointerId: 1, pointerType: "mouse", button: 0 });
  await page.waitForTimeout(2_600);
  await rec.dispatchEvent("pointerup", { pointerId: 1, pointerType: "mouse", button: 0 });
  const record = page.getByRole("dialog", { name: "Record Settings" });
  await expect(record).toContainText("RECORD");
  expect(await colors(record)).toMatchObject({ border: "rgb(255, 78, 85)", theme: "#ff4e55" });
  const recordTitle = record.locator("header.ui-modal-titlebar");
  await expect(recordTitle.getByRole("heading")).toHaveText("RECORD Settings");
  await expect(record.getByRole("radiogroup", { name: "Default Record mode" })).toBeVisible();
  await expect(record.getByRole("switch", { name: "Merge into active Cue" })).toHaveCount(0);
  await expect(record).toContainText("A Cuelist with one Cue asks whether to add, merge, or overwrite");
  await expect(record).toContainText("the recorded values last for this Cue only");
  await expect(record).not.toContainText("Merge current values into");
  await expect(record.locator(".modal-actions")).toHaveCount(0);
  await recordTitle.getByRole("button", { name: "Done", exact: true }).click();
  await expect(record).toBeHidden();

  await rec.dispatchEvent("pointerdown", {
    pointerId: 2,
    pointerType: "mouse",
    button: 0,
    shiftKey: true,
  });
  await page.waitForTimeout(2_600);
  await rec.dispatchEvent("pointerup", { pointerId: 2, pointerType: "mouse", button: 0 });
  const update = page.getByRole("dialog", { name: "Update Settings" });
  await expect(update).toContainText("UPDATE");
  expect(await colors(update)).toMatchObject({ border: "rgb(244, 185, 66)", theme: "#f4b942" });
  const updateTitle = update.locator("header.ui-modal-titlebar");
  await expect(updateTitle.getByRole("heading")).toHaveText("UPDATE Settings");
  await expect(updateTitle.getByRole("button", { name: "Done", exact: true })).toBeEnabled();
  await expect(update.locator(".modal-actions")).toHaveCount(0);
  await updateTitle.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(update).toBeHidden();

  expect((await colors(rec)).border).toBe("rgb(255, 78, 85)");
  await page.waitForTimeout(1_050);
  await rec.click({ modifiers: ["Shift"] });
  await expect(rec).toContainText("UPDATE");
  await expect.poll(async () => (await colors(rec)).border).toBe("rgb(244, 185, 66)");
  await rec.click({ modifiers: ["Shift"] });
  const choice = page.getByRole("dialog", { name: "Update", exact: true });
  expect(await colors(choice)).toMatchObject({ border: "rgb(244, 185, 66)", theme: "#f4b942" });
  await choice.locator("header.ui-modal-titlebar").getByRole("button", { name: "Targets", exact: true }).click();
  const targets = page.getByRole("dialog", { name: "Update Targets" });
  const targetsTitle = targets.locator("header.ui-modal-titlebar");
  await expect(targetsTitle.getByRole("heading")).toHaveText("UPDATE Targets");
  await targetsTitle.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(targets).toBeHidden();
  await expect(rec).toHaveText("REC");
}

async function colors(locator: any) {
  return locator.evaluate((element: HTMLElement) => {
    const style = getComputedStyle(element);
    return { border: style.borderTopColor, theme: style.getPropertyValue("--workflow-theme").trim() };
  });
}
