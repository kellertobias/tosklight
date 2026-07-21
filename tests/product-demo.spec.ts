import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import type { Locator, Page } from "@playwright/test";
import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import type { ApiDriver } from "../apps/control-ui/e2e/bench/api";
import type { DeskDriver } from "../apps/control-ui/e2e/bench/desk";
import { activeShowId, loadCanonicalCopy, programmer } from "./support/catalog";
import {
  aimFixtureAt,
  demoObjects,
  ensurePlannedDemoFixtureLibrary,
  type PlannedDemoPatchPhase,
  seedPlannedDemoPatch,
  seedPlannedDemoProgramming,
} from "./support/plannedDemoState";
import artifactResolver from "../tools/artifact-paths.cjs";

const { artifactPaths } = artifactResolver;

const VIDEO = path.join(artifactPaths.visual, "product-demo", "tosklight-product-demo.webm");
const SCREENSHOT = path.join(artifactPaths.visual, "product-demo", "tosklight-product-demo-1920x1080.png");
const DEMO_SHOW = fileURLToPath(new URL("../assets/demo.show", import.meta.url));
const RECORDING = process.env.LIGHT_VISUAL_RECORDING === "1";
const UPDATE_DEMO_SHOW = process.env.LIGHT_UPDATE_DEMO_SHOW === "1";

