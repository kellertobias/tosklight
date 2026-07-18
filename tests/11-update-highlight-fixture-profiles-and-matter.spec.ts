import { execFile } from "node:child_process";
import fs from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { ApiDriver } from "../apps/control-ui/e2e/bench/api";
import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import type { Locator, Page } from "../apps/control-ui/node_modules/@playwright/test/index.js";
import { pairedScenario } from "../apps/control-ui/e2e/bench/pairedScenario";
import { blankFixtureProfile, fixtureDefinitionFromProfileMode } from "../apps/control-ui/src/components/setup/fixtureProfileModel";
import {
  loadCanonicalCopy,
  object,
  objects,
  pressCommand,
  programmer,
  putObject,
} from "./support/catalog";

interface UpdateGroupState {
  groupId: string;
  groupName: string;
  revision: number;
  original: string[];
  added: string;
}

interface HighlightFixture {
  id: string;
  number: number;
}

interface HighlightScenarioState {
  showId: string;
  fixtures: HighlightFixture[];
  storedPresetId: string;
  selectionStayedComplete?: boolean;
}

interface HighlightSurfaceState {
  showId: string;
  fixtures: HighlightFixture[];
  liveGroup: {
    id: string;
    name: string;
    initial: string[];
    updated: string[];
  };
  steppedSelection?: string[];
  restoredSelection?: string[];
  highSurvivedEmpty?: boolean;
  highFollowedSelection?: boolean;
  reconnectRetained?: boolean;
}

interface HighlightSequenceState {
  showId: string;
  fixtures: HighlightFixture[];
  expectedSequence: string[][];
  observedSequence: string[][];
  singletonGroupId: string;
  completeGroupId: string;
  highStayedOff?: boolean;
  wrappedForward?: boolean;
  wrappedBackward?: boolean;
  highSurvivedEmpty?: boolean;
  highFollowedSelection?: boolean;
  removedCaptureRejected?: boolean;
  altCaptureWasNoOp?: boolean;
  geometryVerified?: boolean;
  fixtureSheetVerified?: boolean;
  noCommandBarPanel?: boolean;
}

interface FixtureProfileState {
  manufacturer: string;
  name: string;
}

interface MatterScenarioState {
  observed: any | null;
  page: number;
  slot: number;
  emptySlot: number;
  playbackNumber: number;
}

const sqlite = promisify(execFile);

test("HIGHLIGHT-004 @api › ownership conflicts retain same-user sessions, release on the last session, and stay desk-local", async ({ api, bench }) => {
  await loadCanonicalCopy(api, bench, "highlight-004", "default-stage");
  const fixtures = await fixturesByNumber(api, [101, 102, 103]);
  await api.request("POST", "/api/v1/users", { name: "Highlight A", enabled: true });
  await api.request("POST", "/api/v1/users", { name: "Highlight B", enabled: true });

  const userAFirst = new ApiDriver(api.baseUrl);
  userAFirst.session = await userAFirst.request(
    "POST",
    "/api/v1/sessions",
    { username: "Highlight A", desk_id: api.session!.desk.id },
    false,
  );
  const userASecond = new ApiDriver(api.baseUrl);
  userASecond.session = await userASecond.request(
    "POST",
    "/api/v1/sessions",
    { username: "Highlight A", desk_id: api.session!.desk.id },
    false,
  );
  const userB = new ApiDriver(api.baseUrl);
  userB.session = await userB.request(
    "POST",
    "/api/v1/sessions",
    { username: "Highlight B", desk_id: api.session!.desk.id },
    false,
  );

  await userAFirst.command("selection.set", { fixtures: [fixtures[0].id] });
  await userB.command("selection.set", { fixtures: [fixtures[1].id] });
  await highlightAction(userAFirst, "on");
  const ownerBeforeConflict = await highlightState(userAFirst);
  expect(ownerBeforeConflict).toMatchObject({
    active: true,
    output_enabled: true,
    owner_user_name: "Highlight A",
  });
  expect(ownerBeforeConflict.remembered).toHaveLength(1);
  await expect(highlightAction(userB, "toggle")).rejects.toThrow(/another user on this desk/i);
  expect(await highlightState(userAFirst)).toMatchObject(ownerBeforeConflict);
  expect(await highlightState(userB)).toMatchObject({
    active: false,
    output_enabled: false,
  });

  await highlightAction(userB, "next");
  expect((await programmer(userB)).selected).toEqual([fixtures[1].id]);
  expect(await highlightState(userAFirst)).toMatchObject({ active: true, output_enabled: true, owner_user_name: "Highlight A" });
  await userASecond.request("DELETE", `/api/v1/sessions/${userASecond.session!.session_id}`);
  expect((await highlightState(userAFirst)).active).toBe(true);
  await expect(highlightAction(userB, "on")).rejects.toThrow(/another user on this desk/i);

  await userAFirst.request("DELETE", `/api/v1/sessions/${userAFirst.session!.session_id}`);
  await highlightAction(userB, "on");
  expect(await highlightState(userB)).toMatchObject({ active: true, output_enabled: true, owner_user_name: "Highlight B" });

  const otherDesk = new ApiDriver(api.baseUrl);
  await otherDesk.login("Highlight A");
  await otherDesk.command("selection.set", { fixtures: [fixtures[2].id] });
  await highlightAction(otherDesk, "on");
  expect(await highlightState(otherDesk)).toMatchObject({
    active: true,
    output_enabled: true,
    owner_user_name: "Highlight A",
    remembered: [{ fixture_id: fixtures[2].id }],
  });
  expect((await highlightState(userB)).owner_user_name).toBe("Highlight B");
});

test("HIGHLIGHT-005 @ui › Highlight errors remain reachable above production content without moving accepted controls", async ({ api, bench, desk, page }) => {
  await loadCanonicalCopy(api, bench, "highlight-005", "default-stage");
  const errors = [
    { status: 409, message: "Highlight output is active for another user on this desk" },
    { status: 500, message: "The Highlight action was rejected by the desk" },
  ];

  for (const viewport of [{ width: 1280, height: 720 }, { width: 1600, height: 1100 }]) {
    await page.setViewportSize(viewport);
    await desk.open(bench.baseUrl);
    await openBuiltIn(page, "Fixtures");
    await expect(page.locator(".programmer-number-block")).toBeVisible();

    for (const error of errors) {
      const before = await softwareHighlightGeometry(page);
      await page.route("**/api/v1/highlight/action", async (route) => {
        await route.fulfill({
          status: error.status,
          contentType: "application/json",
          body: JSON.stringify({ error: error.message }),
        });
      }, { times: 1 });
      await highlightKey(page, "HIGH").click();
      const alert = page.locator("[data-highlight-error-alert]");
      await expect(alert).toHaveCount(1);
      await expect(alert).toContainText(error.message);
      await page.getByRole("button", { name: /Open show menu/ }).click();
      const modal = page.getByRole("dialog", { name: "Show", exact: true });
      await expect(modal).toBeVisible();
      await assertReachableAlert(page, alert, modal, viewport);
      expect(await softwareHighlightGeometry(page)).toEqual(before);
      await expect(highlightKey(page, "HIGH")).toHaveText("HIGH");
      await expect(page.locator(".command-line-bar [aria-label='Highlight status']")).toHaveCount(0);
      const dismiss = page.getByRole("button", { name: "Dismiss Highlight error" });
      await dismiss.focus();
      await expect(dismiss).toBeFocused();
      await dismiss.press("Enter");
      await expect(alert).toBeHidden();
      await page.getByRole("button", { name: "Close Show" }).click();
      await expect(modal).toBeHidden();
    }

    const hardware = await bench.osc();
    const clientId = `highlight-005-${viewport.width}-${crypto.randomUUID()}`;
    try {
      await page.route("**/api/v1/highlight/action", async (route) => {
        await route.fulfill({
          status: 409,
          contentType: "application/json",
          body: JSON.stringify({ error: errors[0].message }),
        });
      }, { times: 1 });
      await highlightKey(page, "HIGH").click();
      const alert = page.locator("[data-highlight-error-alert]");
      await expect(alert).toBeVisible();
      await hardware.subscribe(clientId, api.session!.desk.osc_alias);
      await expect.poll(async () => (await api.request<any>("GET", "/api/v1/bootstrap", undefined, false)).hardware_connected).toBe(true);
      await expect(page.locator(".hardware-right-pane .hardware-control-summary")).toBeVisible();
      const hardwareBefore = await hardwareHighlightGeometry(page);
      await page.getByRole("button", { name: /Open show menu/ }).click();
      const modal = page.getByRole("dialog", { name: "Show", exact: true });
      await assertReachableAlert(page, alert, modal, viewport);
      expect(await hardwareHighlightGeometry(page)).toEqual(hardwareBefore);
      await page.getByRole("button", { name: "Dismiss Highlight error" }).click();
      await page.getByRole("button", { name: "Close Show" }).click();
    } finally {
      await hardware.send("/light/unsubscribe", [clientId]).catch(() => undefined);
      await hardware.close();
      await expect.poll(async () => (await api.request<any>("GET", "/api/v1/bootstrap", undefined, false)).hardware_connected).toBe(false);
    }
  }
});

