import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import type { ApiDriver } from "../apps/control-ui/e2e/bench/api";
import type { Page } from "@playwright/test";

test.describe.configure({ mode: "serial" });
test.setTimeout(180_000);
test.use({ viewport: { width: 1600, height: 1100 } });

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const SCREENSHOT_DIR = path.join(ROOT, "docs/help/assets/screenshots");
const PANE_SCREENSHOT_DIR = path.join(SCREENSHOT_DIR, "panes");
const WORKFLOW_SCREENSHOT_DIR = path.join(SCREENSHOT_DIR, "workflows");
const SCREENSHOT_TEXT_FILE = "documentation-cue-notes.md";

interface ShowEntry { id: string; name: string }
interface VersionedObject<T = Record<string, unknown>> { id: string; revision: number; body: T }
interface PatchedFixtureBody { fixture_id: string; fixture_number?: number; logical_heads?: Array<{ fixture_id: string }> }

test("captures help and README screenshots from the default show desk", async ({ page, desk, api }) => {
  page.setDefaultTimeout(12_000);
  await fs.mkdir(SCREENSHOT_DIR, { recursive: true });
  await fs.mkdir(PANE_SCREENSHOT_DIR, { recursive: true });
  await fs.mkdir(WORKFLOW_SCREENSHOT_DIR, { recursive: true });
  await Promise.all(
    (await fs.readdir(PANE_SCREENSHOT_DIR))
      .filter((file) => file.endsWith(".png"))
      .map((file) => fs.unlink(path.join(PANE_SCREENSHOT_DIR, file))),
  );
  await Promise.all(
    (await fs.readdir(WORKFLOW_SCREENSHOT_DIR))
      .filter((file) => file.endsWith(".png"))
      .map((file) => fs.unlink(path.join(WORKFLOW_SCREENSHOT_DIR, file))),
  );
  await openSeededDefaultStageShow(api);

  await desk.open(api.baseUrl);
  await page.getByRole("button", { name: "DESKTOPS" }).click();
  await page.getByRole("button", { name: /Programming/ }).click();
  await selectFixtures(page, desk, "1 + 2 + 3 + 4 + 5 + 6");
  await setDimmerByTouch(page, 50);
  await setStagePaneTo3d(page);
  await expect(page.locator(".group-strip .group-card").filter({ hasText: "Front Fresnels" })).toBeVisible();
  await expect(page.locator(".control-section")).toContainText("50%");
  await expect(page.locator("canvas")).toBeVisible();
  await page.waitForTimeout(500);
  await page.locator(".control-section.programmer").screenshot({ path: shot("software-keypad.png") });
  await page.screenshot({ path: shot("default-desk-overview.png"), fullPage: true });

  await openCuelistDetailWithPlayback(page);
  await expect(page.locator(".control-section.playbacks")).toBeVisible();
  await expect(page.locator(".cue-table tbody tr")).toHaveCount(6);
  await expect(page.locator(".playback-fader-bank")).toContainText("Front Fresnels");
  await page.screenshot({ path: shot("cuelist-playback.png"), fullPage: true });

  await page.getByRole("button", { name: "BUILT-INS" }).click();
  await page.locator(".dock-entry").filter({ hasText: "Fixtures" }).click();
  await expect(page.locator(".fixture-window")).toBeVisible();
  await api.request("POST", "/api/v1/highlight/action", { action: "next" });
  await expect(page.locator('.fixture-window [data-step-selection="active"]')).toHaveCount(1);
  await expect(page.locator('.fixture-window [data-step-selection="base"]').first()).toBeVisible();
  await page.screenshot({ path: shot("fixture-sheet-programmer.png"), fullPage: true });
  await api.request("POST", "/api/v1/highlight/action", { action: "all" });

  await captureWorkflowReference(page);
  const patch = await api.request<any>("GET", "/api/v1/patch", undefined, false);
  const selectedDmx = patch.fixtures.find((fixture: any) => fixture.universe === 1 && fixture.address != null)
    ?? patch.fixtures.find((fixture: any) => fixture.universe != null && fixture.address != null);
  if (!selectedDmx) throw new Error("The documentation show has no patched DMX channel");
  await capturePaneReference(page, { universe: selectedDmx.universe, address: selectedDmx.address });

  await page.getByRole("button", { name: /Open show menu/ }).click();
  await page.getByRole("button", { name: "Help", exact: true }).click();
  await expect(page.locator(".help-window")).toBeVisible();
  await page.screenshot({ path: shot("help-command-line.png"), fullPage: true });

  const expectedPaneFiles = paneReference.flatMap(([, , slug, settingsTab]) => [`${slug}.png`, ...(settingsTab ? [`${slug}-settings.png`] : [])]).sort();
  await expect.poll(async () => (await fs.readdir(PANE_SCREENSHOT_DIR)).filter((file) => file.endsWith(".png")).sort()).toEqual(expectedPaneFiles);
  await expect.poll(async () => (await fs.readdir(WORKFLOW_SCREENSHOT_DIR)).filter((file) => file.endsWith(".png")).sort()).toEqual(workflowScreenshots);
});