test("@ui narrates the complete Full HD product demo surface in one regression run", async ({ api, bench, desk, page }, testInfo) => {
  test.setTimeout(RECORDING ? 900_000 : 300_000);
  page.setDefaultTimeout(15_000);
  await loadCanonicalCopy(api, bench, "planned-product-demo", "default-stage");
  const video = page.video();
  let completedShow: Buffer | null = null;
  try {
    await desk.open(`${bench.baseUrl}/?demo=product`);
    const demo = page.getByTestId("product-demo");
    const app = demo.locator(".product-demo-application");
    const screenFrame = demo.locator(".product-demo-screen-frame");
    const keypad = demo.locator(".demo-number-block");
    await expect(demo).toBeVisible();
    await expect(screenFrame).toHaveCSS("border-left-width", "10px");
    const appBox = await app.boundingBox();
    expect(appBox).not.toBeNull();
    expect(appBox!.width / appBox!.height).toBeCloseTo(16 / 9, 2);
    await expect(demo.locator("[data-demo-chapter]")).toHaveCount(8);
    await expect(app.locator(".control-section.hardware-connected")).toBeVisible();
    const stage = demo.locator(".stage-3d-canvas");
    const stageCanvas = stage.locator("canvas");
    await expect(stageCanvas).toBeVisible();
    await stageCanvas.evaluate((canvas) => { canvas.dataset.recordingCanvas = "stable"; });
    await expect(stage).toHaveAttribute("data-camera-position", "0,1.625,8");
    await expect(stage).toHaveAttribute("data-camera-target", "0,2.6,-4");
    await expect(stage).toHaveAttribute("data-environment-brightness", "1");
    await expect(stage).toHaveAttribute("data-floor-grid", "off");
    await expect(stage).toHaveAttribute("data-beam-guides", "off");
    for (const universe of [1, 2, 3, 4])
      await expect(demo.getByLabel(`Live DMX universe ${universe}`).locator(".product-demo-dmx-cell")).toHaveCount(512);
    await desk.titleCard("SHOW SETUP", "Create an empty show, build the venue and rig, patch the lamps, then configure live network output.");
    await expect(demo.locator('[data-demo-chapter="SHOW SETUP"]')).toHaveClass(/active/);
    await expect(demo.locator("[data-demo-current-action]")).toContainText("Create an empty show");
    if (RECORDING) {
      await expect(page.locator("#light-recording-title-card")).toContainText("SHOW SETUP");
      await expect(page.locator("#light-recording-title-card")).toHaveAttribute("aria-hidden", "true");
    }
    const originalShowId = await activeShowId(api);
    const showMenuButton = app.getByRole("button", { name: /Open show menu/ });
    await desk.click(showMenuButton);
    await expect(demo.locator("[data-demo-current-action]")).toContainText("Open show menu");
    if (RECORDING) await expect(page.locator("#light-recording-click-layer")).toHaveAttribute("data-click-count", "1");
    await pause(page, 300);
    await desk.click(page.locator(".show-modal").getByRole("button", { name: "New Show", exact: true }));
    await pause(page, 250);
    await desk.click(page.getByRole("dialog", { name: "New show" }).getByRole("button", { name: "Create Empty Show", exact: true }));
    await expect.poll(() => activeShowId(api)).not.toBe(originalShowId);
    await pause(page, 350);
    const showId = await activeShowId(api);
    await desk.click(page.locator(".show-modal").getByRole("button", { name: "Show Patch", exact: true }));
    const patchWindow = app.locator(".show-patch-layout");
    await expect(patchWindow).toBeVisible();
    await expect(stage).toHaveAttribute("data-beam-guides", "on");
    await pause(page, 350);
    for (const layer of ["Front Truss", "Back Truss", "Floor", "House Lights", "Stage"]) {
      await desk.click(patchWindow.getByRole("button", { name: "+ Add layer", exact: true }));
      await pause(page, 120);
      const layerDialog = page.locator(".patch-small-modal", { hasText: "Add layer" });
      await layerDialog.getByLabel("Layer name").fill(layer);
      await pause(page, 100);
      await desk.click(layerDialog.getByRole("button", { name: "Add layer", exact: true }));
      await pause(page, 180);
    }
    await expect.poll(async () => (await demoObjects<any>(api, showId, "patch_layer")).map((item) => item.body.name).sort()).toEqual(["Back Truss", "Floor", "Front Truss", "House Lights", "Stage"]);
    const layerObjects = await demoObjects<any>(api, showId, "patch_layer");
    const layerIds = Object.fromEntries(layerObjects.map((item) => [item.body.name, item.id]));

    await desk.fastForward("Loading fixture profiles for the visible patch workflow.", async () => {
      const fastForward = demo.locator("#light-recording-fast-forward");
      await expect(fastForward).toHaveAttribute("data-placement", "narrative");
      const [fastForwardBox, applicationBox] = await Promise.all([fastForward.boundingBox(), app.boundingBox()]);
      expect(fastForwardBox).not.toBeNull();
      expect(applicationBox).not.toBeNull();
      expect(fastForwardBox!.y).toBeGreaterThanOrEqual(applicationBox!.y + applicationBox!.height);
      return ensurePlannedDemoFixtureLibrary(api);
    });

    await desk.titleCard("SHOW SETUP · STAGE", "Add the first stage deck through Show Patch, place it by hand, then build the remaining deck as physical multi-patches.");
    await addFixtureViaUi(desk, page, patchWindow, "Stage", {
      search: "Stage Element 2 × 1 m", family: "Stage Element 2 × 1 m", mode: "50 cm",
      name: "Stage", fixtureNumber: "0.1",
    });
    await positionFixtureViaUi(desk, page, keypad, fixtureRow(patchWindow, "0.1"), { x: -3, y: .5, z: 0 });
    await fastForwardPatchPhase(desk, page, api, showId, patchWindow, layerIds, "stage", ["Stage"], "Building the remaining stage decks, trusses and curtains via API.");

    await desk.titleCard("SHOW SETUP · ACL", "Patch both ACL control fixtures first, place the first ACL by hand, then reveal the two tightly mounted physical fans.");
    await addFixtureViaUi(desk, page, patchWindow, "Back Truss", {
      search: "Dimmer PAR Can", family: "Dimmer PAR Can", mode: "8-bit",
      name: "ACL In", fixtureNumber: 81, patch: "1.1", count: 2,
    });
    const firstAclLocation = { x: -.4, y: 3.8, z: 4.5 };
    await positionFixtureViaUi(desk, page, keypad, fixtureRow(patchWindow, 81), firstAclLocation, aimFixtureAt({ x: -.4, y: 4, z: 4.3 }, { x: -3.8, y: -2, z: 0 }));
    await fastForwardPatchPhase(desk, page, api, showId, patchWindow, layerIds, "acl", ["Back Truss"], "Completing the centered ACL fan-out and the two 80 cm-wide outside ACL clusters.");
    await fastForwardPatchPhase(desk, page, api, showId, patchWindow, layerIds, "strips", ["Stage", "Back Truss"], "Mounting four vertical pipes and two vertical Sunstrips per pipe.");

    await desk.titleCard("SHOW SETUP · FRONT", "Patch the first front light by hand, aim it across the stage, then complete the mirrored left and right fans.");
    await addFixtureViaUi(desk, page, patchWindow, "Front Truss", {
      search: "Dimmer Fresnel", family: "Dimmer Fresnel", mode: "8-bit",
      name: "Front Left 1", fixtureNumber: 1, patch: "2.1",
    });
    const firstFrontLocation = { x: -3.8, y: -3, z: 4 };
    await positionFixtureViaUi(desk, page, keypad, fixtureRow(patchWindow, 1), firstFrontLocation, aimFixtureAt(firstFrontLocation, { x: -3.8, y: 1.5, z: 0 }));
    await fastForwardPatchPhase(desk, page, api, showId, patchWindow, layerIds, "front", ["Front Truss"], "Completing the mirrored front-light fans across the stage corners.");

    await desk.titleCard("SHOW SETUP · FLOOR", "Patch and aim the first floor LED by hand, then build four aligned groups of four.");
    await addFixtureViaUi(desk, page, patchWindow, "Floor", {
      search: "RGBW LED", family: "RGBW LED", mode: "DRGBW 8-bit dimmer first",
      name: "Floor Spot 1", fixtureNumber: 301, patch: "3.241",
    });
    const firstFloorLocation = { x: -3.3, y: 3.9, z: .6 };
    await positionFixtureViaUi(desk, page, keypad, fixtureRow(patchWindow, 301), firstFloorLocation, aimFixtureAt({ x: -3.3, y: 1.6, z: .2 }, { x: -4.1, y: -3, z: 4 }));
    await fastForwardPatchPhase(desk, page, api, showId, patchWindow, layerIds, "floor", ["Floor"], "Completing four floor-LED groups with shared depth and height.");

    await desk.titleCard("SHOW SETUP · HOUSE", "Patch the first house light and its first physical multi-patch through the desk UI.");
    await addFixtureViaUi(desk, page, patchWindow, "House Lights", {
      search: "Generic Dimmer", family: "Dimmer", mode: "8-bit",
      name: "House Light", fixtureNumber: 99, patch: "2.13",
    });
    const houseLightRow = fixtureRow(patchWindow, 99);
    await positionFixtureViaUi(desk, page, keypad, houseLightRow, { x: 0, y: -7, z: 5 });
    await desk.click(houseLightRow);
    await desk.click(patchWindow.getByRole("button", { name: "+ Add multi-patch", exact: true }));
    const firstMultipatch = patchWindow.locator(".multipatch-row").last();
    await desk.click(firstMultipatch.locator(".patch-address"));
    const multipatchAddress = page.getByRole("dialog", { name: "Multi-patch Address" });
    for (const key of ["2", "Universe separator", "1", "4"])
      await desk.click(multipatchAddress.getByRole("button", { name: key === "Universe separator" ? key : `Address ${key}`, exact: true }));
    await desk.click(multipatchAddress.getByRole("button", { name: "Set Address", exact: true }));
    await positionMultipatchViaUi(desk, page, firstMultipatch, { x: 0, y: -6, z: 5 });
    await fastForwardPatchPhase(desk, page, api, showId, patchWindow, layerIds, "house", ["House Lights"], "Completing the repeated house-light and house-mood multi-patches.");

    const rig = await fastForwardPatchPhase(
      desk,
      page,
      api,
      showId,
      patchWindow,
      layerIds,
      "remaining",
      ["Back Truss", "Front Truss", "Floor"],
      "Adding the remaining profiles, washes, blinders and haze while the patch visibly fills.",
      (generatedRig) => seedPlannedDemoProgramming(api, showId, generatedRig),
    );
    await expect.poll(async () => (await api.request<any>("GET", "/api/v1/patch", undefined, false)).fixtures.length).toBe(66);
    await expect(patchWindow.locator(".ui-window-info")).toContainText("66 fixtures");
    const physicalCount = (await api.request<any>("GET", "/api/v1/patch", undefined, false)).fixtures
      .reduce((count: number, fixture: any) => count + 1 + (fixture.multipatch?.length ?? 0), 0);
    expect(physicalCount).toBe(114);
    const physicalInstances = (fixtureNumber: number) => {
      const fixture = rig.fixtures[fixtureNumber];
      return [{ location: fixture.location, rotation: fixture.rotation }, ...(fixture.multipatch ?? [])];
    };
    const stageDeck = physicalInstances(10_001);
    expect(stageDeck).toHaveLength(16);
    expect(stageDeck.map((item) => item.location.y).sort((left, right) => left - right)).toEqual([
      500, 500, 500, 500, 1500, 1500, 1500, 1500, 2500, 2500, 2500, 2500, 3500, 3500, 3500, 3500,
    ]);
    const curtains = [10_009, 10_010, 10_011].map((number) => rig.fixtures[number]);
    expect(curtains.map((fixture) => fixture.location)).toEqual([
      { x: -2000, y: 4300, z: -500 }, { x: 0, y: 4300, z: -500 }, { x: 2000, y: 4300, z: -500 },
    ]);
    const aclIn = physicalInstances(81);
    const aclOut = physicalInstances(82);
    expect(aclIn).toHaveLength(8);
    expect(Math.max(...aclIn.map((item) => item.location.x)) - Math.min(...aclIn.map((item) => item.location.x))).toBe(800);
    expect(new Set(aclIn.map((item) => `${item.location.y}:${item.location.z}`))).toEqual(new Set(["3800:4500"]));
    expect(aclOut).toHaveLength(8);
    expect(aclOut[3].location.x - aclOut[0].location.x).toBe(800);
    expect(aclOut[7].location.x - aclOut[4].location.x).toBe(800);
    expect(new Set(aclOut.map((item) => `${item.location.y}:${item.location.z}`))).toEqual(new Set(["3800:4500"]));
    for (const fixtureNumber of [10_005, 10_006, 10_007, 10_008]) expect(rig.fixtures[fixtureNumber].rotation.y).toBe(90);
    const strips = Array.from({ length: 8 }, (_, index) => rig.fixtures[401 + index]);
    expect(strips.map((fixture) => fixture.location.x)).toEqual([-1500, -1500, -500, -500, 500, 500, 1500, 1500]);
    expect(strips.map((fixture) => fixture.location.z)).toEqual([2850, 1700, 2850, 1700, 2850, 1700, 2850, 1700]);
    expect(strips.every((fixture) => fixture.rotation.y === 90)).toBe(true);
    const frontLeftAim = [1, 2, 3, 4].map((number) => rig.fixtures[number].rotation.y);
    const frontRightAim = [5, 6, 7, 8].map((number) => rig.fixtures[number].rotation.y);
    for (let index = 0; index < 4; index++) expect(frontLeftAim[index]).toBeCloseTo(-frontRightAim[3 - index], 6);
    const floor = Array.from({ length: 16 }, (_, index) => rig.fixtures[301 + index]);
    expect(new Set(floor.map((fixture) => `${fixture.location.y}:${fixture.location.z}`))).toEqual(new Set(["3900:600"]));
    expect(floor.map((fixture) => fixture.location.x)).toEqual([-3300, -3100, -2900, -2700, -1300, -1100, -900, -700, 700, 900, 1100, 1300, 2700, 2900, 3100, 3300]);
    const houseLight = physicalInstances(99);
    expect(houseLight.map((item) => `${item.location.y}:${item.location.z}`)).toEqual(["-7000:5000", "-6000:5000", "-5000:5000", "-4000:5000"]);
    const houseMood = physicalInstances(98);
    expect(houseMood).toHaveLength(8);
    expect(houseMood.map((item) => item.location.x)).toEqual([-3500, -2500, -1500, -500, 500, 1500, 2500, 3500]);
    expect(new Set(houseMood.map((item) => `${item.location.y}:${item.location.z}`))).toEqual(new Set(["-5000:4000"]));
    for (const fixtureNumber of [801, 802]) {
      expect(rig.fixtures[fixtureNumber].location.y).toBe(-2000);
      expect(rig.fixtures[fixtureNumber].rotation).toEqual({ x: -20, y: 0, z: 0 });
    }
    await expect(demo.locator(".stage-3d-canvas canvas")).toBeVisible();
    await expect(demo.locator(".stage-3d-canvas canvas")).toHaveAttribute("data-recording-canvas", "stable");

    await desk.titleCard("SHOW SETUP · SAVE", "The complete venue remains autosaved while the provisional show is named Demo Show.");
    await desk.click(app.getByRole("button", { name: /Open show menu/ }));
    await desk.click(page.locator(".show-modal").getByRole("button", { name: "Save As", exact: true }));
    const saveDialog = page.getByRole("dialog", { name: "Save show" });
    await saveDialog.getByLabel("Show name").fill("Demo Show");
    await desk.click(saveDialog.getByRole("button", { name: "Name Empty Show", exact: true }));
    await expect.poll(async () => (await api.request<any>("GET", "/api/v1/bootstrap", undefined, false)).active_show?.name).toBe("Demo Show");

    await desk.click(page.locator(".show-modal").getByRole("button", { name: "Enter Setup", exact: true }));
    await expect(stage).toHaveAttribute("data-beam-guides", "off");
    await desk.click(app.locator(".setup-window nav").getByRole("button", { name: "Outputs", exact: true }));
    const routes = app.getByRole("region", { name: "Output routes" });
    await createOutputRoute(desk, page, routes, "Art-Net", 1, 1, bench.artnet.port);
    await createOutputRoute(desk, page, routes, "sACN", 2, 1, bench.sacn.port);
    await createOutputRoute(desk, page, routes, "Art-Net", 3, 1, bench.artnet.port);
    await expect(routes.locator("article")).toHaveCount(3);
    await expect(routes).toContainText("Logical 1 → Art-Net 1");
    await expect(routes).toContainText("Logical 2 → sACN 1");
    await expect(routes).toContainText("Logical 3 → Art-Net 1");

    await desk.titleCard("GROUP PREPARATION", "Fixture-sheet shortcuts expose the working groups; the simulated keypad records the front lights through the operator command path.");
    await openBuiltIn(desk, app, "Fixtures");
    const fixtureWindow = app.locator(".fixture-window");
    await desk.click(fixtureWindow.getByRole("button", { name: "Settings", exact: true }));
    const fixtureSettings = page.getByRole("dialog", { name: "Fixture Sheet" });
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

    await clearSelection(desk, keypad, api);
    for (const fixture of Array.from({ length: 8 }, (_, index) => index + 1))
      await desk.click(fixtureSheetRow(fixtureWindow, rig.fixtures[fixture].fixture_id));
    await desk.click(keypad.getByRole("button", { name: "RECORD", exact: true }));
    await desk.click(fixtureWindow.locator(".group-strip .group-card").nth(8));
    await chooseRecordMode(desk, page, "Overwrite");
    await expect.poll(async () => (await demoObjects<any>(api, showId, "group")).find((item) => item.id === "9")?.body.fixtures.length).toBe(8);

    await clearSelection(desk, keypad, api);
    await keypadCommand(desk, keypad, ["2", "0", "1", "TRU", "2", "0", "7", "ENT"]);
    await expect.poll(async () => (await programmer(api)).selected.length).toBe(rig.washTargets.length);
    await keypadCommand(desk, keypad, ["RECORD", "GRP", "2", "ENT"], false);
    await expect(keypad.getByRole("button", { name: "RECORD", exact: true })).toHaveAttribute("aria-pressed", "false");

    await keypadCommand(desk, keypad, ["GRP", "1", "DIV", "2", "ENT"]);
    await expect.poll(async () => (await programmer(api)).selected.length).toBe(4);
    await desk.click(keypad.getByRole("button", { name: "RECORD", exact: true }));
    await keypadCommand(desk, keypad, ["GRP", "1", "1", "ENT"], false);
    await expect(keypad.getByRole("button", { name: "RECORD", exact: true })).toHaveAttribute("aria-pressed", "false");

    await desk.click(fixtureWindow.locator(".group-strip .group-card").nth(0));
    await keypadCommand(desk, keypad, ["DIV", "DIV", "ENT"], false);
    await expect.poll(async () => (await programmer(api)).selected.length).toBe(4);
    await desk.click(keypad.getByRole("button", { name: "RECORD", exact: true }));
    await desk.click(fixtureWindow.locator(".group-strip .group-card").nth(11));
    await chooseRecordMode(desk, page, "Overwrite");
    await expect.poll(async () => (await programmer(api)).selected.length).toBe(4);
    const groups = await demoObjects<any>(api, showId, "group");
    expect(Object.fromEntries(groups.map((item) => [item.id, item.body.name]))).toMatchObject({
      "1": "Profiles", "2": "Wash", "3": "LED", "4": "Strips", "9": "Front", "11": "Profiles Odd", "12": "Profiles Even",
    });
    await expect(fixtureWindow.locator(".group-strip")).toContainText("Profiles");

    await desk.titleCard("LAMP CONTROL", "Lamps On sends only authored discharge-lamp strike commands. It never highlights LEDs or conventional dimmers.");
    await clearSelection(desk, keypad, api);
    const programmerBeforeLampControl = await programmer(api);
    await desk.click(app.getByRole("button", { name: "Control", exact: true }));
    await desk.click(app.getByRole("button", { name: "Special Dialog", exact: true }));
    const specialDialog = page.locator(".special-dialog-card");
    await expect(specialDialog).toContainText("0 fixtures selected");
    await desk.click(specialDialog.getByRole("button", { name: "Lamps On", exact: true }));
    await expect.poll(async () => await programmer(api)).toEqual(programmerBeforeLampControl);
    await desk.click(specialDialog.getByRole("button", { name: "×", exact: true }));

    await desk.titleCard("PRESET PROGRAMMING", "Seven merged colours, five moving-light positions and portable gobo looks are ready for live programming.");
    await desk.click(fixtureWindow.locator(".group-strip .group-card").nth(1));
    await expect.poll(async () => (await programmer(api)).selected.length).toBe(rig.washTargets.length);
    await openBuiltIn(desk, app, "Presets");
    const presetWindow = app.locator(".preset-pool-window");
    await desk.click(presetWindow.getByRole("button", { name: "Color", exact: true }));
    await desk.click(app.getByRole("button", { name: "Color", exact: true }).last());
    await setEncoderValue(desk, page, "Enc 1 · color red", 100);
    await setEncoderValue(desk, page, "Enc 2 · color green", 0);
    await setEncoderValue(desk, page, "Enc 3 · color blue", 0);
    await desk.click(keypad.getByRole("button", { name: "RECORD", exact: true }));
    await desk.click(presetWindow.locator(".preset-card").nth(0));
    await expect.poll(async () => (await demoObjects<any>(api, showId, "preset")).some((item) => item.id === "2.1")).toBe(true);
    await setPresetButtonTitle(desk, page, keypad, presetWindow, 0, "Red");

    await setEncoderValue(desk, page, "Enc 1 · color red", 0);
    await setEncoderValue(desk, page, "Enc 3 · color blue", 100);
    await keypadCommand(desk, keypad, ["RECORD", "2", ".", "5", "ENT"]);
    await expect.poll(async () => (await demoObjects<any>(api, showId, "preset")).some((item) => item.id === "2.5")).toBe(true);
    await setPresetButtonTitle(desk, page, keypad, presetWindow, 4, "Blue");
    for (const name of ["Red", "Yellow", "Green", "Cyan", "Blue", "Magenta", "White"])
      await expect(presetWindow.getByRole("button", { name: new RegExp(`${name} Color ·`) })).toBeVisible();
    const presets = await demoObjects<any>(api, showId, "preset");
    expect(presets.filter((item) => item.body.family === "Color")).toHaveLength(7);
    expect(presets.filter((item) => item.body.family === "Position")).toHaveLength(5);
    await clearProgrammer(desk, keypad, api);

    await desk.titleCard("CUE PROGRAMMING", "A three-step main look, four auto-off colour playbacks and a Reset-wrapped Speed A ACL chaser fill playback page 1.");
    await expect(demo.getByRole("region", { name: "Virtual playback controls" })).toBeVisible();
    const playbackObjects = await demoObjects<any>(api, showId, "playback");
    expect(playbackObjects.map((item) => item.body.number).sort((a, b) => a - b)).toEqual([1, 2, 3, 4, 21, 22, 23, 24]);
    const cueLists = await demoObjects<any>(api, showId, "cue_list");
    expect(cueLists.find((item) => item.body.name === "Demo Main")?.body.cues).toHaveLength(3);
    expect(cueLists.find((item) => item.body.name === "ACL Chase")?.body).toMatchObject({ mode: "chaser", wrap_mode: "reset", speed_group: "A" });

    await desk.titleCard("BUSKING", "Red wash and profiles start together; group masters and the main sequence build the stage before colour playbacks swap live.");
    await desk.click(demo.getByRole("button", { name: "Playback 21 button 1", exact: true }));
    await desk.click(demo.getByRole("button", { name: "Playback 23 button 1", exact: true }));
    for (const slot of [1, 2, 3]) await demo.getByLabel(`Playback ${slot} fader`).fill("1");
    await desk.click(demo.getByRole("button", { name: "Playback 3 button 1", exact: true }));
    await bench.tick(1_200);
    await expect.poll(async () => (await api.request<any>("GET", "/api/v1/dmx", undefined, false)).universes.some((frame: any) => frame.slots.some((value: number) => value > 0))).toBe(true);
    await pause(page, 1_800);

    await desk.click(demo.getByRole("button", { name: "Playback 3 button 1", exact: true }));
    await desk.click(demo.getByRole("button", { name: "Playback 22 button 1", exact: true }));
    await expect.poll(async () => activeNumbers(api)).not.toContain(21);
    await desk.click(demo.getByRole("button", { name: "Playback 24 button 1", exact: true }));
    await desk.click(demo.getByRole("button", { name: "Playback 3 button 1", exact: true }));
    await expect.poll(async () => activeNumbers(api)).toContain(3);
    await desk.click(presetWindow.locator(".group-strip .group-card").nth(1));
    await expect.poll(async () => (await programmer(api)).selected.length).toBe(rig.washTargets.length);
    await desk.click(presetWindow.getByRole("button", { name: /Blue Color ·/ }));
    await expect.poll(async () => blueStageColors(api, rig)).toBeGreaterThanOrEqual(7);

    await desk.titleCard("PRELOADING", "Physical playback changes and new moving-light positions are prepared blind, then committed together with a four-second programmer fade.");
    await clearProgrammer(desk, keypad, api);
    await desk.click(keypad.getByRole("button", { name: "PRELOAD GO", exact: true }));
    await desk.click(demo.getByRole("button", { name: "Playback 21 button 1", exact: true }));
    await desk.click(demo.getByRole("button", { name: "Playback 24 button 1", exact: true }));
    await openBuiltIn(desk, app, "Fixtures");
    const preloadFixtures = app.locator(".fixture-window");
    await desk.click(fixtureSheetRow(preloadFixtures, rig.fixtures[101].fixture_id));
    await desk.click(fixtureSheetRow(preloadFixtures, rig.fixtures[201].fixture_id));
    await expect.poll(async () => (await programmer(api)).selected.length).toBe(2);
    await desk.click(app.getByRole("button", { name: "Position", exact: true }));
    await setEncoderRange(desk, page, 1, "Pan", [20, 80]);
    await setEncoderValue(desk, page, "Prog. Fade", 4);
    await expect.poll(async () => (await programmer(api)).preload_pending.length).toBeGreaterThanOrEqual(2);
    const pending = await programmer(api);
    expect(pending.preload_playback_pending.length).toBeGreaterThanOrEqual(2);
    expect(pending.preload_pending.length).toBeGreaterThanOrEqual(2);
    await pause(page, 1_500);
    await desk.click(keypad.getByRole("button", { name: "PRELOAD GO", exact: true }));
    await bench.tick(6_000);
    await pause(page, 6_000);

    await desk.titleCard("ACL CHASER · SPEED A", "The final playback starts the alternating ACL fan; two learned taps set its Speed A tempo.");
    await desk.click(demo.getByRole("button", { name: "Playback 4 button 1", exact: true }));
    await page.keyboard.press("F9");
    await page.waitForTimeout(650);
    await page.keyboard.press("F9");
    await expect.poll(async () => activeNumbers(api)).toContain(4);
    await bench.tick(2_000);
    await expect.poll(async () => (await api.request<any>("GET", "/api/v1/dmx", undefined, false)).universes.some((frame: any) => frame.slots.some((value: number) => value > 0))).toBe(true);
    if (UPDATE_DEMO_SHOW) completedShow = await downloadCompletedDemoShow(api, showId);
    await pause(page, 2_000);

    if (RECORDING) {
      await fs.mkdir(path.dirname(SCREENSHOT), { recursive: true });
      await page.screenshot({ path: SCREENSHOT });
      await testInfo.attach("planned-demo-full-hd-screenshot", { path: SCREENSHOT, contentType: "image/png" });
    }
  } finally {
    if (video) {
      await fs.mkdir(path.dirname(VIDEO), { recursive: true });
      // Register the durable copy before page closure. The dedicated `npm run test:demo` result directory
      // prevents unrelated concurrent Playwright runs from deleting the source recording.
      const saveVideo = video.saveAs(VIDEO);
      await page.close();
      await saveVideo;
      await testInfo.attach("planned-demo-video", { path: VIDEO, contentType: "video/webm" });
    }
  }
  if (UPDATE_DEMO_SHOW) {
    expect(completedShow).not.toBeNull();
    await publishDemoShowAsset(completedShow!);
    await testInfo.attach("completed-demo-show", { path: DEMO_SHOW, contentType: "application/vnd.light.show" });
  }
});

