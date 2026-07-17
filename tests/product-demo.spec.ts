import fs from "node:fs/promises";
import path from "node:path";
import type { Locator, Page } from "@playwright/test";
import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import type { ApiDriver } from "../apps/control-ui/e2e/bench/api";
import { activeShowId, loadCanonicalCopy, programmer } from "./support/catalog";
import {
  demoObjects,
  seedPlannedDemoPatch,
  seedPlannedDemoProgramming,
  seedPlannedDemoRoutes,
} from "./support/plannedDemoState";

const ROOT = path.resolve(import.meta.dirname, "..");
const VIDEO = path.join(ROOT, "artifacts", "product-demo", "tosklight-product-demo.webm");
const SCREENSHOT = path.join(ROOT, "artifacts", "product-demo", "tosklight-product-demo-1920x1080.png");
const RECORDING = process.env.LIGHT_VISUAL_RECORDING === "1";

test("@ui narrates the complete Full HD product demo surface in one regression run", async ({ api, bench, desk, page }, testInfo) => {
  test.setTimeout(RECORDING ? 900_000 : 300_000);
  page.setDefaultTimeout(15_000);
  await loadCanonicalCopy(api, bench, "planned-product-demo", "default-stage");
  const video = page.video();
  try {
    await desk.open(`${bench.baseUrl}/?demo=product`);
    await installTitleOverlay(page);
    const demo = page.getByTestId("product-demo");
    const app = demo.locator(".product-demo-application");
    const keypad = demo.locator(".demo-number-block");
    await expect(demo).toBeVisible();
    await expect(app.locator(".control-section.hardware-connected")).toBeVisible();
    await expect(demo.locator(".stage-3d-canvas canvas")).toBeVisible();
    for (const universe of [1, 2, 3, 4])
      await expect(demo.getByLabel(`Live DMX universe ${universe}`).locator(".product-demo-dmx-cell")).toHaveCount(512);

    await title(page, "SHOW SETUP", "Create an empty show, build the venue and rig, patch the lamps, then configure live network output.");
    const originalShowId = await activeShowId(api);
    await app.getByRole("button", { name: /Open show menu/ }).click();
    await page.locator(".show-modal").getByRole("button", { name: "New Show", exact: true }).click();
    await page.getByRole("dialog", { name: "New show" }).getByRole("button", { name: "Create Empty Show", exact: true }).click();
    await expect.poll(() => activeShowId(api)).not.toBe(originalShowId);
    const showId = await activeShowId(api);
    await page.locator(".show-modal").getByRole("button", { name: "Show Patch", exact: true }).click();
    const patchWindow = app.locator(".show-patch-layout");
    await expect(patchWindow).toBeVisible();
    for (const layer of ["Front Truss", "Back Truss", "Floor", "House Lights", "Stage"]) {
      await patchWindow.getByRole("button", { name: "+ Add layer", exact: true }).click();
      const layerDialog = page.locator(".patch-small-modal", { hasText: "Add layer" });
      await layerDialog.getByLabel("Layer name").fill(layer);
      await layerDialog.getByRole("button", { name: "Add layer", exact: true }).click();
    }
    await expect.poll(async () => (await demoObjects<any>(api, showId, "patch_layer")).map((item) => item.body.name).sort()).toEqual(["Back Truss", "Floor", "Front Truss", "House Lights", "Stage"]);
    const layerObjects = await demoObjects<any>(api, showId, "patch_layer");
    const layerIds = Object.fromEntries(layerObjects.map((item) => [item.body.name, item.id]));

    await title(page, "SHOW SETUP · RIG", "An 8 × 4 metre stage, three four-point trusses, four pipes and two five-metre curtains define the venue.");
    const rig = await seedPlannedDemoPatch(api, showId, layerIds);
    await expect.poll(async () => (await api.request<any>("GET", "/api/v1/patch", undefined, false)).fixtures.length).toBe(80);
    await expect(patchWindow.locator(".ui-window-info")).toContainText("80 fixtures");
    await expect(demo.locator(".stage-3d-canvas canvas")).toBeVisible();

    await title(page, "SHOW SETUP · SAVE", "The complete venue remains autosaved while the provisional show is named Demo Show.");
    await app.getByRole("button", { name: /Open show menu/ }).click();
    await page.locator(".show-modal").getByRole("button", { name: "Save As", exact: true }).click();
    const saveDialog = page.getByRole("dialog", { name: "Save show" });
    await saveDialog.getByLabel("Show name").fill("Demo Show");
    await saveDialog.getByRole("button", { name: "Rename Show", exact: true }).click();
    await expect.poll(async () => (await api.request<any>("GET", "/api/v1/bootstrap", undefined, false)).active_show?.name).toBe("Demo Show");

    await seedPlannedDemoRoutes(api, showId, bench.artnet.port, bench.sacn.port);
    await page.locator(".show-modal").getByRole("button", { name: "Enter Setup", exact: true }).click();
    await app.locator(".setup-window nav").getByRole("button", { name: "Outputs", exact: true }).click();
    const routes = app.getByRole("region", { name: "Output routes" });
    await expect(routes.locator("article")).toHaveCount(3);
    await expect(routes).toContainText("Logical 1 → Art-Net 1");
    await expect(routes).toContainText("Logical 2 → sACN 1");
    await expect(routes).toContainText("Logical 3 → Art-Net 1");

    await title(page, "GROUP PREPARATION", "Fixture-sheet shortcuts expose the working groups; the simulated keypad records the front lights through the operator command path.");
    await openBuiltIn(app, "Fixtures");
    const fixtureWindow = app.locator(".fixture-window");
    await fixtureWindow.getByRole("button", { name: "Settings", exact: true }).click();
    const fixtureSettings = page.getByRole("dialog", { name: "Fixture Sheet Settings" });
    await expect(fixtureSettings).toBeVisible();
    const groupsTab = fixtureSettings.getByRole("tab", { name: "Groups", exact: true });
    await groupsTab.focus();
    await page.keyboard.press("Enter");
    await expect(groupsTab).toHaveAttribute("aria-selected", "true");
    const groupShortcuts = fixtureSettings.getByRole("switch", { name: "Enable group shortcuts" });
    if (!(await groupShortcuts.isChecked())) {
      await groupShortcuts.focus();
      await page.keyboard.press("Space");
    }
    const closeSettings = fixtureSettings.getByRole("button", { name: "Close settings", exact: true });
    await closeSettings.focus();
    await page.keyboard.press("Enter");

    await seedPlannedDemoProgramming(api, showId, rig);
    await keypadCommand(keypad, ["1", "TRU", "8", "ENT"]);
    await keypad.getByRole("button", { name: "RECORD", exact: true }).click();
    await keypadCommand(keypad, ["GRP", "9", "ENT"], false);
    await expect.poll(async () => (await demoObjects<any>(api, showId, "group")).find((item) => item.id === "9")?.body.fixtures.length).toBe(8);
    await keypadCommand(keypad, ["GRP", "1", "DIV", "2", "ENT"]);
    await expect.poll(async () => (await programmer(api)).selected.length).toBeGreaterThan(0);
    const groups = await demoObjects<any>(api, showId, "group");
    expect(Object.fromEntries(groups.map((item) => [item.id, item.body.name]))).toMatchObject({
      "1": "Profiles", "2": "Wash", "3": "LED", "4": "Strips", "9": "Front", "11": "Profiles Odd", "12": "Profiles Even",
    });
    await expect(fixtureWindow.locator(".group-strip")).toContainText("Profiles");

    await title(page, "TURN LIGHTS ON", "With the selection cleared, Control → Special Dialog → Lamps On addresses every compatible lamp across the show.");
    await clearSelection(keypad, api);
    await app.getByRole("button", { name: "Control", exact: true }).click();
    await app.getByRole("button", { name: "Special Dialog", exact: true }).click();
    const specialDialog = page.locator(".special-dialog-card");
    await expect(specialDialog).toContainText("0 fixtures selected");
    await specialDialog.getByRole("button", { name: "Lamps On", exact: true }).click();
    await expect.poll(async () => (await programmer(api)).values.filter((value) => value.attribute === "intensity" || value.attribute === "control.lamp").length).toBeGreaterThan(40);
    await specialDialog.getByRole("button", { name: "×", exact: true }).click();

    await title(page, "PRESET PROGRAMMING", "Seven merged colours, five moving-light positions and portable gobo looks are ready for live programming.");
    await openBuiltIn(app, "Presets");
    const presetWindow = app.locator(".preset-pool-window");
    for (const name of ["Red", "Yellow", "Green", "Cyan", "Blue", "Magenta", "White"])
      await expect(presetWindow.getByRole("button", { name: new RegExp(`${name} Color ·`) })).toBeVisible();
    const presets = await demoObjects<any>(api, showId, "preset");
    expect(presets.filter((item) => item.body.family === "Color")).toHaveLength(7);
    expect(presets.filter((item) => item.body.family === "Position")).toHaveLength(5);

    await title(page, "CUE PROGRAMMING", "A three-step main look, four auto-off colour playbacks and a Reset-wrapped Speed A ACL chaser fill playback page 1.");
    await expect(demo.getByRole("region", { name: "Virtual playback controls" })).toBeVisible();
    const playbackObjects = await demoObjects<any>(api, showId, "playback");
    expect(playbackObjects.map((item) => item.body.number).sort((a, b) => a - b)).toEqual([1, 2, 3, 4, 21, 22, 23, 24]);
    const cueLists = await demoObjects<any>(api, showId, "cue_list");
    expect(cueLists.find((item) => item.body.name === "Demo Main")?.body.cues).toHaveLength(3);
    expect(cueLists.find((item) => item.body.name === "ACL Chase")?.body).toMatchObject({ mode: "chaser", wrap_mode: "reset", speed_group: "A" });

    await title(page, "BUSKING", "Red wash and profiles start together; group masters and the main sequence build the stage before colour playbacks swap live.");
    await demo.getByRole("button", { name: "Playback 21 button 1", exact: true }).click();
    await demo.getByRole("button", { name: "Playback 23 button 1", exact: true }).click();
    for (const slot of [1, 2, 3]) await demo.getByLabel(`Playback ${slot} fader`).fill("1");
    await demo.getByRole("button", { name: "Playback 3 button 1", exact: true }).click();
    await bench.tick(1_200);
    await expect.poll(async () => (await api.request<any>("GET", "/api/v1/dmx", undefined, false)).universes.some((frame: any) => frame.slots.some((value: number) => value > 0))).toBe(true);
    await pause(page, 1_800);

    await demo.getByRole("button", { name: "Playback 3 button 1", exact: true }).click();
    await demo.getByRole("button", { name: "Playback 22 button 1", exact: true }).click();
    await api.request("POST", "/api/v1/playback-pool/21/off", {});
    await expect.poll(async () => activeNumbers(api)).not.toContain(21);
    await demo.getByRole("button", { name: "Playback 24 button 1", exact: true }).click();
    await demo.getByRole("button", { name: "Playback 3 button 1", exact: true }).click();
    await api.request("POST", "/api/v1/playback-pool/23/off", {});
    await expect.poll(async () => activeNumbers(api)).not.toContain(23);
    await api.request("POST", "/api/v1/cuelists/3/off", {});
    await api.request("POST", "/api/v1/cuelists/3/go-to", { cue_number: 2, surface: "physical" });
    await expect.poll(async () => activeNumbers(api)).toContain(3);

    await title(page, "PRELOADING", "Physical playback changes and new moving-light positions are prepared blind, then committed together with a four-second programmer fade.");
    await keypad.getByRole("button", { name: "PRELOAD GO", exact: true }).click();
    await demo.getByRole("button", { name: "Playback 21 button 1", exact: true }).click();
    await demo.getByRole("button", { name: "Playback 24 button 1", exact: true }).click();
    await api.command("programmer.set", { fixture_id: rig.profileTargets[0], attribute: "pan", value: .2 });
    await api.command("programmer.set", { fixture_id: rig.washTargets[0], attribute: "pan", value: .8 });
    const configuration = (await api.request<any>("GET", "/api/v1/configuration")).configuration;
    await api.request("PUT", "/api/v1/configuration", { ...configuration, programmer_fade_millis: 4_000 });
    const pending = await programmer(api);
    expect(pending.preload_playback_pending.length).toBeGreaterThanOrEqual(2);
    expect(pending.preload_pending.length).toBeGreaterThanOrEqual(2);
    await pause(page, 1_500);
    await keypad.getByRole("button", { name: "PRELOAD GO", exact: true }).click();
    await bench.tick(6_000);
    await pause(page, 6_000);

    await title(page, "ACL CHASER · SPEED A", "The final playback starts the alternating ACL fan; two learned taps set its Speed A tempo.");
    await demo.getByRole("button", { name: "Playback 4 button 1", exact: true }).click();
    await api.request("POST", "/api/v1/speed-groups/A/action", { action: "learn", captured_at_millis: 1_000 });
    await api.request("POST", "/api/v1/speed-groups/A/action", { action: "learn", captured_at_millis: 1_650 });
    await expect.poll(async () => activeNumbers(api)).toContain(4);
    await bench.tick(2_000);
    await expect.poll(async () => (await api.request<any>("GET", "/api/v1/dmx", undefined, false)).universes.some((frame: any) => frame.slots.some((value: number) => value > 0))).toBe(true);
    await pause(page, 2_000);

    if (RECORDING) {
      await fs.mkdir(path.dirname(SCREENSHOT), { recursive: true });
      await page.screenshot({ path: SCREENSHOT });
      await testInfo.attach("planned-demo-full-hd-screenshot", { path: SCREENSHOT, contentType: "image/png" });
    }
  } finally {
    if (video) {
      await fs.mkdir(path.dirname(VIDEO), { recursive: true });
      await page.context().close();
      await video.saveAs(VIDEO);
      await testInfo.attach("planned-demo-video", { path: VIDEO, contentType: "video/webm" });
    }
  }
});