function shot(file: string): string {
  return path.join(SCREENSHOT_DIR, file);
}

function workflowShot(file: string): string {
  return path.join(WORKFLOW_SCREENSHOT_DIR, file);
}

const workflowScreenshots = [
  "desk-setup-inputs.png",
  "desk-setup-lock.png",
  "desk-setup-network-api.png",
  "desk-setup-output-engine.png",
  "desk-setup-screens.png",
  "desk-setup-shows-recovery.png",
  "desk-setup-timecode.png",
  "desk-setup-users.png",
  "fixture-library-create.png",
  "fixture-library-import.png",
  "fixture-library-mode-editor.png",
  "fixture-library.png",
  "fixture-sheet-settings-columns.png",
  "fixture-sheet-settings-view.png",
  "mvr-export.png",
  "mvr-new-show.png",
  "patch-add-fixture.png",
  "show-change-user.png",
  "show-load-revisions.png",
  "show-menu.png",
  "show-patch.png",
  "stage-settings.png",
  "stage-setup-2d.png",
].sort();

async function openSeededDefaultStageShow(api: ApiDriver): Promise<ShowEntry> {
  const bytes = await fs.readFile(new URL("./fixtures/default-stage.show", import.meta.url));
  const show = await api.request<ShowEntry>("POST", "/api/v1/shows", {
    name: `Docs Default Stage ${crypto.randomUUID()}`,
    data_base64: bytes.toString("base64"),
    overwrite: false,
  });
  await seedScreenshotProgramming(api, show.id);
  await api.request("POST", `/api/v1/shows/${show.id}/open`, { transition: "hold_current" });
  return show;
}

const paneReference = [
  ["presets", "Preset pool", "presets", "Pool"],
  ["groups", "Group pool", "groups", null],
  ["fixtures", "Fixture sheet", "fixtures", "Shortcuts"],
  ["stage", "Stage", "stage", "Stage"],
  ["cuelist_pool", "Cuelist Pool", "cuelist-pool", null],
  ["cues", "Cues · Cuelist", "cues", null],
  ["cuelists", "Cuelists (tabs)", "cuelists", null],
  ["virtual_playbacks", "Virtual Playbacks", "virtual-playbacks", "Virtual Playbacks"],
  ["file_manager", "File Manager", "file-manager", "File Manager"],
  ["text_editor", "Text Editor", "text-editor", "Text Editor"],
  ["channels", "Channels", "channels", null],
  ["dynamics", "Dynamics", "dynamics", null],
  ["dmx", "DMX output", "dmx", null],
  ["help", "Help", "help", null],
] as const;