async function downloadCompletedDemoShow(api: ApiDriver, showId: string): Promise<Buffer> {
  for (const route of await demoObjects<any>(api, showId, "route")) {
    const port = route.body.protocol === "sacn" ? 5568 : 6454;
    await api.request("PUT", `/api/v1/shows/${showId}/objects/route/${route.id}`, {
      ...route.body,
      destination: `127.0.0.1:${port}`,
    }, true, route.revision);
  }
  const response = await fetch(`${api.baseUrl}/api/v1/shows/${showId}/download`, {
    headers: { authorization: `Bearer ${api.session?.token}` },
  });
  expect(response.ok).toBe(true);
  return Buffer.from(await response.arrayBuffer());
}

async function publishDemoShowAsset(show: Buffer): Promise<void> {
  const temporary = `${DEMO_SHOW}.${process.pid}.${crypto.randomUUID()}.tmp`;
  await fs.mkdir(path.dirname(DEMO_SHOW), { recursive: true });
  await fs.writeFile(temporary, show);
  await fs.rename(temporary, DEMO_SHOW);
}

interface DemoFixturePlacement {
  search: string;
  family: string;
  mode: string;
  name: string;
  fixtureNumber: number | string;
  patch?: string;
  count?: number;
}

async function addFixtureViaUi(
  desk: DeskDriver,
  page: Page,
  patchWindow: Locator,
  layer: string,
  fixture: DemoFixturePlacement,
): Promise<void> {
  await activatePatchLayer(desk, patchWindow, layer);
  await desk.click(patchWindow.getByRole("button", { name: "+ Add fixture", exact: true }));
  const browser = page.locator(".fixture-browser-modal");
  await browser.getByRole("textbox", { name: "Search", exact: true }).fill(fixture.search);
  const familyButton = browser.locator(".fixture-picker-columns > section").nth(1).getByRole("button")
    .filter({ has: page.getByText(fixture.family, { exact: true }) }).first();
  await desk.click(familyButton);
  const modeSelect = browser.locator(".fixture-mode-detail select");
  const modeValue = await modeSelect.locator("option").evaluateAll((options, prefix) =>
    options.find((option) => option.textContent?.startsWith(prefix as string))?.getAttribute("value") ?? null, fixture.mode);
  if (!modeValue) throw new Error(`Demo fixture mode ${fixture.mode} was not available for ${fixture.family}`);
  await modeSelect.selectOption(modeValue);
  await desk.click(browser.locator(".fixture-mode-detail").getByRole("button", { name: "Add fixture", exact: true }));
  const placement = page.locator(".fixture-placement-modal");
  await placement.getByRole("textbox", { name: /^Fixture name\b/ }).fill(fixture.name);
  await placement.getByRole("textbox", { name: "Start fixture ID", exact: true }).fill(String(fixture.fixtureNumber));
  await placement.getByRole("textbox", { name: "Count", exact: true }).fill(String(fixture.count ?? 1));
  if (fixture.patch) await placement.getByRole("textbox", { name: /^Address \(universe\.address\)/ }).fill(fixture.patch);
  const add = placement.getByRole("button", { name: `Add ${fixture.count ?? 1} fixtures`, exact: true });
  for (let attempt = 0; attempt < 3 && await placement.isVisible(); attempt++) {
    await desk.click(add);
    await placement.waitFor({ state: "hidden", timeout: 5_000 }).catch(() => undefined);
  }
  await expect(fixtureRow(patchWindow, fixture.fixtureNumber)).toBeVisible();
}