test("FIXTURE-002 @restart › focused assets and physical metadata remain immutable across edit, patch, and restart", async ({ api, bench, desk, page }) => {
  test.setTimeout(90_000);
  await loadCanonicalCopy(api, bench, "fixture-002", "default-stage");
  const manufacturer = `Feature 21 ${crypto.randomUUID()}`;
  const name = "Complete Asset Fixture";
  const physical = {
    width_millimetres: 420,
    height_millimetres: 680,
    depth_millimetres: 310,
    weight_kilograms: 24.5,
    power_watts: 720,
    color_temperature_kelvin: 6500,
    luminous_output_lumens: 18500,
    beam_angle_degrees: 36,
  };
  const files = {
    photoA: "fixture-002-photo-a.png",
    photoB: "fixture-002-photo-b.png",
    icon: "fixture-002-icon.png",
    modelA: "fixture-002-model-a.glb",
    modelB: "fixture-002-model-b.glb",
  };
  await extractFixtureAsset("generic--dimmer-profile.toskfixture", "assets/icon.png", `${bench.dataDir}/shows/${files.photoA}`);
  await extractFixtureAsset("generic--dimmer-par-can.toskfixture", "assets/icon.png", `${bench.dataDir}/shows/${files.photoB}`);
  await fs.copyFile(`${bench.dataDir}/shows/${files.photoB}`, `${bench.dataDir}/shows/${files.icon}`);
  await extractFixtureAsset("generic--dimmer-profile.toskfixture", "assets/model.glb", `${bench.dataDir}/shows/${files.modelA}`);
  await extractFixtureAsset("generic--dimmer-par-can.toskfixture", "assets/model.glb", `${bench.dataDir}/shows/${files.modelB}`);
  const expectedAssets = {
    photoA: `data:image/png;base64,${(await fs.readFile(`${bench.dataDir}/shows/${files.photoA}`)).toString("base64")}`,
    photoB: `data:image/png;base64,${(await fs.readFile(`${bench.dataDir}/shows/${files.photoB}`)).toString("base64")}`,
    icon: `data:image/png;base64,${(await fs.readFile(`${bench.dataDir}/shows/${files.icon}`)).toString("base64")}`,
    modelA: `data:application/octet-stream;base64,${(await fs.readFile(`${bench.dataDir}/shows/${files.modelA}`)).toString("base64")}`,
    modelB: `data:application/octet-stream;base64,${(await fs.readFile(`${bench.dataDir}/shows/${files.modelB}`)).toString("base64")}`,
  };

  await desk.open(bench.baseUrl);
  await page.getByRole("button", { name: /Open show menu/ }).click();
  await page.getByRole("button", { name: "Enter Setup", exact: true }).click();
  await page.getByRole("button", { name: "Open Fixture Library", exact: true }).click();
  await page.getByRole("button", { name: "Create fixture", exact: true }).click();
  let editor = page.getByRole("dialog", { name: "Create fixture profile" });
  await editor.getByLabel(/^Manufacturer/).fill(manufacturer);
  await editor.getByLabel(/^Fixture name/).fill(name);
  await editor.getByLabel("Fixture short name").fill("Asset Fixture");
  await chooseCustomSelect(editor, "Fixture type", "wash mover");
  await editor.getByLabel("Fixture notes").fill("Complete Generic asset and physical metadata acceptance fixture.");
  for (const [label, value] of [
    ["Width (mm)", physical.width_millimetres], ["Height (mm)", physical.height_millimetres],
    ["Depth (mm)", physical.depth_millimetres], ["Weight (kg)", physical.weight_kilograms],
    ["Power consumption (W)", physical.power_watts], ["Color temperature (K)", physical.color_temperature_kelvin],
    ["Luminous output (lm)", physical.luminous_output_lumens],
    ["Beam angle (degrees)", physical.beam_angle_degrees],
  ] as const) await editor.getByLabel(label).fill(String(value));
  await expect(editor.getByLabel("Connectors")).toHaveCount(0);
  await expect(editor.getByLabel("Light source")).toHaveCount(0);
  await expect(editor.getByLabel("Color rendering index (CRI)")).toHaveCount(0);
  await expect(editor.getByLabel("Lens")).toHaveCount(0);
  const assetColumns = editor.locator(".fixture-notes-assets > div");
  await expect(assetColumns).toHaveCount(3);
  await expect(assetColumns.nth(0).getByRole("heading", { name: "Notes" })).toBeVisible();
  await expect(assetColumns.nth(1).getByRole("heading", { name: "Fixture photograph" })).toBeVisible();
  await expect(assetColumns.nth(2).getByRole("heading", { name: "Visualizer" })).toBeVisible();
  const assetColumnBoxes = await Promise.all([0, 1, 2].map((index) => assetColumns.nth(index).boundingBox()));
  expect(assetColumnBoxes.every(Boolean)).toBe(true);
  expect(Math.max(...assetColumnBoxes.map((box) => box!.width)) - Math.min(...assetColumnBoxes.map((box) => box!.width))).toBeLessThan(2);
  expect(assetColumnBoxes[0]!.x).toBeLessThan(assetColumnBoxes[1]!.x);
  expect(assetColumnBoxes[1]!.x).toBeLessThan(assetColumnBoxes[2]!.x);

  await editor.getByRole("button", { name: "Choose photograph", exact: true }).click();
  await selectConfinedFile(page, files.photoA);
  await expect(editor.getByAltText("Fixture photograph preview")).toHaveAttribute("src", expectedAssets.photoA);
  await editor.getByRole("button", { name: "Replace photograph", exact: true }).click();
  await selectConfinedFile(page, files.photoB);
  await expect(editor.getByAltText("Fixture photograph preview")).toHaveAttribute("src", expectedAssets.photoB);
  expect(await editor.getByAltText("Fixture photograph preview").getAttribute("src")).not.toBe(expectedAssets.photoA);
  await editor.getByRole("button", { name: "Remove photograph", exact: true }).click();
  await expect(editor.getByAltText("Fixture photograph preview")).toHaveCount(0);
  await editor.getByRole("button", { name: "Choose fixture icon", exact: true }).click();
  await selectConfinedFile(page, files.icon);
  await editor.getByRole("button", { name: "Choose visualizer glb model", exact: true }).click();
  await selectConfinedFile(page, files.modelA);
  await expect(editor.getByRole("status")).toContainText("GLB 2.0 · 1268 bytes");
  const preview = editor.getByLabel("Visualizer GLB model preview");
  const previewCanvas = preview.locator("canvas");
  await expect(preview).toHaveAttribute("title", "Drag to rotate; scroll to zoom");
  await expect(previewCanvas).toBeVisible();
  const beforeOrbit = await previewCanvas.screenshot();
  const previewBox = await previewCanvas.boundingBox();
  expect(previewBox).not.toBeNull();
  await page.mouse.move(previewBox!.x + previewBox!.width / 2, previewBox!.y + previewBox!.height / 2);
  await page.mouse.down();
  await page.mouse.move(previewBox!.x + previewBox!.width * .8, previewBox!.y + previewBox!.height * .6, { steps: 5 });
  await page.mouse.up();
  const afterOrbit = await previewCanvas.screenshot();
  expect(afterOrbit.equals(beforeOrbit)).toBe(false);
  await editor.getByRole("button", { name: "Replace visualizer glb model", exact: true }).click();
  await selectConfinedFile(page, files.modelB);
  await expect(editor.getByRole("status")).toContainText("GLB 2.0 · 1448 bytes");
  await editor.getByRole("button", { name: "Save fixture", exact: true }).click();
  await expect(editor).toBeHidden();

  const profile = (await api.request<any[]>("GET", "/api/v1/fixture-profiles", undefined, false)).find((candidate) => candidate.manufacturer === manufacturer && candidate.name === name);
  expect(profile).toBeDefined();
  expect(profile).toMatchObject({ revision: 1, photograph_asset: null, stage_icon_asset: expectedAssets.icon, model_asset: expectedAssets.modelB, physical });

  await page.getByPlaceholder("Search manufacturer, fixture, mode, or type").fill(manufacturer);
  await page.getByRole("button", { name: new RegExp(name) }).click();
  await page.getByRole("button", { name: "Edit fixture", exact: true }).click();
  editor = page.getByRole("dialog", { name: "Edit fixture profile" });
  await expect(editor.getByLabel("Width (mm)")).toHaveValue("420");
  await expect(editor.getByLabel("Color temperature (K)")).toHaveValue("6500");
  await expect(editor.getByLabel("Luminous output (lm)")).toHaveValue("18500");
  await expect(editor.getByLabel("Beam angle (degrees)")).toHaveValue("36");
  await expect(editor.getByAltText("Fixture photograph preview")).toHaveCount(0);
  await expect(editor.getByText("Fixture icon assigned")).toBeVisible();
  await expect(editor.getByText("Visualizer GLB model assigned")).toBeVisible();
  await expect(editor.getByRole("status")).toContainText("GLB 2.0 · 1448 bytes");

  await editor.getByLabel("Beam angle (degrees)").fill("42");
  await editor.getByRole("button", { name: "Choose photograph", exact: true }).click();
  await selectConfinedFile(page, files.photoA);
  await editor.getByRole("button", { name: "Replace visualizer glb model", exact: true }).click();
  await selectConfinedFile(page, files.modelA);
  await expect(editor.getByRole("status")).toContainText("GLB 2.0 · 1268 bytes");
  await editor.getByRole("button", { name: "Save fixture", exact: true }).click();
  await page.getByRole("alertdialog", { name: "Create a new fixture revision?" }).getByRole("button", { name: "Save and create revision" }).click();
  await expect(editor).toBeHidden();

  const revisions = await api.request<any[]>("GET", `/api/v1/fixture-profiles/${profile.id}/revisions`, undefined, false);
  expect(revisions.map((candidate) => candidate.revision)).toEqual([1, 2]);
  expect(revisions[0]).toMatchObject({
    photograph_asset: null,
    stage_icon_asset: expectedAssets.icon,
    model_asset: expectedAssets.modelB,
    physical: { beam_angle_degrees: 36 },
  });
  expect(revisions[1]).toMatchObject({
    photograph_asset: expectedAssets.photoA,
    stage_icon_asset: expectedAssets.icon,
    model_asset: expectedAssets.modelA,
    physical: { ...physical, beam_angle_degrees: 42 },
  });

  const definition = fixtureDefinitionFromProfileMode(revisions[1], revisions[1].modes[0]);
  const fixture = (await objects<any>(api, "patched_fixture"))[0];
  await putObject(api, "patched_fixture", fixture.id, {
    ...fixture.body,
    definition,
    split_patches: [{ split: 1, universe: fixture.body.universe, address: fixture.body.address }],
  }, fixture.revision);
  const patched = await object<any>(api, "patched_fixture", fixture.id);
  expect(patched.body.definition.profile_snapshot).toMatchObject(revisions[1]);

  await bench.stopServerGracefully(api.session!.token);
  await bench.startServer();
  await api.login();
  const reopened = await object<any>(api, "patched_fixture", fixture.id);
  expect(reopened.body.definition.profile_snapshot).toMatchObject(revisions[1]);
  expect(reopened.body.definition.profile_snapshot.physical.beam_angle_degrees).toBe(42);
});

