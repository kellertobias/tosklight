import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import type { ApiDriver } from "../apps/control-ui/e2e/bench/api";
import type { Page } from "@playwright/test";

test.describe.configure({ mode: "serial" });
test.setTimeout(60_000);
test.use({ viewport: { width: 1600, height: 1100 } });

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const SCREENSHOT_DIR = path.join(ROOT, "docs/help/assets/screenshots");

interface ShowEntry { id: string; name: string }
interface VersionedObject<T = Record<string, unknown>> { id: string; revision: number; body: T }
interface PatchedFixtureBody { fixture_id: string; fixture_number?: number; logical_heads?: Array<{ fixture_id: string }> }

test("captures help and README screenshots from the default show desk", async ({ page, desk, api }) => {
  await fs.mkdir(SCREENSHOT_DIR, { recursive: true });
  await openSeededDefaultStageShow(api);

  await desk.open(api.baseUrl);
  await page.getByRole("button", { name: "DESKS" }).click();
  await page.getByRole("button", { name: /Programming/ }).click();
  await selectFixtures(page, desk, "1 + 2 + 3 + 4 + 5 + 6");
  await setDimmerByTouch(page, 50);
  await setStagePaneTo3d(page);
  await expect(page.locator(".group-strip .group-card").filter({ hasText: "Front Fresnels" })).toBeVisible();
  await expect(page.locator(".control-section")).toContainText("50%");
  await expect(page.locator("canvas")).toBeVisible();
  await page.waitForTimeout(500);
  await page.screenshot({ path: shot("default-desk-overview.png"), fullPage: true });

  await openCuelistDetailWithPlayback(page);
  await expect(page.locator(".control-section.playbacks")).toBeVisible();
  await expect(page.locator(".cue-table tbody tr")).toHaveCount(6);
  await expect(page.locator(".playback-fader-bank")).toContainText("Front Fresnels");
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
    await put(api, showId, "playback", String(Number(id) + 1), playback(Number(id) + 1, name, { type: "group", group_id: id }, ["on", "off", "flash"]));
  }
  await put(api, showId, "playback_page", "1", { number: 1, name: "Main", slots: { "1": 1, "2": 2, "3": 3, "4": 4, "5": 5 } });
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