async function positionFixtureViaUi(
  desk: DeskDriver,
  page: Page,
  keypad: Locator,
  row: Locator,
  location: { x: number; y: number; z: number },
  rotation?: { x: number; y: number; z: number },
): Promise<void> {
  await editFixtureVector(desk, page, keypad, row, 9, "Set fixture location", location, true);
  if (rotation) await editFixtureVector(desk, page, keypad, row, 10, "Set fixture rotation", rotation, false);
}

async function editFixtureVector(
  desk: DeskDriver,
  page: Page,
  keypad: Locator,
  row: Locator,
  cell: number,
  heading: string,
  value: { x: number; y: number; z: number },
  metres: boolean,
): Promise<void> {
  const setButton = keypad.getByRole("button", { name: "SET", exact: true });
  await expect(setButton).not.toHaveClass(/patch-set-armed/);
  await desk.click(setButton);
  await expect(setButton).toHaveClass(/patch-set-armed/);
  await desk.click(row.locator("td").nth(cell).getByRole("button"));
  const modal = page.locator(".patch-edit-modal", { hasText: heading });
  for (const axis of ["X", "Y", "Z"] as const)
    await modal.getByLabel(`${axis}${metres ? " (m)" : ""}`, { exact: true }).fill(String(value[axis.toLowerCase() as "x" | "y" | "z"]));
  await desk.click(modal.getByRole("button", { name: "Set", exact: true }));
}