async function captureWorkflowReference(page: Page) {
  const openShowMenu = async () => {
    await page.getByRole("button", { name: /Open show menu/ }).click();
    await expect(page.locator(".show-modal")).toBeVisible();
  };
  const closeNested = async (selector: string) => {
    const modal = page.locator(selector);
    await modal.locator(".modal-close").click();
    await expect(modal).toBeHidden();
  };

  await openShowMenu();
  await page.locator(".show-modal").getByRole("button", { name: "Enter Setup", exact: true }).hover();
  await page.locator(".show-modal").screenshot({ path: workflowShot("show-menu.png") });
  await page.getByRole("button", { name: "Load", exact: true }).click();
  await page.getByRole("dialog", { name: "Load show" }).screenshot({ path: workflowShot("show-load-revisions.png") });
  await closeNested(".load-show-modal");
  await page.getByRole("button", { name: /Change User/ }).click();
  await page.getByRole("dialog", { name: "Change user" }).screenshot({ path: workflowShot("show-change-user.png") });
  await closeNested('[role="dialog"][aria-label="Change user"]');
  await page.getByRole("button", { name: "New Show", exact: true }).click();
  await page.getByRole("button", { name: "Load from MVR", exact: true }).click();
  await page.getByRole("dialog", { name: "MVR import and export" }).screenshot({ path: workflowShot("mvr-new-show.png") });
  await closeNested(".mvr-modal");
  await page.getByRole("button", { name: "Save As", exact: true }).click();
  await page.getByRole("button", { name: "Export as MVR", exact: true }).click();
  const exportMvr = page.getByRole("dialog", { name: "MVR import and export" });
  await expect(exportMvr.locator(".mvr-summary")).toBeVisible();
  await exportMvr.screenshot({ path: workflowShot("mvr-export.png") });
  await closeNested(".mvr-modal");
  await page.getByRole("button", { name: "Show Patch", exact: true }).click();

  await expect(page.locator(".patch-window")).toBeVisible();
  await page.locator(".patch-window").screenshot({ path: workflowShot("show-patch.png") });
  await page.getByRole("button", { name: "+ Add fixture", exact: true }).click();
  await expect(page.locator(".fixture-browser-modal")).toBeVisible();
  await page.locator(".fixture-browser-modal").screenshot({ path: workflowShot("patch-add-fixture.png") });
  await page.getByRole("button", { name: "Close Add fixture", exact: true }).click();

  await openShowMenu();
  await page.getByRole("button", { name: "Enter Setup", exact: true }).click();
  await expect(page.locator(".setup-window")).toBeVisible();
  const setupSections = [
    ["Shows & recovery", "desk-setup-shows-recovery.png"],
    ["Users & sessions", "desk-setup-users.png"],
    ["Programmer", "desk-setup-inputs.png"],
    ["Outputs", "desk-setup-output-engine.png"],
    ["Timecode", "desk-setup-timecode.png"],
    ["Network & Inputs", "desk-setup-network-api.png"],
    ["Screens & playback", "desk-setup-screens.png"],
  ] as const;
  for (const [section, file] of setupSections) {
    await page.locator(".setup-window nav").getByRole("button", { name: section, exact: true }).click();
    await page.locator(".setup-window").screenshot({ path: workflowShot(file) });
  }
  await page.getByRole("button", { name: "Desk Lock", exact: true }).click();
  await page.getByRole("dialog", { name: "Desk Lock" }).screenshot({ path: workflowShot("desk-setup-lock.png") });
  await page.getByRole("button", { name: "Close Desk Lock settings", exact: true }).click();
  await page.locator(".setup-window nav").getByRole("button", { name: "Shows & recovery", exact: true }).click();
  await page.getByRole("button", { name: "Open Fixture Library", exact: true }).click();
  await expect(page.locator(".fixture-library-setup")).toBeVisible();
  await page.getByRole("dialog", { name: "Fixture Library" }).screenshot({ path: workflowShot("fixture-library.png") });
  await page.getByRole("button", { name: "Create fixture", exact: true }).click();
  await page.locator(".fixture-profile-editor-modal").screenshot({ path: workflowShot("fixture-library-create.png") });
  await page.getByRole("tab", { name: "Modes", exact: true }).click();
  await page.getByRole("button", { name: "Edit channels for Default", exact: true }).click();
  const modeEditor = page.getByRole("dialog", { name: "Edit Default mode" });
  await expect(modeEditor).toBeVisible();
  await modeEditor.screenshot({ path: workflowShot("fixture-library-mode-editor.png") });
  await modeEditor.getByRole("button", { name: "Close mode editor", exact: true }).click();
  await page.getByRole("button", { name: "Close fixture editor", exact: true }).click();
  await page.getByRole("button", { name: "Import GDTF", exact: true }).click();
  await page.locator(".gdtf-import-modal").screenshot({ path: workflowShot("fixture-library-import.png") });
  await page.getByRole("button", { name: "Close Import GDTF", exact: true }).click();
  await page.getByRole("button", { name: "Close Fixture Library", exact: true }).click();

  await page.getByRole("button", { name: "BUILT-INS" }).click();
  await page.locator(".dock-entry").filter({ hasText: "Stage" }).click();
  await page.getByRole("button", { name: "Setup positions", exact: true }).click();
  await expect(page.locator(".stage-window")).toBeVisible();
  await page.locator(".stage-window").screenshot({ path: workflowShot("stage-setup-2d.png") });
  await page.locator(".stage-window").getByRole("button", { name: "Settings" }).click();
  await page.getByRole("dialog", { name: "Stage Settings" }).screenshot({ path: workflowShot("stage-settings.png") });
  await page.getByRole("dialog", { name: "Stage Settings" }).getByRole("button", { name: "Close settings" }).click();

  await page.locator(".dock-entry").filter({ hasText: "Fixtures" }).click();
  await page.locator(".fixture-window").getByRole("button", { name: "Settings" }).click();
  const fixtureSettings = page.getByRole("dialog", { name: "Fixture Sheet" });
  await fixtureSettings.screenshot({ path: workflowShot("fixture-sheet-settings-view.png") });
  await fixtureSettings.getByRole("tab", { name: "Columns", exact: true }).click();
  await fixtureSettings.screenshot({ path: workflowShot("fixture-sheet-settings-columns.png") });
  await fixtureSettings.getByRole("button", { name: "Close settings" }).click();

}