pairedScenario<UpdateGroupState>({
  id: "UPDATE-001",
  title: "Update Add New appends ordered Group membership through the authoritative workflow",
  arrange: async ({ api, bench }, surface) => {
    await loadCanonicalCopy(api, bench, `update-001-${surface}`);
    const groups = await objects<any>(api, "group");
    const fixtures = (await objects<any>(api, "patched_fixture")).map((entry) => entry.body.fixture_id as string);
    const group = groups.find((entry) => !entry.body.derived_from && !entry.body.frozen_from && entry.body.fixtures.length > 0);
    expect(group).toBeDefined();
    const added = fixtures.find((fixture) => !group!.body.fixtures.includes(fixture));
    expect(added).toBeDefined();
    await api.command("selection.set", { fixtures: [added] });
    return {
      groupId: group!.id,
      groupName: group!.body.name || `Group ${group!.id}`,
      revision: group!.revision,
      original: [...group!.body.fixtures],
      added: added!,
    };
  },
  api: async ({ api }, state) => {
    await api.request("POST", "/api/v1/update/apply", {
      target: { family: { type: "group" }, object_id: state.groupId },
      mode: { target_type: "existing_content", mode: "add_new" },
      expected_revision: state.revision,
    });
  },
  ui: async ({ api, bench, desk, page }, state) => {
    await desk.open(bench.baseUrl);
    await expect.poll(async () => (await programmer(api)).selected).toEqual([state.added]);
    await page.keyboard.press("Shift+End");
    await expect(page.getByText(/UPDATE armed · touch a recordable target/i)).toBeVisible();
    await openGroups(page);
    const target = page.locator(".group-pool-window .group-card").filter({ hasText: state.groupName }).first();
    await expect(target).toBeVisible();
    await target.click();
    const dialog = page.getByRole("dialog", { name: new RegExp(`Update ${escapeRegex(state.groupName)}`, "i") });
    await expect(dialog).toBeVisible();
    await dialog.getByRole("button", { name: "Add New", exact: true }).click();
    await expect(dialog.getByText(/Changed 1/)).toBeVisible();
    await dialog.getByRole("button", { name: "Update Group", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Update complete" })).toBeVisible();
  },
  assert: async ({ api }, state) => {
    const stored = await object<any>(api, "group", state.groupId);
    expect(stored.revision).toBe(state.revision + 1);
    expect(stored.body.fixtures).toEqual([...state.original, state.added]);
    expect((await programmer(api)).selected).toEqual([state.added]);
  },
});

test("UPDATE-002 @restart › pre-Update desk settings migrate once and Cue, Preset, and ordered Group updates remain undoable", async ({ api, bench, desk, page }) => {
  test.setTimeout(90_000);
  const show = await loadCanonicalCopy(api, bench, "update-002-legacy");
  const showEntry = (await api.request<any[]>("GET", "/api/v1/shows", undefined, false)).find((entry) => entry.id === show.id);
  expect(showEntry).toBeDefined();
  const fixtures = (await objects<any>(api, "patched_fixture")).slice(0, 4);
  expect(fixtures).toHaveLength(4);
  const [first, second, third, fourth] = fixtures.map((fixture) => fixture.body.fixture_id as string);

  const cueListId = crypto.randomUUID();
  const cueId = crypto.randomUUID();
  const cueBaseline = {
    id: cueListId,
    name: "Legacy Update Cue",
    priority: 0,
    mode: "sequence",
    looped: false,
    chaser_step_millis: 1_000,
    speed_group: null,
    intensity_priority_mode: "htp",
    wrap_mode: "off",
    restart_mode: "first_cue",
    force_cue_timing: false,
    disable_cue_timing: false,
    chaser_xfade_millis: 0,
    speed_multiplier: 1,
    cues: [{
      id: cueId,
      number: 1,
      name: "Legacy cue",
      changes: [{ fixture_id: first, attribute: "intensity", value: { kind: "normalized", value: 0.2 }, automatic_restore: false }],
      group_changes: [],
      fade_millis: 0,
      delay_millis: 0,
      trigger: { type: "manual" },
      phasers: [],
    }],
  };
  const presetId = "0.1";
  const presetBaseline = {
    name: "Legacy Update Preset",
    family: "Mixed",
    values: { [first]: { intensity: { kind: "normalized", value: 0.1 } } },
    group_values: {},
  };
  const groupId = "39";
  const groupBaseline = groupBody("Legacy ordered Group", [first, second]);
  await putObject(api, "cue_list", cueListId, cueBaseline);
  await putObject(api, "preset", presetId, presetBaseline);
  await putObject(api, "group", groupId, groupBaseline);

  const configuration = await api.request<any>("GET", "/api/v1/configuration", undefined, false);
  await api.request("PUT", "/api/v1/configuration", configuration.configuration);
  await bench.stopServerGracefully(api.session!.token);
  await runSql(`${bench.dataDir}/desk.sqlite`, "UPDATE settings SET value=json_remove(value,'$.update_settings_by_desk') WHERE key='server_configuration'; UPDATE schema_info SET version=6;");
  expect(await readSql(showEntry.path, "SELECT version FROM schema_info")).toBe("4");
  expect(await readSql(showEntry.path, "SELECT count(*) FROM metadata WHERE key LIKE 'update_%'")).toBe("0");

  await bench.startServer();
  await api.login();
  const migratedDefaults = {
    cue_mode: "add_to_current_cue",
    preset_mode: "update_existing",
    group_mode: "update_existing",
    other_target_modes: {},
    show_update_modal_on_touch: true,
  };
  expect(await api.request<any>("GET", "/api/v1/update/settings")).toEqual(migratedDefaults);
  expect(await readSql(`${bench.dataDir}/desk.sqlite`, "SELECT version FROM schema_info")).toBe("8");
  const authoritativeCueBaseline = (await object<any>(api, "cue_list", cueListId)).body;
  const authoritativePresetBaseline = (await object<any>(api, "preset", presetId)).body;
  const authoritativeGroupBaseline = (await object<any>(api, "group", groupId)).body;

  await api.command("selection.set", { fixtures: [first, second] });
  await api.command("programmer.set", { fixture_id: first, attribute: "intensity", value: 0.8 });
  await api.command("programmer.set", { fixture_id: second, attribute: "intensity", value: 0.7 });
  const unrelatedBeforeCue = await objectRows(showEntry.path, "cue_list", cueListId);
  const cueResult = await api.request<any>("POST", "/api/v1/update/apply", {
    target: { family: { type: "cue" }, object_id: cueListId, cue_id: cueId, cue_number: 1 },
    mode: { target_type: "cue", mode: migratedDefaults.cue_mode },
    expected_revision: 1,
  });
  expect(cueResult.revision_after).toBe(2);
  const updatedCue = await object<any>(api, "cue_list", cueListId);
  expect(updatedCue.body.cues[0].changes).toHaveLength(1);
  expect(updatedCue.body.cues[0].changes[0]).toMatchObject({
    fixture_id: first,
    attribute: "intensity",
    value: { kind: "normalized" },
    automatic_restore: false,
  });
  expect(updatedCue.body.cues[0].changes[0].value.value).toBeCloseTo(0.8, 5);
  expect((await programmer(api)).values).toEqual(expect.arrayContaining([
    expect.objectContaining({ fixture_id: first, attribute: "intensity" }),
    expect.objectContaining({ fixture_id: second, attribute: "intensity" }),
  ]));
  expect(await objectRows(showEntry.path, "cue_list", cueListId)).toEqual(unrelatedBeforeCue);
  await api.request("POST", `/api/v1/shows/${show.id}/objects/cue_list/${cueListId}/undo`, undefined, true, updatedCue.revision);
  expect((await object<any>(api, "cue_list", cueListId)).body).toEqual(authoritativeCueBaseline);

  const unrelatedBeforePreset = await objectRows(showEntry.path, "preset", presetId);
  const preset = await object<any>(api, "preset", presetId);
  const presetResult = await api.request<any>("POST", "/api/v1/update/apply", {
    target: { family: { type: "preset" }, object_id: presetId },
    mode: { target_type: "existing_content", mode: migratedDefaults.preset_mode },
    expected_revision: preset.revision,
  });
  expect(presetResult.revision_after).toBe(preset.revision + 1);
  const updatedPreset = await object<any>(api, "preset", presetId);
  expect(Object.keys(updatedPreset.body.values)).toEqual([first]);
  expect(updatedPreset.body.values[first].intensity).toMatchObject({ kind: "normalized" });
  expect(updatedPreset.body.values[first].intensity.value).toBeCloseTo(0.8, 5);
  expect(await objectRows(showEntry.path, "preset", presetId)).toEqual(unrelatedBeforePreset);
  await api.request("POST", `/api/v1/shows/${show.id}/objects/preset/${presetId}/undo`, undefined, true, updatedPreset.revision);
  expect((await object<any>(api, "preset", presetId)).body).toEqual(authoritativePresetBaseline);

  await api.command("selection.set", { fixtures: [second, third, first, fourth] });
  const group = await object<any>(api, "group", groupId);
  const defaultPreview = await api.request<any>("POST", "/api/v1/update/preview", {
    target: { family: { type: "group" }, object_id: groupId },
    mode: { target_type: "existing_content", mode: migratedDefaults.group_mode },
    expected_revision: group.revision,
  });
  expect(defaultPreview.mode).toEqual({ target_type: "existing_content", mode: "update_existing" });
  expect(defaultPreview.items.filter((item: any) => item.outcome.outcome === "unchanged")).toHaveLength(2);
  expect(defaultPreview.items.filter((item: any) => item.outcome.outcome === "ignored")).toHaveLength(2);

  await desk.open(bench.baseUrl);
  await page.keyboard.press("Shift+End");
  await expect(page.getByText(/UPDATE armed · touch a recordable target/i)).toBeVisible();
  await openGroups(page);
  await page.locator(".group-pool-window .group-card").filter({ hasText: "Legacy ordered Group" }).click();
  const updateDialog = page.getByRole("dialog", { name: /Update Legacy ordered Group/i });
  await expect(updateDialog).toBeVisible();
  await expect(updateDialog.getByRole("button", { name: "Update Existing", exact: true })).toHaveClass(/active/);
  await updateDialog.getByRole("button", { name: "Add New", exact: true }).click();
  await updateDialog.getByRole("button", { name: "Update Group", exact: true }).click();
  await expect(page.getByRole("dialog", { name: "Update complete" })).toBeVisible();
  const updatedGroup = await object<any>(api, "group", groupId);
  expect(updatedGroup.body.fixtures).toEqual([first, second, third, fourth]);
  expect((await programmer(api)).selected).toEqual([second, third, first, fourth]);
  await api.request("POST", `/api/v1/shows/${show.id}/objects/group/${groupId}/undo`, undefined, true, updatedGroup.revision);
  expect((await object<any>(api, "group", groupId)).body).toEqual(authoritativeGroupBaseline);

  await bench.stopServerGracefully(api.session!.token);
  await bench.startServer();
  await api.login();
  expect(await api.request<any>("GET", "/api/v1/update/settings")).toEqual(migratedDefaults);
  expect(await readSql(showEntry.path, "SELECT version FROM schema_info")).toBe("4");
  const reopenedPreset = await object<any>(api, "preset", presetId);
  const repeated = await api.request<any>("POST", "/api/v1/update/apply", {
    target: { family: { type: "preset" }, object_id: presetId },
    mode: { target_type: "existing_content", mode: migratedDefaults.preset_mode },
    expected_revision: reopenedPreset.revision,
  });
  expect(repeated.revision_after).toBe(reopenedPreset.revision + 1);
  const repeatedPreset = await object<any>(api, "preset", presetId);
  expect(Object.keys(repeatedPreset.body.values)).toEqual([first]);
  expect(repeatedPreset.body.values[first].intensity.value).toBeCloseTo(0.8, 5);
});

pairedScenario<HighlightScenarioState>({
  id: "HIGHLIGHT-001",
  title: "HIGH follows the actual selection while stepped values remain normal programmer data",
  arrange: async ({ api, bench }, surface) => {
    const show = await loadCanonicalCopy(api, bench, `highlight-001-${surface}`, "default-stage");
    const fixtures = await fixturesByNumber(api, [101, 102, 103]);
    await api.command("selection.set", { fixtures: [fixtures[0].id] });
    await api.command("programmer.set", { fixture_id: fixtures[0].id, attribute: "pan", value: 0.63 });
    await api.command("selection.set", { fixtures: fixtureIds(fixtures) });
    return { showId: show.id, fixtures, storedPresetId: "197" };
  },
  api: async ({ api }, state) => {
    await highlightAction(api, "on");
    state.selectionStayedComplete = selectionsEqual((await programmer(api)).selected, fixtureIds(state.fixtures));
    await highlightAction(api, "next");
    const first = (await programmer(api)).selected[0];
    await api.command("programmer.set", { fixture_id: first, attribute: "pan", value: 0.41 });
    await highlightAction(api, "next");
    const second = (await programmer(api)).selected[0];
    await api.command("programmer.set", { fixture_id: second, attribute: "pan", value: 0.52 });
    await highlightAction(api, "off");
    await storeCurrentProgrammerPreset(api, state.showId, state.storedPresetId);
  },
  ui: async ({ api, bench, desk, page }, state) => {
    await desk.open(bench.baseUrl);
    await expectSelection(api, fixtureIds(state.fixtures));
    await clickHighlightKey(page, api, "HIGH");
    await expect.poll(async () => (await highlightState(api)).active).toBe(true);
    state.selectionStayedComplete = selectionsEqual((await programmer(api)).selected, fixtureIds(state.fixtures));
    await clickHighlightKey(page, api, "NEXT", [state.fixtures[0].id]);
    await api.command("programmer.set", { fixture_id: state.fixtures[0].id, attribute: "pan", value: 0.41 });
    await clickHighlightKey(page, api, "NEXT", [state.fixtures[1].id]);
    await api.command("programmer.set", { fixture_id: state.fixtures[1].id, attribute: "pan", value: 0.52 });
    await clickHighlightKey(page, api, "HIGH");
    await expect.poll(async () => (await highlightState(api)).active).toBe(false);
    await storeCurrentProgrammerPreset(api, state.showId, state.storedPresetId);
  },
  assert: async ({ api }, state) => {
    expect(state.selectionStayedComplete).toBe(true);
    const highlight = await highlightState(api);
    expect(highlight).toMatchObject({ active: false, output_enabled: false, mode: "step", active_index: 1, can_previous: true, can_next: true });
    expect(highlight.remembered.map((fixture: any) => fixture.fixture_id)).toEqual(fixtureIds(state.fixtures));
    expect(highlight.active_fixture.fixture_id).toBe(state.fixtures[1].id);
    const current = await programmer(api);
    expect(current.selected).toEqual([state.fixtures[1].id]);
    const values = current.values ?? [];
    expect(values.some((entry) => entry.fixture_id === state.fixtures[0].id && entry.attribute === "pan")).toBe(true);
    expect(values.some((entry) => entry.fixture_id === state.fixtures[1].id && entry.attribute === "pan")).toBe(true);
    expect(values.every((entry) => !String(entry.attribute).toLowerCase().includes("highlight"))).toBe(true);
    const preset = await object<any>(api, "preset", state.storedPresetId);
    const storedAttributes = Object.values(preset.body.values ?? {}).flatMap((attributes: any) => Object.keys(attributes));
    expect(storedAttributes).toContain("pan");
    expect(storedAttributes.every((attribute) => !attribute.toLowerCase().includes("highlight"))).toBe(true);
  },
});

pairedScenario<HighlightSurfaceState>({
  id: "HIGHLIGHT-002",
  title: "live Group ALL restoration, external selection, empty HIGH, and lifecycle stay authoritative",
  arrange: async ({ api, bench }, surface) => {
    const show = await loadCanonicalCopy(api, bench, `highlight-002-${surface}`, "default-stage");
    const fixtures = await fixturesByNumber(api, [101, 102, 103, 104, 105, 106]);
    const initial = fixtureIds(fixtures.slice(0, 4));
    const updated = [fixtures[3].id, fixtures[1].id, fixtures[4].id, fixtures[0].id];
    const liveGroup = { id: "30", name: "Feature 20 Live Group", initial, updated };
    await putObject(api, "group", liveGroup.id, groupBody(liveGroup.name, initial));
    return {
      showId: show.id,
      fixtures,
      liveGroup,
    };
  },
  api: async ({ api }, state) => {
    await api.command("group.select", { group_id: state.liveGroup.id });
    await highlightAction(api, "next");
    await highlightAction(api, "next");
    state.steppedSelection = [...(await programmer(api)).selected];
    const stored = await object<any>(api, "group", state.liveGroup.id);
    await putObject(api, "group", state.liveGroup.id, { ...stored.body, fixtures: state.liveGroup.updated }, stored.revision);
    await highlightAction(api, "all");
    state.restoredSelection = [...(await programmer(api)).selected];
    await highlightAction(api, "on");
    await api.command("selection.set", { fixtures: [] });
    state.highSurvivedEmpty = (await highlightState(api)).active && (await programmer(api)).selected.length === 0;
    await api.command("selection.set", { fixtures: [state.fixtures[2].id, state.fixtures[3].id] });
    state.highFollowedSelection = (await highlightState(api)).active
      && selectionsEqual((await programmer(api)).selected, [state.fixtures[2].id, state.fixtures[3].id]);
    const deskId = api.session!.desk.id;
    await api.login("Operator", deskId);
    state.reconnectRetained = (await highlightState(api)).active;
    await api.request("POST", `/api/v1/shows/${state.showId}/open`, { transition: "hold_current" });
  },
  ui: async ({ api, bench, desk, page }, state) => {
    await desk.open(bench.baseUrl);
    await openGroups(page);
    await page.locator(".group-pool-window .group-card").filter({ hasText: state.liveGroup.name }).first().click();
    await expectSelection(api, state.liveGroup.initial);
    await clickHighlightKey(page, api, "NEXT", [state.liveGroup.initial[0]]);
    await clickHighlightKey(page, api, "NEXT", [state.liveGroup.initial[1]]);
    state.steppedSelection = [...(await programmer(api)).selected];
    const stored = await object<any>(api, "group", state.liveGroup.id);
    await putObject(api, "group", state.liveGroup.id, { ...stored.body, fixtures: state.liveGroup.updated }, stored.revision);
    await clickHighlightKey(page, api, "ALL", state.liveGroup.updated);
    state.restoredSelection = [...(await programmer(api)).selected];
    await clickHighlightKey(page, api, "HIGH");
    await page.locator('[data-keypad-key="CLR"]').click();
    await expectSelection(api, []);
    state.highSurvivedEmpty = (await highlightState(api)).active;
    await openBuiltIn(page, "Fixtures");
    await fixtureSheetRowById(page, state.fixtures[2].id).click();
    await fixtureSheetRowById(page, state.fixtures[3].id).click();
    await expectSelection(api, [state.fixtures[2].id, state.fixtures[3].id]);
    state.highFollowedSelection = (await highlightState(api)).active;
    await page.reload();
    await expect(page.locator(".connection-cover")).toBeHidden({ timeout: 10_000 });
    state.reconnectRetained = (await highlightState(api)).active;
    await api.request("POST", `/api/v1/shows/${state.showId}/open`, { transition: "hold_current" });
  },
  assert: async ({ api }, state) => {
    expect(state.steppedSelection).toEqual([state.liveGroup.initial[1]]);
    expect(state.restoredSelection).toEqual(state.liveGroup.updated);
    expect(state.highSurvivedEmpty).toBe(true);
    expect(state.highFollowedSelection).toBe(true);
    expect(state.reconnectRetained).toBe(true);
    const highlight = await highlightState(api);
    expect(highlight).toMatchObject({
      active: false,
      output_enabled: false,
      mode: "selection",
      active_index: null,
      active_fixture: null,
    });
    expect(highlight.remembered.map((fixture: any) => fixture.fixture_id)).toEqual([
      state.fixtures[2].id,
      state.fixtures[3].id,
    ]);
  },
});

pairedScenario<HighlightSequenceState>({
  id: "HIGHLIGHT-003",
  title: "PREV NEXT ALL mutate the real selection and preserve exact Programmer keypad geometry",
  arrange: async ({ api, bench }, surface) => {
    const show = await loadCanonicalCopy(api, bench, `highlight-003-${surface}`, "default-stage");
    const fixtures = await fixturesByNumber(api, [101, 102, 103, 104]);
    await api.command("selection.set", { fixtures: fixtureIds(fixtures) });
    return {
      showId: show.id,
      fixtures,
      expectedSequence: [
        [fixtures[0].id],
        [fixtures[1].id],
        fixtureIds(fixtures),
        [fixtures[3].id],
        [fixtures[2].id],
        [fixtures[1].id],
      ],
      observedSequence: [],
      singletonGroupId: "92",
      completeGroupId: "93",
    };
  },
  api: async ({ api }, state) => {
    for (const action of ["next", "next", "all", "previous", "previous", "previous"] as const) {
      await highlightAction(api, action);
      state.observedSequence.push([...(await programmer(api)).selected]);
    }
    state.highStayedOff = !(await highlightState(api)).active;

    await highlightAction(api, "next");
    await highlightAction(api, "next");
    await highlightAction(api, "next");
    state.wrappedForward = selectionsEqual((await programmer(api)).selected, [state.fixtures[0].id]);
    await highlightAction(api, "previous");
    state.wrappedBackward = selectionsEqual((await programmer(api)).selected, [state.fixtures[3].id]);

    await restoreSecondStep(api);
    await api.command("programmer.set", { fixture_id: state.fixtures[1].id, attribute: "pan", value: 0.72 });
    await api.executeLegacyCommandLine(`RECORD GROUP ${state.singletonGroupId}`);
    await highlightAction(api, "all");
    await api.executeLegacyCommandLine(`RECORD GROUP ${state.completeGroupId}`);

    await highlightAction(api, "on");
    await api.command("selection.set", { fixtures: [] });
    state.highSurvivedEmpty = (await highlightState(api)).active;
    await api.command("selection.set", { fixtures: [state.fixtures[2].id, state.fixtures[3].id] });
    state.highFollowedSelection = (await highlightState(api)).active;
    await highlightAction(api, "off");
    const removedActions = await Promise.all(["capture", "reset"].map(async (action) => {
      try {
        await api.request("POST", "/api/v1/highlight/action", { action });
        return false;
      } catch {
        return true;
      }
    }));
    state.removedCaptureRejected = removedActions.every(Boolean);
  },
  ui: async ({ api, bench, desk, page }, state) => {
    await desk.open(bench.baseUrl);
    await expectSelection(api, fixtureIds(state.fixtures));
    for (const [index, key] of (["NEXT", "NEXT", "ALL", "PREV", "PREV", "PREV"] as const).entries()) {
      await clickHighlightKey(page, api, key, state.expectedSequence[index]);
      state.observedSequence.push([...(await programmer(api)).selected]);
    }
    state.highStayedOff = !(await highlightState(api)).active;

    await clickHighlightKey(page, api, "NEXT", [state.fixtures[2].id]);
    await clickHighlightKey(page, api, "NEXT", [state.fixtures[3].id]);
    await clickHighlightKey(page, api, "NEXT", [state.fixtures[0].id]);
    state.wrappedForward = true;
    await clickHighlightKey(page, api, "PREV", [state.fixtures[3].id]);
    state.wrappedBackward = true;

    await clickHighlightKey(page, api, "ALL", fixtureIds(state.fixtures));
    await clickHighlightKey(page, api, "NEXT", [state.fixtures[0].id]);
    await clickHighlightKey(page, api, "NEXT", [state.fixtures[1].id]);
    await setPanThroughUi(page, 72);
    await pressCommand(page, `RECORD GROUP ${state.singletonGroupId}`, `RECORD GROUP ${state.singletonGroupId}`);
    await clickHighlightKey(page, api, "ALL", fixtureIds(state.fixtures));
    await pressCommand(page, `RECORD GROUP ${state.completeGroupId}`, `RECORD GROUP ${state.completeGroupId}`);

    await clickHighlightKey(page, api, "NEXT", [state.fixtures[0].id]);
    await openBuiltIn(page, "Fixtures");
    await assertFixtureSheetStep(page, state.fixtures, state.fixtures[0].number);
    await clickHighlightKey(page, api, "HIGH");
    await assertFixtureSheetStep(page, state.fixtures, state.fixtures[0].number);
    state.fixtureSheetVerified = true;

    await page.locator('[data-keypad-key="CLR"]').click();
    await expectSelection(api, []);
    state.highSurvivedEmpty = (await highlightState(api)).active;
    await fixtureSheetRowById(page, state.fixtures[2].id).click();
    await fixtureSheetRowById(page, state.fixtures[3].id).click();
    await expectSelection(api, [state.fixtures[2].id, state.fixtures[3].id]);
    state.highFollowedSelection = (await highlightState(api)).active;
    await clickHighlightKey(page, api, "HIGH");
    await expectSelection(api, [state.fixtures[2].id, state.fixtures[3].id]);

    await page.keyboard.press("Alt+H");
    await expect.poll(async () => (await highlightState(api)).active).toBe(true);
    await page.waitForTimeout(175);
    await page.keyboard.press("Alt+ArrowRight");
    await expectSelection(api, [state.fixtures[2].id]);
    await page.waitForTimeout(175);
    await page.keyboard.press("Alt+ArrowLeft");
    await expectSelection(api, [state.fixtures[3].id]);
    await page.waitForTimeout(175);
    await page.keyboard.press("Alt+a");
    await expectSelection(api, [state.fixtures[2].id, state.fixtures[3].id]);
    const beforeAltCapture = await highlightState(api);
    await page.keyboard.press("Alt+c");
    await page.waitForTimeout(175);
    const afterAltCapture = await highlightState(api);
    state.altCaptureWasNoOp = JSON.stringify(afterAltCapture) === JSON.stringify(beforeAltCapture);

    await verifyProgrammerKeypadGeometry(page, api);
    await operateProgrammerFade(page, api);
    state.geometryVerified = true;
    await expect(page.locator(".command-line-bar .highlight-feedback, .command-line-bar [aria-label='Highlight status']")).toHaveCount(0);
    state.noCommandBarPanel = true;
  },
  assert: async ({ api }, state, surface) => {
    expect(state.observedSequence).toEqual(state.expectedSequence);
    expect(state.highStayedOff).toBe(true);
    expect(state.wrappedForward).toBe(true);
    expect(state.wrappedBackward).toBe(true);
    expect(state.highSurvivedEmpty).toBe(true);
    expect(state.highFollowedSelection).toBe(true);
    expect((await object<any>(api, "group", state.singletonGroupId)).body.fixtures).toEqual([state.fixtures[1].id]);
    expect((await object<any>(api, "group", state.completeGroupId)).body.fixtures).toEqual(fixtureIds(state.fixtures));
    const current = await programmer(api);
    expect(current.values.some((entry) => entry.fixture_id === state.fixtures[1].id && entry.attribute === "pan")).toBe(true);
    if (surface === "api") {
      expect(state.removedCaptureRejected).toBe(true);
    } else {
      expect(state.altCaptureWasNoOp).toBe(true);
      expect(state.geometryVerified).toBe(true);
      expect(state.fixtureSheetVerified).toBe(true);
      expect(state.noCommandBarPanel).toBe(true);
    }
  },
});

pairedScenario<FixtureProfileState>({
  id: "FIXTURE-001",
  title: "a complete fixture profile is created through the desk-wide revisioned library",
  arrange: async ({ api, bench }, surface) => {
    await loadCanonicalCopy(api, bench, `fixture-001-${surface}`);
    return {
      manufacturer: `Acceptance ${surface}`,
      name: `Revisioned profile ${crypto.randomUUID().slice(0, 8)}`,
    };
  },
  api: async ({ api }, state) => {
    const profile = blankFixtureProfile();
    profile.manufacturer = state.manufacturer;
    profile.name = state.name;
    await api.request("PUT", "/api/v1/fixture-profiles", profile, true, 0);
  },
  ui: async ({ bench, desk, page }, state) => {
    await desk.open(bench.baseUrl);
    await page.getByRole("button", { name: /Open show menu/ }).click();
    await page.getByRole("button", { name: "Enter Setup", exact: true }).click();
    await page.getByRole("button", { name: "Open Fixture Library", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Fixture Library" })).toBeVisible();
    await page.getByRole("button", { name: "Create fixture", exact: true }).click();
    const editor = page.getByRole("dialog", { name: "Create fixture profile" });
    await editor.getByLabel(/^Manufacturer/).fill(state.manufacturer);
    await editor.getByLabel(/^Fixture name/).fill(state.name);
    await editor.getByRole("button", { name: "Save fixture", exact: true }).click();
    await expect(editor).toBeHidden();
  },
  assert: async ({ api }, state) => {
    const profiles = await api.request<any[]>("GET", "/api/v1/fixture-profiles", undefined, false);
    const profile = profiles.find((candidate) => candidate.manufacturer === state.manufacturer && candidate.name === state.name);
    expect(profile).toBeDefined();
    expect(profile).toMatchObject({ schema_version: 2, revision: 1 });
    expect(profile.modes).toHaveLength(1);
    expect(profile.modes[0]).toMatchObject({ name: "Default", splits: [{ number: 1, footprint: 1 }] });
    const revisions = await api.request<any[]>("GET", `/api/v1/fixture-profiles/${profile.id}/revisions`, undefined, false);
    expect(revisions.map((candidate) => candidate.revision)).toEqual([1]);
  },
});

pairedScenario<MatterScenarioState>({
  id: "MATTER-001",
  title: "the desk-persistent Matter bridge toggle exposes stable explicit page playback lights",
  arrange: async ({ api, bench }, surface) => {
    await loadCanonicalCopy(api, bench, `matter-001-${surface}`);
    const response = await api.request<any>("GET", "/api/v1/configuration", undefined, false);
    if (response.configuration.matter_enabled) {
      await api.request("PUT", "/api/v1/configuration", { ...response.configuration, matter_enabled: false });
    }
    const assignment = await assignFaderlessMatterPlayback(api);
    return { observed: null, ...assignment };
  },
  api: async ({ api }, state) => {
    const response = await api.request<any>("GET", "/api/v1/configuration", undefined, false);
    await api.request("PUT", "/api/v1/configuration", { ...response.configuration, matter_enabled: true });
    state.observed = await api.request<any>("GET", "/api/v1/matter/status");
    const enabled = await api.request<any>("GET", "/api/v1/configuration", undefined, false);
    await api.request("PUT", "/api/v1/configuration", { ...enabled.configuration, matter_enabled: false });
  },
  ui: async ({ api, bench, desk, page }, state) => {
    await desk.open(bench.baseUrl);
    await page.getByRole("button", { name: /Open show menu/ }).click();
    await page.getByRole("button", { name: "Enter Setup", exact: true }).click();
    await page.locator(".setup-window nav").getByRole("button", { name: "Network & Inputs", exact: true }).click();
    const settings = page.locator('article[aria-label="Matter playback bridge"]');
    const toggle = settings.getByRole("switch", { name: "Matter server disabled" });
    await expect(settings.getByText("Desk installation · shared across shows and Desktops")).toBeVisible();
    await expect(toggle).not.toBeChecked();
    await toggle.click();
    await expect(settings.getByRole("switch", { name: "Matter server enabled" })).toBeChecked();
    await expect.poll(async () => (await api.request<any>("GET", "/api/v1/configuration", undefined, false)).configuration.matter_enabled).toBe(true);
    state.observed = await api.request<any>("GET", "/api/v1/matter/status");
    await settings.getByRole("switch", { name: "Matter server enabled" }).click();
    await expect.poll(async () => (await api.request<any>("GET", "/api/v1/configuration", undefined, false)).configuration.matter_enabled).toBe(false);
  },
  assert: async ({ api }, state) => {
    expect(state.observed).toBeTruthy();
    expect(state.observed.enabled).toBe(true);
    const endpointIds = state.observed.lights.map((light: any) => light.endpoint_id);
    expect(new Set(endpointIds).size).toBe(endpointIds.length);
    for (const light of state.observed.lights) {
      expect(light.endpoint_id).toBe(1 + (light.page - 1) * 127 + (light.playback - 1));
      expect(light.playback_number).toBeGreaterThan(0);
      expect(light.level).toBeGreaterThanOrEqual(0);
      expect(light.level).toBeLessThanOrEqual(254);
    }
    const faderlessEndpoint = 1 + (state.page - 1) * 127 + (state.slot - 1);
    expect(state.observed.lights).toContainEqual(expect.objectContaining({
      endpoint_id: faderlessEndpoint,
      page: state.page,
      playback: state.slot,
      playback_number: state.playbackNumber,
      name: expect.stringContaining("Matter Button Only"),
    }));
    const emptyEndpoint = 1 + (state.page - 1) * 127 + (state.emptySlot - 1);
    expect(endpointIds).not.toContain(emptyEndpoint);
    const configuration = await api.request<any>("GET", "/api/v1/configuration", undefined, false);
    expect(configuration.configuration.matter_enabled).toBe(false);
    const disabled = await api.request<any>("GET", "/api/v1/matter/status");
    expect(disabled.lights).toEqual([]);
  },
});

async function assignFaderlessMatterPlayback(api: Parameters<typeof objects>[0]): Promise<{
  page: number;
  slot: number;
  emptySlot: number;
  playbackNumber: number;
}> {
  const pages = await objects<any>(api, "playback_page");
  const pagesByNumber = new Map<number, (typeof pages)[number]>(
    pages.map((page) => [Number(page.body.number), page]),
  );
  const emptyPageNumber = Array.from({ length: 127 }, (_, index) => index + 1)
    .find((page) => Object.keys(pagesByNumber.get(page)?.body.slots ?? {}).length === 0);
  const pageNumber = emptyPageNumber ?? Array.from({ length: 127 }, (_, index) => index + 1)
    .find((page) => {
      const assigned = new Set(Object.keys(pagesByNumber.get(page)?.body.slots ?? {}).map(Number));
      return Array.from({ length: 126 }, (_, index) => index + 1)
        .some((slot) => !assigned.has(slot) && !assigned.has(slot + 1));
    });
  expect(pageNumber).toBeDefined();
  const pageState = pagesByNumber.get(pageNumber!);
  const assignedSlots = new Set(Object.keys(pageState?.body.slots ?? {}).map(Number));
  const slot = Array.from({ length: 126 }, (_, index) => index + 1)
    .find((candidate) => !assignedSlots.has(candidate) && !assignedSlots.has(candidate + 1));
  expect(slot).toBeDefined();
  const emptySlot = slot! + 1;
  const existingCueList = (await objects<any>(api, "cue_list"))[0];
  const cueListId = existingCueList?.id ?? await createMatterAcceptanceCueList(api);
  const result = await api.request<any>(
    "PUT",
    `/api/v1/playback-pages/${pageNumber}/slots/${slot}`,
    {
      playback: {
        number: 0,
        name: "Matter Button Only",
        target: { type: "cue_list", cue_list_id: cueListId },
        buttons: ["toggle", "none", "none"],
        button_count: 1,
        fader: "master",
        has_fader: false,
        go_activates: true,
        auto_off: false,
        xfade_millis: 0,
        color: "#20c997",
        flash_release: "release_all",
        protect_from_swap: false,
      },
      expected_playback_revision: 0,
      expected_page_revision: pageState?.revision ?? 0,
    },
  );
  return {
    page: pageNumber!,
    slot: slot!,
    emptySlot,
    playbackNumber: result.playback.number,
  };
}

async function createMatterAcceptanceCueList(api: Parameters<typeof objects>[0]): Promise<string> {
  const fixture = (await objects<any>(api, "patched_fixture"))[0];
  expect(fixture).toBeDefined();
  const id = crypto.randomUUID();
  await putObject(api, "cue_list", id, {
    id,
    name: "Matter Acceptance Cuelist",
    priority: 0,
    mode: "sequence",
    looped: false,
    chaser_step_millis: 1_000,
    speed_group: null,
    cues: [{
      id: crypto.randomUUID(),
      number: 1,
      name: "Matter On",
      changes: [{
        fixture_id: fixture.body.fixture_id,
        attribute: "intensity",
        value: { kind: "normalized", value: 1 },
        automatic_restore: false,
      }],
      group_changes: [],
      fade_millis: 0,
      delay_millis: 0,
      trigger: { type: "manual" },
      phasers: [],
    }],
  });
  return id;
}

function escapeRegex(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

async function openBuiltIn(page: Page, name: string): Promise<void> {
  const entry = page.locator(".dock-entry").filter({ hasText: name }).first();
  if (!await entry.isVisible()) await page.getByRole("button", { name: "BUILT-INS", exact: true }).click();
  await expect(entry).toBeVisible();
  await entry.click();
}

async function openGroups(page: Page): Promise<void> {
  await page.locator('[data-keypad-key="SHIFT"]').click();
  await page.locator('[data-keypad-key="1"]').click();
  await expect(page.locator(".group-pool-window")).toBeVisible();
}

function fixtureSheetRow(page: Page, number: number) {
  return page
    .locator(".fixture-window .ui-data-table-row:not(.header)")
    .filter({ has: page.getByRole("cell", { name: String(number), exact: true }) })
    .first();
}

function fixtureSheetRowById(page: Page, fixtureId: string) {
  return page.locator(`.fixture-window .ui-data-table-row[data-fixture-id="${fixtureId}"]`).first();
}

async function storeCurrentProgrammerPreset(api: Parameters<typeof objects>[0], showId: string, presetId: string) {
  const programmers = await api.request<any[]>("GET", "/api/v1/programmers", undefined, false);
  const current = programmers.find((entry) => entry.session_id === api.session!.session_id);
  expect(current).toBeDefined();
  const values: Record<string, Record<string, unknown>> = {};
  for (const entry of current.values ?? []) {
    (values[entry.fixture_id] ??= {})[entry.attribute] = entry.value;
  }
  await api.request("POST", `/api/v1/shows/${showId}/presets/${presetId}/store`, {
    mode: "overwrite",
    preset: { name: "Highlight isolation", family: "Mixed", values, group_values: {} },
  }, true, 0);
}

async function fixturesByNumber(
  api: Parameters<typeof objects>[0],
  numbers: number[],
): Promise<HighlightFixture[]> {
  const patched = await objects<any>(api, "patched_fixture");
  const byNumber = new Map<number, HighlightFixture>(patched.map((entry) => [
    Number(entry.body.fixture_number),
    { id: entry.body.fixture_id as string, number: Number(entry.body.fixture_number) },
  ]));
  return numbers.map((number) => {
    const fixture = byNumber.get(number);
    expect(fixture, `Fixture ${number} must exist in default-stage.show`).toBeDefined();
    return fixture!;
  });
}

function fixtureIds(fixtures: HighlightFixture[]): string[] {
  return fixtures.map((fixture) => fixture.id);
}

function selectionsEqual(actual: string[], expected: string[]): boolean {
  return JSON.stringify(actual) === JSON.stringify(expected);
}

function groupBody(name: string, fixtures: string[]) {
  return {
    derived_from: null,
    fixtures,
    frozen_from: null,
    master: 1,
    name,
    playback_fader: null,
    programming: {},
  };
}

async function runSql(file: string, sql: string): Promise<void> {
  await sqlite("sqlite3", [file, sql]);
}

async function readSql(file: string, sql: string): Promise<string> {
  return (await sqlite("sqlite3", ["-noheader", file, sql])).stdout.trim();
}

async function objectRows(file: string, excludedKind: string, excludedId: string): Promise<string> {
  const kind = excludedKind.replaceAll("'", "''");
  const id = excludedId.replaceAll("'", "''");
  return readSql(file, `SELECT group_concat(kind||'|'||id||'|'||revision||'|'||length(body_json), char(10)) FROM (SELECT kind,id,revision,body_json FROM objects WHERE NOT (kind='${kind}' AND id='${id}') ORDER BY kind,id)`);
}

async function extractFixtureAsset(archive: string, asset: string, destination: string): Promise<void> {
  const archivePath = fileURLToPath(new URL(`../assets/fixture-library/${archive}`, import.meta.url));
  const bytes = await new Promise<Buffer>((resolve, reject) => {
    execFile("unzip", ["-p", archivePath, asset], { encoding: "buffer", maxBuffer: 2 * 1024 * 1024 }, (error, stdout) => {
      if (error) reject(error);
      else resolve(Buffer.from(stdout));
    });
  });
  await fs.writeFile(destination, bytes);
}

async function selectConfinedFile(page: Page, filename: string): Promise<void> {
  const picker = page.getByRole("dialog", { name: "Choose files or folders" });
  await expect(picker).toBeVisible();
  await picker.getByRole("button", { name: `${filename}, file` }).click();
  await picker.getByRole("button", { name: "Select", exact: true }).click();
  await expect(picker).toBeHidden();
}

async function chooseCustomSelect(container: Locator, label: string, option: string): Promise<void> {
  const field = container.getByText(label, { selector: "label", exact: true }).locator("..");
  await field.locator(".ui-select-trigger").click();
  await container.page().getByRole("option", { name: option, exact: true }).click();
}

async function highlightState(api: Parameters<typeof objects>[0]): Promise<any> {
  return api.request<any>("GET", "/api/v1/highlight", undefined, true);
}

async function highlightAction(
  api: Parameters<typeof objects>[0],
  action: "on" | "off" | "toggle" | "previous" | "next" | "all",
): Promise<void> {
  await api.request("POST", "/api/v1/highlight/action", { action });
  // The shared hardware/software repeat guard intentionally rejects duplicate
  // physical presses inside 150 ms. Acceptance actions model distinct presses.
  await new Promise((resolve) => setTimeout(resolve, 175));
}

async function expectSelection(api: Parameters<typeof objects>[0], expected: string[]): Promise<void> {
  await expect.poll(async () => (await programmer(api)).selected).toEqual(expected);
}

function highlightKey(page: Page, key: "HIGH" | "PREV" | "NEXT" | "ALL") {
  const fallback = {
    HIGH: ".highlight-toggle",
    PREV: ".highlight-previous",
    NEXT: ".highlight-next",
    ALL: ".highlight-all",
  }[key];
  return page.locator(`[data-keypad-key="${key}"], ${fallback}`).first();
}

async function clickHighlightKey(
  page: Page,
  api: Parameters<typeof objects>[0],
  key: "HIGH" | "PREV" | "NEXT" | "ALL",
  expectedSelection?: string[],
): Promise<void> {
  const button = highlightKey(page, key);
  await expect(button).toBeVisible();
  await expect(button).toBeEnabled();
  await button.click();
  if (expectedSelection) await expectSelection(api, expectedSelection);
  await page.waitForTimeout(175);
}

async function restoreSecondStep(api: Parameters<typeof objects>[0]): Promise<void> {
  await highlightAction(api, "all");
  await highlightAction(api, "next");
  await highlightAction(api, "next");
}

async function setPanThroughUi(page: Page, percent: number): Promise<void> {
  await page.getByRole("button", { name: "Position", exact: true }).click();
  const encoder = page.locator(".vertical-touch-fader-stack").filter({ hasText: "Enc 1 · Pan" });
  await expect(encoder).toBeVisible();
  await encoder.getByRole("button", { name: "Set value" }).click();
  const dialog = page.getByRole("dialog", { name: "Enc 1 · Pan value" });
  await expect(dialog).toBeVisible();
  await page.keyboard.type(String(percent));
  await page.keyboard.press("Enter");
  await expect(dialog).toBeHidden();
}

async function assertFixtureSheetStep(
  page: Page,
  fixtures: HighlightFixture[],
  activeNumber: number,
): Promise<void> {
  for (const fixture of fixtures) {
    const row = fixtureSheetRowById(page, fixture.id);
    await expect(row).toBeVisible();
    await expect(row).toHaveAttribute(
      "data-step-selection",
      fixture.number === activeNumber ? "active" : "base",
    );
  }
}

async function verifyProgrammerKeypadGeometry(
  page: Page,
  api: Parameters<typeof objects>[0],
): Promise<void> {
  const upperNames = ["HIGH", "PREV", "NEXT", "ALL"] as const;
  const lowerNames = ["GRP", "CUE", "TIME", "DIV"] as const;
  const upper = await Promise.all(upperNames.map(async (name) => {
    const locator = highlightKey(page, name);
    await expect(locator).toHaveText(name);
    const box = await locator.boundingBox();
    expect(box).toBeTruthy();
    return { locator, box: box! };
  }));
  const lower = await Promise.all(lowerNames.map(async (name) => {
    const locator = page.locator(`[data-keypad-key="${name}"]`);
    const box = await locator.boundingBox();
    expect(box).toBeTruthy();
    return { locator, box: box! };
  }));

  const tolerance = 1.5;
  for (let index = 0; index < upper.length; index += 1) {
    expect(Math.abs(centerX(upper[index].box) - centerX(lower[index].box))).toBeLessThanOrEqual(tolerance);
    expect(Math.abs(upper[index].box.width - lower[index].box.width)).toBeLessThanOrEqual(tolerance);
    expect(Math.abs(upper[index].box.height - lower[index].box.height)).toBeLessThanOrEqual(tolerance);
    expect(upper[index].box.y + upper[index].box.height).toBeLessThanOrEqual(lower[index].box.y);
  }
  const upperY = centerY(upper[0].box);
  const lowerY = centerY(lower[0].box);
  for (const item of upper) expect(Math.abs(centerY(item.box) - upperY)).toBeLessThanOrEqual(tolerance);
  for (const item of lower) expect(Math.abs(centerY(item.box) - lowerY)).toBeLessThanOrEqual(tolerance);

  const keyStyles = await Promise.all(upper.map(({ locator }) => locator.evaluate((element) => {
    const style = getComputedStyle(element);
    return {
      alignItems: style.alignItems,
      borderRadius: style.borderRadius,
      fontFamily: style.fontFamily,
      fontSize: style.fontSize,
      justifyContent: style.justifyContent,
      textAlign: style.textAlign,
    };
  })));
  expect(keyStyles.every((style) => JSON.stringify(style) === JSON.stringify(keyStyles[0]))).toBe(true);
  await expect(upper[0].locator).toHaveClass(/highlight-armed/);
  await clickHighlightKey(page, api, "HIGH");
  await expect.poll(async () => (await highlightState(api)).active).toBe(false);
  await expect(upper[0].locator).toHaveClass(/highlight-off/);

  const fade = page.locator(".numeric-pad-fade");
  await expect(fade).toHaveAttribute("data-grid-column-span", "2");
  await expect(fade).toHaveAttribute("data-grid-row-span", "2");
  const fadeBox = await fade.boundingBox();
  const delBox = await page.locator('[data-keypad-key="DEL"]').boundingBox();
  const clrBox = await page.locator('[data-keypad-key="CLR"]').boundingBox();
  const movBox = await page.locator('[data-keypad-key="MOV"]').boundingBox();
  expect(fadeBox && delBox && clrBox && movBox).toBeTruthy();
  expect(Math.abs(fadeBox!.width - (clrBox!.x + clrBox!.width - delBox!.x))).toBeLessThanOrEqual(tolerance);
  expect(Math.abs(fadeBox!.height - (movBox!.y + movBox!.height - delBox!.y))).toBeLessThanOrEqual(tolerance);
  const followingGap = delBox!.y - (fadeBox!.y + fadeBox!.height);
  const normalGap = clrBox!.x - (delBox!.x + delBox!.width);
  expect(Math.abs(followingGap - normalGap)).toBeLessThanOrEqual(tolerance);
}

async function softwareHighlightGeometry(page: Page) {
  return Promise.all([
    page.locator(".programmer-number-block"),
    highlightKey(page, "HIGH"),
    page.locator('[data-keypad-key="GRP"]'),
    page.locator(".global-store-button"),
    page.locator(".preload-button"),
  ].map(async (locator) => {
    const box = await locator.boundingBox();
    expect(box).toBeTruthy();
    return roundedBox(box!);
  }));
}

async function hardwareHighlightGeometry(page: Page) {
  return Promise.all([
    page.locator(".hardware-right-pane"),
    page.locator(".hardware-control-summary"),
    page.locator(".global-store-button"),
    page.locator(".preload-button"),
  ].map(async (locator) => {
    const box = await locator.boundingBox();
    expect(box).toBeTruthy();
    return roundedBox(box!);
  }));
}

async function assertReachableAlert(
  page: Page,
  alert: Locator,
  modal: Locator,
  viewport: { width: number; height: number },
) {
  const box = await alert.boundingBox();
  expect(box).toBeTruthy();
  expect(box!.x).toBeGreaterThanOrEqual(0);
  expect(box!.y).toBeGreaterThanOrEqual(0);
  expect(box!.x + box!.width).toBeLessThanOrEqual(viewport.width);
  expect(box!.y + box!.height).toBeLessThanOrEqual(viewport.height);
  const topElementIsAlert = await page.evaluate(({ x, y }) => {
    const top = document.elementFromPoint(x, y);
    return Boolean(top?.closest("[data-highlight-error-alert]"));
  }, { x: centerX(box!), y: centerY(box!) });
  expect(topElementIsAlert).toBe(true);
  const [alertZ, modalZ] = await Promise.all([
    alert.evaluate((element) => Number(getComputedStyle(element).zIndex) || 0),
    modal.evaluate((element) => {
      const layer = element.closest<HTMLElement>(".stacked-modal-layer") ?? element;
      return Number(getComputedStyle(layer).zIndex) || 0;
    }),
  ]);
  expect(alertZ).toBeGreaterThan(modalZ);
}

function roundedBox(box: { x: number; y: number; width: number; height: number }) {
  return Object.fromEntries(Object.entries(box).map(([key, value]) => [key, Math.round(value * 10) / 10]));
}

function centerX(box: { x: number; width: number }): number {
  return box.x + box.width / 2;
}

function centerY(box: { y: number; height: number }): number {
  return box.y + box.height / 2;
}

async function operateProgrammerFade(
  page: Page,
  api: Parameters<typeof objects>[0],
): Promise<void> {
  const fade = page.locator(".numeric-pad-fade");
  const button = fade.getByRole("button", { name: /Prog\. Fade/ });
  await expect(button).toContainText("Prog. Fade");
  await expect(button).toContainText("s");
  await button.click();
  const dialog = page.getByRole("dialog", { name: "Prog. Fade value" });
  await expect(dialog).toBeVisible();
  await page.keyboard.type("4.2");
  await page.keyboard.press("Enter");
  await expect(dialog).toBeHidden();
  await expect.poll(async () => {
    const response = await api.request<any>("GET", "/api/v1/configuration", undefined, false);
    return response.configuration.programmer_fade_millis;
  }).toBe(4_200);
}