async function positionMultipatchViaUi(
  desk: DeskDriver,
  page: Page,
  row: Locator,
  location: { x: number; y: number; z: number },
): Promise<void> {
  await desk.click(row.locator("td").nth(9).getByRole("button"));
  const modal = page.locator(".patch-edit-modal", { hasText: "Set multi-patch location" });
  for (const axis of ["X", "Y", "Z"] as const)
    await modal.getByLabel(`${axis} (m)`, { exact: true }).fill(String(location[axis.toLowerCase() as "x" | "y" | "z"]));
  await desk.click(modal.getByRole("button", { name: "Set", exact: true }));
}

function fixtureRow(patchWindow: Locator, fixtureNumber: number | string): Locator {
  return patchWindow.getByRole("cell", { name: String(fixtureNumber), exact: true }).locator("..").first();
}

function escapeRegex(value: string): string { return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"); }

async function pause(page: Page, millis: number): Promise<void> {
  if (RECORDING) await page.waitForTimeout(millis);
}

async function fastForwardPatchPhase(
  desk: DeskDriver,
  page: Page,
  api: ApiDriver,
  showId: string,
  patchWindow: Locator,
  layerIds: Record<string, string>,
  phase: PlannedDemoPatchPhase,
  layerNames: readonly string[],
  description: string,
  afterPatch?: (rig: Awaited<ReturnType<typeof seedPlannedDemoPatch>>) => Promise<void>,
) {
  return desk.fastForward(description, async () => {
    let rig: Awaited<ReturnType<typeof seedPlannedDemoPatch>> | null = null;
    for (const layerName of layerNames) {
      await activatePatchLayer(desk, patchWindow, layerName);
      const layerId = layerIds[layerName];
      if (!layerId) throw new Error(`Patch layer ${layerName} does not exist`);
      const rows = patchWindow.locator(".patch-table tbody tr");
      const beforeRows = await rows.count();
      rig = await seedPlannedDemoPatch(api, showId, layerIds, [phase], [layerId]);
      await expect.poll(() => rows.count()).toBeGreaterThan(beforeRows);
      await pause(page, 80);
    }
    if (!rig) throw new Error(`Patch phase ${phase} has no destination layers`);
    if (afterPatch) await afterPatch(rig);
    return rig;
  });
}

async function activatePatchLayer(desk: DeskDriver, patchWindow: Locator, layer: string): Promise<void> {
  const layerButton = patchWindow.locator(".patch-layers").getByRole("button")
    .filter({ hasText: new RegExp(`^${escapeRegex(layer)}\\s*\\d+$`) });
  if (!await layerButton.evaluate((button) => button.classList.contains("active"))) await desk.click(layerButton);
  await expect(layerButton).toHaveClass(/active/);
}

async function openBuiltIn(desk: DeskDriver, app: Locator, name: string): Promise<void> {
  await desk.click(app.getByRole("button", { name: "BUILT-INS", exact: true }));
  await desk.click(app.locator(".dock-entry").filter({ hasText: name }).first());
}

async function createOutputRoute(
  desk: DeskDriver,
  page: Page,
  routes: Locator,
  protocol: "Art-Net" | "sACN",
  logicalUniverse: number,
  destinationUniverse: number,
  port: number,
): Promise<void> {
  await desk.click(routes.getByRole("button", { name: "Add route", exact: true }));
  const editor = page.getByRole("dialog", { name: "Output route editor" });
  if (protocol === "sACN") {
    await desk.click(editor.getByRole("button", { name: "Art-Net", exact: true }));
    await desk.click(page.getByRole("option", { name: "sACN", exact: true }));
  }
  await desk.click(editor.getByRole("button", { name: protocol === "Art-Net" ? "Broadcast" : "Multicast", exact: true }));
  await desk.click(page.getByRole("option", { name: "Unicast", exact: true }));
  await editor.getByLabel("Logical universe").fill(String(logicalUniverse));
  await editor.getByLabel("Destination universe").fill(String(destinationUniverse));
  await editor.getByLabel("Destination", { exact: true }).fill(`127.0.0.1:${port}`);
  await editor.getByLabel("Minimum universe size").fill("128");
  await desk.click(editor.getByRole("button", { name: "Save route", exact: true }));
}

function fixtureSheetRow(fixtureWindow: Locator, fixtureId: string): Locator {
  return fixtureWindow.locator(`[data-fixture-id="${fixtureId}"]`).first();
}

async function chooseRecordMode(desk: DeskDriver, page: Page, mode: "Overwrite" | "Merge"): Promise<void> {
  const dialog = page.locator(".record-mode-dialog");
  await expect(dialog).toBeVisible();
  const dialogBox = await dialog.boundingBox();
  const viewport = page.viewportSize();
  expect(dialogBox).not.toBeNull();
  expect(viewport).not.toBeNull();
  expect(dialogBox!.x + dialogBox!.width / 2).toBeCloseTo(viewport!.width / 2, 0);
  expect(dialogBox!.y + dialogBox!.height / 2).toBeCloseTo(viewport!.height / 2, 0);
  await desk.click(dialog.getByRole("button", { name: mode, exact: true }));
  await expect(dialog).toBeHidden();
}

async function setPresetButtonTitle(desk: DeskDriver, page: Page, keypad: Locator, presetWindow: Locator, index: number, title: string): Promise<void> {
  await desk.click(keypad.getByRole("button", { name: "ESCAPE", exact: true }));
  await desk.click(keypad.getByRole("button", { name: "SET", exact: true }));
  await desk.click(presetWindow.locator(".preset-card").nth(index));
  const dialog = page.getByRole("dialog", { name: "Configure preset button" });
  await dialog.getByLabel("Title").fill(title);
  await desk.click(dialog.getByRole("button", { name: "Save button", exact: true }));
}

async function setEncoderValue(desk: DeskDriver, page: Page, label: string, value: number): Promise<void> {
  if (label === "Prog. Fade") {
    await desk.click(page.locator(".hardware-values").getByRole("button", { name: /Prog Fade/ }));
    const dialog = page.getByRole("dialog").filter({ has: page.getByRole("heading", { name: "Prog. Fade", exact: true }) });
    await expect(dialog).toBeVisible();
    await page.keyboard.type(String(value));
    await page.keyboard.press("Enter");
    await expect(dialog).toBeHidden();
    return;
  }
  const control = page.locator(".vertical-touch-fader-stack, .programmer-fade-fader").filter({ hasText: label }).first();
  const slot = Number(label.match(/^Enc (\d+)/)?.[1] ?? 0);
  let dialogName = `${label} value`;
  if (await control.isVisible()) {
    const directButton = control.getByRole("button", { name: "Set value", exact: true });
    await desk.click(await directButton.isVisible() ? directButton : control.getByRole("button").first());
  } else {
    const attribute = label.replace(/^Enc \d+ · /, "");
    const hardwareEncoder = page.getByRole("button", { name: new RegExp(`^Encoder ${slot}: ${attribute},`, "i") }).first();
    await expect(hardwareEncoder).toBeVisible();
    await desk.click(hardwareEncoder);
    dialogName = `Encoder ${slot} value`;
  }
  const dialog = page.getByRole("dialog", { name: dialogName });
  await expect(dialog).toBeVisible();
  await page.keyboard.type(String(value));
  await page.keyboard.press("Enter");
  await expect(dialog).toBeHidden();
}

async function setEncoderRange(desk: DeskDriver, page: Page, slot: number, attribute: string, values: [number, number]): Promise<void> {
  const encoder = page.getByRole("button", { name: new RegExp(`^Encoder ${slot}: ${attribute},`, "i") }).first();
  await expect(encoder).toBeVisible();
  await desk.click(encoder);
  const dialog = page.getByRole("dialog", { name: `Encoder ${slot} value` });
  await expect(dialog).toBeVisible();
  for (const key of [...String(values[0]), "THRU", ...String(values[1]), "ENTER"])
    await desk.click(dialog.getByRole("button", { name: key, exact: true }));
  await expect(dialog).toBeHidden();
}

async function keypadCommand(desk: DeskDriver, keypad: Locator, labels: string[], escape = true): Promise<void> {
  if (escape) await desk.click(keypad.getByRole("button", { name: "ESCAPE", exact: true }));
  for (const label of labels) {
    await desk.click(keypad.getByRole("button", { name: label, exact: true }));
    if (!RECORDING) await keypad.page().waitForTimeout(20);
  }
}

async function clearSelection(desk: DeskDriver, keypad: Locator, api: ApiDriver): Promise<void> {
  // The first Clear can dismiss a pending command before subsequent presses walk selection back.
  for (let attempt = 0; attempt < 12 && (await programmer(api)).selected.length; attempt++)
    await desk.click(keypad.getByRole("button", { name: "CLR", exact: true }));
  await expect.poll(async () => (await programmer(api)).selected).toEqual([]);
}

async function clearProgrammer(desk: DeskDriver, keypad: Locator, api: ApiDriver): Promise<void> {
  for (let attempt = 0; attempt < 4; attempt++) {
    const state = await programmer(api);
    if (!state.selected.length && !state.values.length && !Object.keys(state.group_values).length) return;
    await desk.click(keypad.getByRole("button", { name: "CLR", exact: true }));
  }
  await expect.poll(async () => {
    const state = await programmer(api);
    return state.selected.length + state.values.length + Object.keys(state.group_values).length;
  }).toBe(0);
}

async function blueStageColors(api: ApiDriver, rig: Awaited<ReturnType<typeof seedPlannedDemoPatch>>): Promise<number> {
  const ownerIds = new Set(
    [...Array.from({ length: 8 }, (_, index) => 101 + index), ...Array.from({ length: 7 }, (_, index) => 201 + index)]
      .flatMap((number) => [rig.fixtures[number].fixture_id, ...rig.fixtures[number].logical_heads.map((head) => head.fixture_id)]),
  );
  const visualization = await api.request<any>("GET", "/api/v1/visualization");
  return (visualization.profile_output_values ?? []).filter((entry: any) => {
    if (!ownerIds.has(entry.fixture_id) || entry.attribute !== "color" || entry.value?.kind !== "color_xyz") return false;
    const { x, y, z } = entry.value.value;
    return z > x * 1.5 && z > y * 1.5;
  }).length;
}

async function activeNumbers(api: ApiDriver): Promise<number[]> {
  return (await api.request<any>("GET", "/api/v1/playbacks")).active
    .filter((item: any) => item.enabled)
    .map((item: any) => item.playback_number);
}