async function capturePaneReference(page: Page, selectedDmx: { universe: number; address: number }) {
  await page.getByRole("button", { name: "DESKTOPS" }).click();
  await page.getByRole("button", { name: /New desktop/ }).click();
  await expect(page.locator(".empty-desk")).toBeVisible();
  for (const [, title, slug, settingsTab] of paneReference) {
    await page.locator(".empty-desk").click({ position: { x: 10, y: 10 } });
    await page.getByRole("button", { name: title, exact: true }).click();
    const pane = page.locator(".desk-pane");
    await expect(pane).toBeVisible();
    const gridBox = await page.locator(".desk-grid").boundingBox();
    const resizeBox = await pane.locator(".pane-resize-handle").boundingBox();
    if (!gridBox || !resizeBox) throw new Error(`Cannot resize ${title} for its documentation screenshot`);
    await page.mouse.move(resizeBox.x + resizeBox.width / 2, resizeBox.y + resizeBox.height / 2);
    await page.mouse.down();
    await page.mouse.move(gridBox.x + gridBox.width - 3, gridBox.y + gridBox.height - 3);
    await page.mouse.up();
    await page.waitForTimeout(150);
    if (slug === "text-editor") {
      await pane.getByRole("button", { name: "Open File", exact: true }).click();
      const picker = page.getByRole("dialog", { name: "Choose files or folders" });
      await picker.getByRole("button", { name: `${SCREENSHOT_TEXT_FILE}, file` }).click();
      await picker.getByRole("button", { name: "Select", exact: true }).click();
      await expect(pane.locator(".text-editor-header-state")).toContainText("Saved");
    }
    if (slug === "dmx") {
      await pane.getByRole("button", { name: new RegExp(`^Universe ${selectedDmx.universe}, address ${selectedDmx.address}, value`) }).click();
      await expect(pane.locator(".dmx-fixture-card")).not.toContainText("Fixture: Empty");
    }
    await pane.screenshot({ path: paneShot(`${slug}.png`) });
    await pane.getByRole("button", { name: "Settings" }).click();
    const dialog = page.getByRole("dialog", { name: "Pane Settings" });
    await expect(dialog).toBeVisible();
    if (settingsTab) {
      await dialog.getByRole("tab", { name: settingsTab, exact: true }).click();
      await dialog.screenshot({ path: paneShot(`${slug}-settings.png`) });
      await dialog.getByRole("tab", { name: "Pane Settings", exact: true }).click();
    }
    await dialog.getByRole("button", { name: "Remove pane" }).click();
    await expect(page.locator(".empty-desk")).toBeVisible();
  }
}

function paneShot(file: string): string {
  return path.join(PANE_SCREENSHOT_DIR, file);
}