async function installTitleOverlay(page: Page): Promise<void> {
  if (!RECORDING) return;
  await page.evaluate(() => {
    const overlay = document.createElement("aside");
    overlay.id = "planned-demo-title";
    overlay.innerHTML = "<strong></strong><span></span>";
    Object.assign(overlay.style, {
      position: "fixed", zIndex: "2147483647", top: "14px", right: "14px", width: "560px",
      padding: "14px 18px", border: "1px solid #57cbd9", borderRadius: "8px", color: "white",
      background: "rgba(5, 12, 16, .9)", boxShadow: "0 8px 32px rgba(0,0,0,.45)", pointerEvents: "none",
      fontFamily: "Inter, system-ui, sans-serif",
    });
    Object.assign((overlay.querySelector("strong") as HTMLElement).style, { display: "block", color: "#73e4ef", fontSize: "18px", letterSpacing: ".08em" });
    Object.assign((overlay.querySelector("span") as HTMLElement).style, { display: "block", marginTop: "5px", fontSize: "13px", lineHeight: "1.35" });
    document.body.append(overlay);
  });
}

async function title(page: Page, heading: string, copy: string): Promise<void> {
  if (!RECORDING) return;
  await page.locator("#planned-demo-title strong").evaluate((node, value) => { node.textContent = value; }, heading);
  await page.locator("#planned-demo-title span").evaluate((node, value) => { node.textContent = value; }, copy);
  await page.waitForTimeout(Number(process.env.LIGHT_VISUAL_STEP_PAUSE ?? 1_200));
}

async function pause(page: Page, millis: number): Promise<void> {
  if (RECORDING) await page.waitForTimeout(millis);
}

async function openBuiltIn(app: Locator, name: string): Promise<void> {
  await app.getByRole("button", { name: "BUILT-INS", exact: true }).click();
  await app.locator(".dock-entry").filter({ hasText: name }).first().click();
}

async function keypadCommand(keypad: Locator, labels: string[], escape = true): Promise<void> {
  if (escape) await keypad.getByRole("button", { name: "ESCAPE", exact: true }).click();
  for (const label of labels) await keypad.getByRole("button", { name: label, exact: true }).click();
}

async function clearSelection(keypad: Locator, api: ApiDriver): Promise<void> {
  for (let attempt = 0; attempt < 3 && (await programmer(api)).selected.length; attempt++)
    await keypad.getByRole("button", { name: "CLR", exact: true }).click();
  await expect.poll(async () => (await programmer(api)).selected).toEqual([]);
}

async function activeNumbers(api: ApiDriver): Promise<number[]> {
  return (await api.request<any>("GET", "/api/v1/playbacks")).active
    .filter((item: any) => item.enabled)
    .map((item: any) => item.playback_number);
}
