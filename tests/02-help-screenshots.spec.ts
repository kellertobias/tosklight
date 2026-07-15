import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import type { Page } from "@playwright/test";

test.describe.configure({ mode: "serial" });
test.setTimeout(60_000);
test.use({ viewport: { width: 1600, height: 1100 } });

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const SCREENSHOT_DIR = path.join(ROOT, "docs/help/assets/screenshots");

test("captures help and README screenshots from the default show desk", async ({ page, desk }) => {
  await fs.mkdir(SCREENSHOT_DIR, { recursive: true });

  await desk.open("/");
  await page.getByRole("button", { name: "DESKS" }).click();
  await page.getByRole("button", { name: /Programming/ }).click();
  await selectFixtures(page, desk, "1 + 2 + 3 + 4 + 5 + 6");
  await setDimmerByTouch(page, 50);
  await setStagePaneTo3d(page);
  await expect(page.locator(".group-strip .group-card").filter({ hasText: "All Dimmers" })).toBeVisible();
  await expect(page.locator(".control-section")).toContainText("50%");
  await expect(page.locator("canvas")).toBeVisible();
  await page.screenshot({ path: shot("default-desk-overview.png"), fullPage: true });

  await recordPlaybackLook(page, desk);
  await page.getByRole("button", { name: "DESKS" }).click();
  await page.getByRole("button", { name: /Cuelists/ }).click();
  await expect(page.locator(".control-section.playbacks")).toBeVisible();
  await expect(page.locator(".cuelist-window")).toContainText("Cuelist");
  await page.screenshot({ path: shot("cuelist-playback.png"), fullPage: true });

  await page.getByRole("button", { name: "BUILT-INS" }).click();
  await page.locator(".dock-entry").filter({ hasText: "Fixtures" }).click();
  await expect(page.locator(".fixture-window")).toBeVisible();
  await page.screenshot({ path: shot("fixture-sheet-programmer.png"), fullPage: true });

  await page.getByRole("button", { name: /Open show menu/ }).click();
  await page.getByRole("button", { name: "Help", exact: true }).click();
  await expect(page.locator(".help-window")).toBeVisible();
  await page.screenshot({ path: shot("help-command-line.png"), fullPage: true });

  await expect.poll(async () => (await fs.readdir(SCREENSHOT_DIR)).filter((file) => file.endsWith(".png")).length).toBeGreaterThanOrEqual(4);
});

function shot(file: string): string {
  return path.join(SCREENSHOT_DIR, file);
}

async function selectFixtures(page: Page, desk: { command(value: string): Promise<void> }, command: string) {
  await desk.command(command);
  await expect(page.locator(".fixture-window .ui-data-table-row.selected")).toHaveCount(6);
}

async function setDimmerByTouch(page: Page, value: number) {
  const encoder = page.locator(".vertical-touch-fader-stack").filter({ hasText: "Enc 1 · Dimmer" });
  const setValue = encoder.getByRole("button", { name: "Set value" });
  if (await setValue.isVisible()) await setValue.click();
  else await encoder.locator(".vertical-touch-fader").click();
  await expect(page.getByRole("dialog", { name: "Enc 1 · Dimmer value" })).toBeVisible();
  await page.keyboard.type(String(value));
  await page.keyboard.press("Enter");
}

async function setStagePaneTo3d(page: Page) {
  const stagePane = page.locator(".desk-pane").filter({ hasText: "Stage · Main floor" });
  await stagePane.getByRole("button", { name: "Settings" }).click();
  await page.getByRole("tab", { name: "Stage", exact: true }).click();
  await page.getByRole("radio", { name: "3D" }).click();
  await page.getByRole("tab", { name: "Shortcuts", exact: true }).click();
  await page.getByLabel("Show group shortcuts").check({ force: true });
  await page.getByRole("button", { name: "Close settings" }).click();
  await expect(stagePane.locator("canvas")).toBeVisible();
}

async function recordPlaybackLook(page: Page, desk: { command(value: string): Promise<void> }) {
  await page.getByRole("button", { name: "CLR", exact: true }).click();
  await page.getByRole("button", { name: "CLR", exact: true }).click();
  await desk.command("GROUP 1 AT 70");
  await page.getByRole("button", { name: "REC", exact: true }).click();
  await page.getByRole("button", { name: "BUILT-INS" }).click();
  await page.locator(".dock-entry").filter({ hasText: "Cuelists" }).click();
  const firstCuelist = page.locator(".cuelist-card").first();
  await firstCuelist.click();
  await expect(firstCuelist).toContainText("Cuelist");
  await page.getByRole("button", { name: "SET", exact: true }).click();
  await firstCuelist.click();
  await page.locator(".mode-toggle").click();
  await page.getByRole("button", { name: "Assign Cuelist 1 to page 1 playback 1" }).click();
  await expect(page.locator(".playback-fader-bank")).toContainText("Cuelist 1");
  await expect(page.locator(".cuelist-pool-window")).toContainText("Cuelist");
}