async function seedScreenshotProgramming(api: ApiDriver, showId: string) {
  const patched = await objects<PatchedFixtureBody>(api, showId, "patched_fixture");
  const byNumber = new Map(patched.flatMap((fixture) => fixture.body.fixture_number == null ? [] : [[fixture.body.fixture_number, fixture.body.fixture_id] as const]));
  const fixtureIds = (...numbers: number[]) => numbers.map((number) => {
    const id = byNumber.get(number);
    if (!id) throw new Error(`Default stage fixture ${number} is missing`);
    return id;
  });
  const groups = [
    ["1", "Front Fresnels", fixtureIds(1, 2, 3, 4, 5, 6), 0.9, 2],
    ["2", "Back Profiles", fixtureIds(101, 102, 103, 104, 105, 106, 107, 108), 0.75, 3],
    ["3", "LED Washes", fixtureIds(201, 202, 203, 204, 205), 0.65, 4],
    ["4", "Floor PARs", fixtureIds(401, 402, 403, 404, 405, 406, 407, 408, 409, 410, 411, 412), 0.8, 5],
    ["5", "RGB Sunstrips", fixtureIds(501, 502, 503, 504, 505, 506), 0.7, 6],
  ] as const;
  for (const [id, name, fixtures, master, playbackFader] of groups) {
    await put(api, showId, "group", id, {
      name,
      fixtures,
      derived_from: null,
      frozen_from: null,
      programming: {},
      master,
      playback_fader: playbackFader,
    });
  }
  await put(api, showId, "preset", "1.1", {
    name: "House Half",
    family: "Intensity",
    number: 1,
    values: {},
    group_values: { "1": { intensity: { kind: "normalized", value: 0.5 } } },
  });
  await put(api, showId, "preset", "2.1", {
    name: "Warm Stage",
    family: "Color",
    number: 1,
    values: {},
    group_values: { "3": { "color.red": { kind: "normalized", value: 1 }, "color.green": { kind: "normalized", value: 0.55 }, "color.blue": { kind: "normalized", value: 0.25 } } },
  });

  const cueListId = crypto.randomUUID();
  await put(api, showId, "cue_list", cueListId, {
    id: cueListId,
    name: "Opening Sequence",
    priority: 0,
    mode: "sequence",
    looped: false,
    chaser_step_millis: 1000,
    speed_group: null,
    cues: [
      cue(1, "Preset", [["1", 0.25], ["2", 0.15], ["3", 0.1]]),
      cue(2, "Front warm", [["1", 0.75], ["2", 0.25], ["4", 0.25]]),
      cue(3, "Back build", [["1", 0.45], ["2", 0.8], ["3", 0.55]]),
      cue(4, "Floor punch", [["2", 0.35], ["4", 0.9], ["5", 0.55]]),
      cue(5, "Sunstrip chase", [["3", 0.6], ["4", 0.45], ["5", 1.0]]),
      cue(6, "Final look", [["1", 0.85], ["2", 0.85], ["3", 0.75], ["4", 0.8], ["5", 0.7]]),
    ],
  });
  await put(api, showId, "playback", "1", playback(1, "Opening Sequence", { type: "cue_list", cue_list_id: cueListId }));
  for (const [id, name] of groups.slice(0, 4).map(([id, name]) => [id, name] as const)) {
    await put(api, showId, "playback", String(Number(id) + 1), playback(Number(id) + 1, name, { type: "group", group_id: id }, ["select", "flash", "select_dereferenced"]));
  }
  await put(api, showId, "playback_page", "1", { number: 1, name: "Main", slots: { "1": 1, "2": 2, "3": 3, "4": 4, "5": 5 } });
  await api.request("POST", "/api/v1/files/shows/operations", { operation: "create_file", sources: [], destination: "", name: SCREENSHOT_TEXT_FILE });
  const empty = await api.request<any>("GET", `/api/v1/files/shows/text?path=${encodeURIComponent(SCREENSHOT_TEXT_FILE)}`);
  await api.request("PUT", "/api/v1/files/shows/text", {
    path: SCREENSHOT_TEXT_FILE,
    text: "# Run of show\n\n- 18:45 House open\n- 19:25 Beginners\n- 19:30 Opening Sequence\n\n## Notes\n\nCheck follow spots before preset.\n",
    revision: empty.revision,
  });
}

function cue(number: number, name: string, groupLevels: Array<[string, number]>) {
  return {
    number,
    name,
    changes: [],
    fade_millis: 1500,
    delay_millis: 0,
    trigger: { type: "manual" },
    phasers: [],
    group_changes: groupLevels.map(([group_id, value]) => ({
      group_id,
      attribute: "intensity",
      value: { kind: "normalized", value },
      fade_millis: 1500,
    })),
  };
}

function playback(number: number, name: string, target: Record<string, unknown>, buttons: [string, string, string] = ["go", "go_minus", "flash"]) {
  return { number, name, target, buttons, fader: "master", go_activates: true, auto_off: true, xfade_millis: 0 };
}

async function objects<T>(api: ApiDriver, showId: string, kind: string): Promise<Array<VersionedObject<T>>> {
  return api.request<Array<VersionedObject<T>>>("GET", `/api/v1/shows/${showId}/objects/${kind}`, undefined, false);
}

async function put(api: ApiDriver, showId: string, kind: string, id: string, body: unknown) {
  await api.request("PUT", `/api/v1/shows/${showId}/objects/${kind}/${id}`, body, true, 0);
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

async function openCuelistDetailWithPlayback(page: Page) {
  await page.getByRole("button", { name: "BUILT-INS" }).click();
  await page.locator(".dock-entry").filter({ hasText: "Cuelists" }).click();
  const firstCuelist = page.locator(".cuelist-card").first();
  await firstCuelist.click();
  await expect(page.locator(".cue-table tbody tr")).toHaveCount(6);
  await page.locator(".mode-toggle").click();
  await expect(page.locator(".control-section.playbacks")).toBeVisible();
}
