import type { Locator, Page } from "../apps/control-ui/node_modules/@playwright/test/index.js";
import type { ApiDriver } from "../apps/control-ui/e2e/bench/api";
import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import { pairedScenario } from "../apps/control-ui/e2e/bench/pairedScenario";
import {
  activeShowId,
  fixtureIdsByNumber,
  loadCanonicalCopy,
  object,
  objects,
  programmer,
  putObject,
} from "./support/catalog";

type PlaybackTarget =
  | { type: "cue_list"; cue_list_id: string }
  | { type: "group"; group_id: string }
  | { type: "speed_group"; group: string }
  | { type: "programmer_fade" }
  | { type: "cue_fade" }
  | { type: "grand_master" };

interface PlaybackDefinition {
  number: number;
  name: string;
  target: PlaybackTarget;
  buttons: [string, string, string];
  button_count: number;
  fader: string;
  has_fader: boolean;
  go_activates: boolean;
  auto_off: boolean;
  xfade_millis: number;
  color: string;
  flash_release: string;
  protect_from_swap: boolean;
  presentation_icon?: string;
  presentation_image?: string;
}

interface PreparedShow {
  showId: string;
  cueListId: string;
  fixtures: Record<number, string>;
}

type PlaybackConfigurationObservation = {
  page: number;
  slot: number;
  number: number;
  targetType: PlaybackTarget["type"];
  targetMatchesExpected: boolean;
  buttons: string[];
  buttonCount: number;
  fader: string;
  hasFader: boolean;
  color: string;
};

type Pbk001State = PreparedShow & {
  before: Awaited<ReturnType<typeof inertSnapshot>>;
  inspected?: PlaybackConfigurationObservation;
};

type Pbk002State = PreparedShow & {
  assigned?: PlaybackConfigurationObservation;
};

type Pbk003State = PreparedShow & {
  runtimeBeforeSelect?: any;
  dmxBeforeSelect?: number[];
};

type Pbk004State = PreparedShow & {
  timings: string;
  checkpoints: Array<{ cue: number; position: number; progress: number; direction: string; intensity: number }>;
};

type Pbk005State = PreparedShow & {
  permanentBefore: Record<number, any>;
  levelsBefore: Record<number, number>;
  observations: {
    tempDuring?: boolean;
    tempAfter?: boolean;
    swapDuring?: boolean;
    swapAfter?: boolean;
    tempLevels?: Record<number, number>;
    swapLevels?: Record<number, number>;
  };
};

test.describe("docs/testing/07-playback-configuration.md", () => {
  pairedScenario<Pbk001State>({
    id: "PBK-001",
    title: "Set inspection resolves one playback identity and Close is mutation-free",
    arrange: async ({ api, bench }, surface) => {
      const prepared = await prepareShow(api, bench, `pbk-001-paired-${surface}`, "compact-rig");
      await installPlaybacks(api, [definition(40, "Configured Sequence", { type: "cue_list", cue_list_id: prepared.cueListId })], { 1: 40 });
      await poolAction(api, 40, "go");
      await poolAction(api, 40, "master", { value: 0.6 });
      return { ...prepared, before: await inertSnapshot(api, 40) };
    },
    api: async ({ api }, state) => {
      await api.request("GET", "/api/v1/playback-pool/40");
      state.inspected = await playbackConfigurationObservation(api, 1, 1, state.cueListId);
    },
    ui: async ({ api, bench, desk, page }, state) => {
      await desk.open(bench.baseUrl);
      await openPlaybackMode(page);
      state.before = await inertSnapshot(api, 40);
      await armSet(page);
      await page.getByRole("button", { name: "Playback representation page 1 playback 1" }).click();
      const modal = await expectConfigurationModal(page, 1, 1);
      await expect(modal.getByRole("button", { name: "Function", exact: true })).toBeVisible();
      await expect(modal.getByRole("button", { name: "Behavior", exact: true })).toBeVisible();
      await expect(modal.getByRole("button", { name: "Layout", exact: true })).toBeVisible();
      state.inspected = await playbackConfigurationObservation(api, 1, 1, state.cueListId);
      await modal.getByRole("button", { name: "Close playback configuration", exact: true }).click();
      await expect(modal).toBeHidden();
    },
    assert: async ({ api }, state) => {
      expect(state.inspected).toEqual({
        page: 1,
        slot: 1,
        number: 40,
        targetType: "cue_list",
        targetMatchesExpected: true,
        buttons: ["go_minus", "go", "flash"],
        buttonCount: 3,
        fader: "master",
        hasFader: true,
        color: "#20c997",
      });
      expect(await inertSnapshot(api, 40)).toEqual(state.before);
    },
  });

  test("PBK-001 @supplemental › direct and legacy read APIs preserve page/slot state", async ({ api, bench }) => {
    const prepared = await prepareShow(api, bench, "pbk-001-api", "compact-rig");
    await installPlaybacks(api, [definition(40, "API Identity", { type: "cue_list", cue_list_id: prepared.cueListId })], { 1: 40 });
    await poolAction(api, 40, "go");
    await poolAction(api, 40, "master", { value: 0.6 });
    const before = await inertSnapshot(api, 40);
    const snapshot = await playbackSnapshot(api);
    expect(snapshot.pages.find((candidate: any) => candidate.number === 1)?.slots["1"]).toBe(40);
    expect(snapshot.pool.find((candidate: any) => candidate.number === 40)).toEqual(before.object.body);
    const direct = await api.request<any>("GET", "/api/v1/playback-pool/40");
    const legacyAlias = await api.request<any>("GET", "/api/v1/cuelists/40");
    expect(direct.playback).toEqual(before.object.body);
    expect(legacyAlias.playback).toEqual(before.object.body);
    expect(await inertSnapshot(api, 40)).toEqual(before);
  });

  test("PBK-001 @supplemental-ui › SET intercepts every physical control without operating it", async ({ api, bench, desk, page }) => {
    const prepared = await prepareShow(api, bench, "pbk-001-physical", "compact-rig");
    const playback = definition(41, "Configured Sequence", { type: "cue_list", cue_list_id: prepared.cueListId });
    await installPlaybacks(api, [playback], { 1: 41 });
    await poolAction(api, 41, "go");
    await poolAction(api, 41, "master", { value: 0.6 });

    await desk.open(bench.baseUrl);
    await openPlaybackMode(page);
    const before = await inertSnapshot(api, 41);
    const surfaces: Array<[string, () => Locator]> = [
      ["top button", () => playbackCard(page, 1).getByRole("button", { name: "GO −", exact: true })],
      ["middle button", () => playbackCard(page, 1).getByRole("button", { name: "GO +", exact: true })],
      ["bottom button", () => playbackCard(page, 1).getByRole("button", { name: "FLASH", exact: true })],
      ["fader track and handle", () => playbackCard(page, 1).getByRole("slider", { name: "Master" })],
      ["software representation", () => page.getByRole("button", { name: "Playback representation page 1 playback 1" })],
    ];

    for (const [surface, target] of surfaces) {
      await test.step(`SET then ${surface}`, async () => {
        await armSet(page);
        await target().click();
        const modal = await expectConfigurationModal(page, 1, 1);
        await expect(modal.getByRole("button", { name: "Function", exact: true })).toBeVisible();
        await expect(modal.getByRole("button", { name: "Behavior", exact: true })).toBeVisible();
        await expect(modal.getByRole("button", { name: "Layout", exact: true })).toBeVisible();
        await modal.getByRole("button", { name: "Close playback configuration", exact: true }).click();
        await expect(modal).toBeHidden();
        expect(await inertSnapshot(api, 41)).toEqual(before);
      });
    }

    await armSet(page);
    await page.getByRole("button", { name: "Playback representation page 1 playback 2" }).click();
    const empty = await expectConfigurationModal(page, 1, 2);
    await expect(empty.getByRole("radio", { name: "None" })).toBeVisible();
    await empty.getByRole("button", { name: "Close playback configuration", exact: true }).click();
    expect((await pageObject(api, 1)).body.slots["2"]).toBeUndefined();
    expect(await inertSnapshot(api, 41)).toEqual(before);
  });

  test("PBK-001 @supplemental-ui › Virtual cells share the modal with one-button topology and presentation", async ({ api, bench, desk, page }) => {
    const prepared = await prepareShow(api, bench, "pbk-001-virtual", "compact-rig");
    await installPlaybacks(api, [definition(42, "Virtual Sequence", { type: "cue_list", cue_list_id: prepared.cueListId }, {
      buttons: ["toggle", "none", "none"], button_count: 1, has_fader: false, presentation_icon: "▶",
    })], { 1: 42 });
    await desk.open(bench.baseUrl);
    const pane = await addVirtualPlaybackPane(page);
    await expect(pane.getByRole("button", { name: "Set Source", exact: true })).toBeVisible();
    await expect(pane.getByRole("button", { name: "Add Target", exact: true })).toBeVisible();

    await pane.getByRole("button", { name: "Settings", exact: true }).click();
    const settings = page.getByRole("dialog", { name: "Pane Settings" });
    await settings.getByRole("tab", { name: "Virtual Playbacks", exact: true }).click();
    await expect(settings.getByLabel("Rows")).toBeVisible();
    await expect(settings.getByLabel("Columns")).toBeVisible();
    await expect(settings.getByText(/Cuelist assignment|Action assignment/i)).toHaveCount(0);
    await settings.getByRole("button", { name: "Close settings" }).click();

    const before = await inertSnapshot(api, 42);
    await armSet(page);
    await pane.getByRole("button", { name: /Virtual playback page 1 cell 1 Virtual Sequence/ }).click();
    let modal = await expectConfigurationModal(page, 1, 1);
    await expect(modal).toHaveAttribute("data-topology", "1 button · faderless");
    await expect(selectTrigger(modal, "Presentation")).toBeVisible();
    await chooseSelect(page, modal, "Presentation", "Image background");
    await expect(modal.getByLabel("Image background")).toBeVisible();
    await modal.getByRole("button", { name: "Layout", exact: true }).click();
    await expect(selectTrigger(modal, "Top button")).toBeVisible();
    await expect(selectTrigger(modal, "Middle button")).toHaveCount(0);
    await expect(modal.getByText("No fader on this playback.", { exact: true })).toBeVisible();
    await modal.getByRole("button", { name: "Close playback configuration", exact: true }).click();
    expect(await inertSnapshot(api, 42)).toEqual(before);

    await armSet(page);
    await pane.getByRole("button", { name: /Virtual playback page 1 cell 2 empty/ }).click();
    modal = await expectConfigurationModal(page, 1, 2);
    await expect(modal).toHaveAttribute("data-topology", "1 button · faderless");
    await expect(selectTrigger(modal, "Presentation")).toBeVisible();
    await modal.getByRole("button", { name: "Close playback configuration", exact: true }).click();
    expect((await pageObject(api, 1)).body.slots["2"]).toBeUndefined();
  });

  pairedScenario<Pbk002State>({
    id: "PBK-002",
    title: "Cue List assignment, color, and None plus Apply clear are atomic",
    arrange: async ({ api, bench }, surface) => {
      const prepared = await prepareShow(api, bench, `pbk-002-paired-${surface}`, "default-stage");
      await writePage(api, 1, {});
      return prepared;
    },
    api: async ({ api }, state) => {
      await saveSlot(api, 1, 1, definition(0, "Playback 1.1", { type: "cue_list", cue_list_id: state.cueListId }, { color: "#8b5cf6" }));
      state.assigned = await playbackConfigurationObservation(api, 1, 1, state.cueListId);
      await clearSlot(api, 1, 1);
    },
    ui: async ({ api, bench, desk, page }, state) => {
      await desk.open(bench.baseUrl);
      await openPlaybackMode(page);
      await armSet(page);
      await page.getByRole("button", { name: "Playback representation page 1 playback 1" }).click();
      let modal = await expectConfigurationModal(page, 1, 1);
      await modal.getByRole("radio", { name: "Configured Sequence", exact: true }).click();
      await choosePlaybackColor(page, modal, "#8b5cf6");
      await modal.getByRole("button", { name: "Apply", exact: true }).click();
      await expect(modal).toBeHidden();
      state.assigned = await playbackConfigurationObservation(api, 1, 1, state.cueListId);
      await armSet(page);
      await page.getByRole("button", { name: "Playback representation page 1 playback 1" }).click();
      modal = await expectConfigurationModal(page, 1, 1);
      await modal.getByRole("radio", { name: "None", exact: true }).click();
      await expect(modal.getByText("Playback will be cleared", { exact: true })).toBeVisible();
      await modal.getByRole("button", { name: "Apply", exact: true }).click();
      await expect(modal).toBeHidden();
    },
    assert: async ({ api }, state) => {
      expect(state.assigned).toMatchObject({
        page: 1,
        slot: 1,
        targetType: "cue_list",
        targetMatchesExpected: true,
        buttons: ["go_minus", "go", "flash"],
        buttonCount: 3,
        fader: "master",
        hasFader: true,
        color: "#8b5cf6",
      });
      expect((await pageObject(api, 1)).body.slots["1"]).toBeUndefined();
      expect((await objects(api, "cue_list")).some((item) => item.id === state.cueListId)).toBe(true);
    },
  });

  test("PBK-002 @supplemental › every function, topology, migration, conflict, and reload path is atomic", async ({ api, bench }) => {
    const prepared = await prepareShow(api, bench, "pbk-002-functions", "default-stage");
    await writePage(api, 1, {});
    const assignments: Array<{ slot: number; target: PlaybackTarget; buttons: [string, string, string]; count: number; fader: string; hasFader: boolean }> = [
      { slot: 1, target: { type: "cue_list", cue_list_id: prepared.cueListId }, buttons: ["go_minus", "go", "flash"], count: 1, fader: "master", hasFader: false },
      { slot: 2, target: { type: "group", group_id: "1" }, buttons: ["select", "select_dereferenced", "flash"], count: 2, fader: "master", hasFader: true },
      ...["A", "B", "C", "D", "E"].map((group, index) => ({ slot: index + 3, target: { type: "speed_group", group } as PlaybackTarget, buttons: ["double", "half", "learn"] as [string, string, string], count: 3, fader: "learned_percentage", hasFader: true })),
      { slot: 8, target: { type: "programmer_fade" }, buttons: ["double", "half", "off"], count: 0, fader: "master", hasFader: true },
      { slot: 9, target: { type: "cue_fade" }, buttons: ["double", "half", "off"], count: 3, fader: "master", hasFader: true },
      { slot: 10, target: { type: "grand_master" }, buttons: ["blackout", "pause_dynamics", "flash"], count: 3, fader: "master", hasFader: true },
    ];

    for (const assignment of assignments) {
      const result = await saveSlot(api, 1, assignment.slot, definition(0, `Function ${assignment.slot}`, assignment.target, {
        buttons: assignment.buttons.map((action, index) => index < assignment.count ? action : "none") as [string, string, string],
        button_count: assignment.count,
        fader: assignment.fader,
        has_fader: assignment.hasFader,
        color: "#8b5cf6",
      }));
      expect(result.playback).toMatchObject({
        target: assignment.target,
        buttons: assignment.buttons.map((action, index) => index < assignment.count ? action : "none"),
        button_count: assignment.count,
        fader: assignment.fader,
        has_fader: assignment.hasFader,
        color: "#8b5cf6",
      });
    }

    const first = await playbackAt(api, 1, 1);
    const firstPage = await pageObject(api, 1);
    const staleResponse = await fetch(`${api.baseUrl}/api/v1/playback-pages/1/slots/1`, {
      method: "PUT",
      headers: { authorization: `Bearer ${api.session!.token}`, "content-type": "application/json" },
      body: JSON.stringify({
        playback: { ...first.body, name: "Must not land" },
        expected_playback_revision: first.revision - 1,
        expected_page_revision: firstPage.revision - 1,
      }),
    });
    expect(staleResponse.status).toBe(409);
    expect((await playbackAt(api, 1, 1)).body.name).toBe("Function 1");
    expect((await pageObject(api, 1)).body).toEqual(firstPage.body);

    const invalidResponse = await fetch(`${api.baseUrl}/api/v1/playback-pages/1/slots/1`, {
      method: "PUT",
      headers: { authorization: `Bearer ${api.session!.token}`, "content-type": "application/json" },
      body: JSON.stringify({
        playback: { ...first.body, target: { type: "group", group_id: "1" }, buttons: ["go", "go_minus", "flash"] },
        expected_playback_revision: first.revision,
        expected_page_revision: firstPage.revision,
      }),
    });
    expect(invalidResponse.status).toBe(400);
    expect((await playbackAt(api, 1, 1)).body).toEqual(first.body);

    await putObject(api, "playback", "700", {
      number: 700,
      name: "Legacy Playback",
      target: { type: "cue_list", cue_list_id: prepared.cueListId },
      buttons: ["go_minus", "go", "flash"],
      fader: "master",
      go_activates: true,
      auto_off: true,
      xfade_millis: 0,
    });
    const pageWithLegacy = await pageObject(api, 1);
    await putObject(api, "playback_page", "1", { ...pageWithLegacy.body, slots: { ...pageWithLegacy.body.slots, "11": 700 } }, pageWithLegacy.revision);
    const migrated = (await playbackSnapshot(api)).pool.find((item: any) => item.number === 700);
    expect(migrated).toMatchObject({ button_count: 3, has_fader: true, color: "#20c997", flash_release: "release_all", protect_from_swap: false });
    await saveSlot(api, 1, 11, migrated);
    expect((await playbackAt(api, 1, 11)).body).toMatchObject({ button_count: 3, has_fader: true, color: "#20c997" });

    await api.request("POST", `/api/v1/shows/${prepared.showId}/open`, { transition: "hold_current" });
    for (const assignment of assignments) {
      expect((await playbackAt(api, 1, assignment.slot)).body).toMatchObject({ target: assignment.target, color: "#8b5cf6" });
    }

    const sourceBefore = await object<any>(api, "cue_list", prepared.cueListId);
    const assigned = await playbackAt(api, 1, 1);
    await poolAction(api, assigned.body.number, "on");
    await clearSlot(api, 1, 1);
    expect((await pageObject(api, 1)).body.slots["1"]).toBeUndefined();
    expect((await objects(api, "playback")).some((item) => item.id === String(assigned.body.number))).toBe(false);
    expect((await playbackSnapshot(api)).active.some((item: any) => item.playback_number === assigned.body.number)).toBe(false);
    expect(await object<any>(api, "cue_list", prepared.cueListId)).toEqual(sourceBefore);
  });

  test("PBK-002 @supplemental-ui › grouped functions reset layout defaults and None plus Apply is explicit", async ({ api, bench, desk, page }) => {
    const prepared = await prepareShow(api, bench, "pbk-002-ui", "default-stage");
    await writePage(api, 1, {});
    await desk.open(bench.baseUrl);
    await openPlaybackMode(page);
    await armSet(page);
    await page.getByRole("button", { name: "Playback representation page 1 playback 1" }).click();
    let modal = await expectConfigurationModal(page, 1, 1);
    await expect(modal.getByRole("button", { name: "Apply", exact: true })).toBeDisabled();
    await modal.getByRole("radio", { name: "Group Master", exact: true }).click();
    await expect(modal.getByRole("button", { name: "Apply", exact: true })).toBeEnabled();
    await modal.getByRole("button", { name: "Layout", exact: true }).click();
    await expect(selectTrigger(modal, "Top button")).toContainText("Select");
    await expect(selectTrigger(modal, "Middle button")).toContainText("Select dereferenced");
    await expect(selectTrigger(modal, "Bottom button")).toContainText("Flash");
    await expect(selectTrigger(modal, "Fader")).toContainText("Group intensity master");
    await modal.getByRole("button", { name: "Function", exact: true }).click();
    await modal.getByRole("radio", { name: "Speed Master", exact: true }).click();
    await modal.getByRole("button", { name: "Layout", exact: true }).click();
    await expect(selectTrigger(modal, "Top button")).toContainText("Double");
    await expect(selectTrigger(modal, "Middle button")).toContainText("Half");
    await expect(selectTrigger(modal, "Bottom button")).toContainText("Learn");
    await expect(selectTrigger(modal, "Fader")).toContainText("Learned-speed percentage");
    await modal.getByRole("button", { name: "Function", exact: true }).click();
    await modal.getByRole("radio", { name: "Cue List", exact: true }).click();
    await modal.getByRole("button", { name: "Layout", exact: true }).click();
    await expect(selectTrigger(modal, "Top button")).toContainText("GO −");
    await expect(selectTrigger(modal, "Middle button")).toContainText("GO +");
    await expect(selectTrigger(modal, "Bottom button")).toContainText("Flash");
    await modal.getByRole("button", { name: "Function", exact: true }).click();
    await choosePlaybackColor(page, modal, "#8b5cf6");
    await modal.getByRole("button", { name: "Behavior", exact: true }).click();
    await expect(modal.getByRole("radiogroup", { name: "When Flash or Swap is released", exact: true })).toBeVisible();
    await expect(modal.getByText(/leaves this Cue List active at zero intensity/)).toBeVisible();
    await expect(modal.getByRole("switch", { name: "Turn off when other playbacks take full control", exact: true })).toBeVisible();
    await modal.getByRole("radio", { name: "Intensity only", exact: true }).click();
    await modal.getByRole("switch", { name: "Protect from Swap", exact: true }).locator("..").click();
    await modal.getByRole("button", { name: "Apply", exact: true }).click();
    await expect(modal).toBeHidden();

    let stored = await playbackAt(api, 1, 1);
    expect(stored.body).toMatchObject({ target: { type: "cue_list", cue_list_id: prepared.cueListId }, buttons: ["go_minus", "go", "flash"], color: "#8b5cf6", flash_release: "release_intensity_only", protect_from_swap: true });
    await expect(playbackCard(page, 1)).toHaveCSS("--playback-color", "#8b5cf6");
    await page.reload();
    await openPlaybackMode(page);
    await expect(playbackCard(page, 1)).toHaveCSS("--playback-color", "#8b5cf6");

    await armSet(page);
    await page.getByRole("button", { name: "Playback representation page 1 playback 1" }).click();
    modal = await expectConfigurationModal(page, 1, 1);
    await modal.getByRole("radio", { name: "None", exact: true }).click();
    await modal.getByRole("button", { name: "Close playback configuration", exact: true }).click();
    stored = await playbackAt(api, 1, 1);
    expect(stored.body.color).toBe("#8b5cf6");
    await armSet(page);
    await page.getByRole("button", { name: "Playback representation page 1 playback 1" }).click();
    modal = await expectConfigurationModal(page, 1, 1);
    await modal.getByRole("radio", { name: "None", exact: true }).click();
    await modal.getByRole("button", { name: "Apply", exact: true }).click();
    await expect(modal).toBeHidden();
    expect((await pageObject(api, 1)).body.slots["1"]).toBeUndefined();
    expect((await objects(api, "cue_list")).some((item) => item.id === prepared.cueListId)).toBe(true);
  });

  pairedScenario<Pbk003State>({
    id: "PBK-003",
    title: "default navigation and remapped Select Contents dispatch one exact action",
    arrange: async ({ api, bench }, surface) => {
      const prepared = await prepareShow(api, bench, `pbk-003-paired-${surface}`, "compact-rig");
      await installPlaybacks(api, [definition(43, "Action Matrix", { type: "cue_list", cue_list_id: prepared.cueListId })], { 1: 43 });
      return prepared;
    },
    api: async ({ api }, state) => {
      await pressButton(api, 43, 2);
      await pressButton(api, 43, 2);
      await pressButton(api, 43, 1);
      await setFirstButton(api, 1, "select_contents");
      state.runtimeBeforeSelect = await activePlayback(api, 43);
      state.dmxBeforeSelect = await logicalDmx(api);
      await pressButton(api, 43, 1);
    },
    ui: async ({ api, bench, desk, page }, state) => {
      await desk.open(bench.baseUrl);
      await openPlaybackMode(page);
      const card = playbackCard(page, 1);
      await card.getByRole("button", { name: "GO +", exact: true }).click();
      await expect.poll(async () => (await activePlayback(api, 43)).current_cue_number).toBe(1);
      await card.getByRole("button", { name: "GO +", exact: true }).click();
      await expect.poll(async () => (await activePlayback(api, 43)).current_cue_number).toBe(2);
      await card.getByRole("button", { name: "GO −", exact: true }).click();
      await expect.poll(async () => (await activePlayback(api, 43)).current_cue_number).toBe(1);
      await armSet(page);
      await card.getByRole("button", { name: "GO −", exact: true }).click();
      const modal = await expectConfigurationModal(page, 1, 1);
      await modal.getByRole("button", { name: "Layout", exact: true }).click();
      await chooseSelect(page, modal, "Top button", "Select contents");
      await modal.getByRole("button", { name: "Apply", exact: true }).click();
      await expect(modal).toBeHidden();
      state.runtimeBeforeSelect = await activePlayback(api, 43);
      state.dmxBeforeSelect = await logicalDmx(api);
      await card.getByRole("button", { name: "SELECT CONTENTS", exact: true }).click();
      await expect.poll(async () => (await programmer(api)).selection_expression?.type).toBe("playback_contents");
    },
    assert: async ({ api }, state) => {
      expect((await playbackAt(api, 1, 1)).body.buttons).toEqual(["select_contents", "go", "flash"]);
      expect(await activePlayback(api, 43)).toEqual(state.runtimeBeforeSelect);
      expect(await logicalDmx(api)).toEqual(state.dmxBeforeSelect);
      const selected = await programmer(api);
      expect(selected.selection_expression).toEqual({
        type: "playback_contents",
        items: [
          { type: "fixture", fixture_id: state.fixtures[1] },
          { type: "fixture", fixture_id: state.fixtures[2] },
          { type: "live_group", group_id: "3" },
        ],
      });
      expect(selected.values).toEqual([]);
    },
  });

  test("PBK-003 @supplemental › every Cuelist mapping preserves its distinct action semantics", async ({ api, bench }) => {
    const prepared = await prepareShow(api, bench, "pbk-003-actions", "compact-rig", [0.2, 0.8, 0.4], 10_000, 5_000);
    await installPlaybacks(api, [definition(43, "Action Matrix", { type: "cue_list", cue_list_id: prepared.cueListId })], { 1: 43 });
    const timingBefore = (await object<any>(api, "cue_list", prepared.cueListId)).body.cues.map((cue: any) => ({ fade_millis: cue.fade_millis, delay_millis: cue.delay_millis }));

    await setFirstButton(api, 1, "go");
    await pressButton(api, 43);
    expect((await activePlayback(api, 43)).current_cue_number).toBe(1);
    await pressButton(api, 43);
    expect((await activePlayback(api, 43)).current_cue_number).toBe(2);
    await setFirstButton(api, 1, "go_minus");
    await pressButton(api, 43);
    expect((await activePlayback(api, 43)).current_cue_number).toBe(1);

    await setFirstButton(api, 1, "fast_forward");
    await pressButton(api, 43);
    expect(await activePlayback(api, 43)).toMatchObject({ current_cue_number: 2, transition_timing_bypassed: true });
    await setFirstButton(api, 1, "fast_rewind");
    await pressButton(api, 43);
    expect(await activePlayback(api, 43)).toMatchObject({ current_cue_number: 1, transition_timing_bypassed: true });
    expect((await object<any>(api, "cue_list", prepared.cueListId)).body.cues.map((cue: any) => ({ fade_millis: cue.fade_millis, delay_millis: cue.delay_millis }))).toEqual(timingBefore);

    await poolAction(api, 43, "master", { value: 0.25 });
    await setFirstButton(api, 1, "on");
    await pressButton(api, 43);
    expect(await activePlayback(api, 43)).toMatchObject({ enabled: true, master: 1, fader_position: 0.25 });
    await setFirstButton(api, 1, "off");
    await pressButton(api, 43);
    expect(await activePlayback(api, 43)).toMatchObject({ enabled: false, fader_position: 0.25, fader_pickup_required: true });
    await poolAction(api, 43, "master", { value: 0.8 });
    expect(await activePlayback(api, 43)).toMatchObject({ enabled: false, fader_pickup_required: true });
    await poolAction(api, 43, "master", { value: 0 });
    await poolAction(api, 43, "master", { value: 0.6 });
    const recovered = await activePlayback(api, 43);
    expect(recovered).toMatchObject({ enabled: true, fader_pickup_required: false });
    expect(recovered.master).toBeCloseTo(0.6, 5);
    expect(recovered.fader_position).toBeCloseTo(0.6, 5);

    await setFirstButton(api, 1, "toggle");
    await pressButton(api, 43);
    expect((await activePlayback(api, 43)).enabled).toBe(false);
    await pressButton(api, 43);
    expect((await activePlayback(api, 43)).enabled).toBe(true);

    const beforeSelect = await activePlayback(api, 43);
    const dmxBeforeSelect = await logicalDmx(api);
    await setFirstButton(api, 1, "select");
    await pressButton(api, 43);
    expect((await playbackSnapshot(api)).selected_playback).toBe(43);
    expect(await activePlayback(api, 43)).toEqual(beforeSelect);
    expect(await logicalDmx(api)).toEqual(dmxBeforeSelect);

    await setFirstButton(api, 1, "select_contents");
    await pressButton(api, 43);
    const selected = await programmer(api);
    expect(selected.selection_expression).toMatchObject({
      type: "playback_contents",
      items: [
        { type: "fixture", fixture_id: prepared.fixtures[1] },
        { type: "fixture", fixture_id: prepared.fixtures[2] },
        { type: "live_group", group_id: "3" },
      ],
    });
    expect(selected.values).toHaveLength(0);
    expect((await activePlayback(api, 43)).current_cue_number).toBe(beforeSelect.current_cue_number);
  });

  test("PBK-003 @supplemental-ui › physical controls expose default and remapped action feedback", async ({ api, bench, desk, page }) => {
    const prepared = await prepareShow(api, bench, "pbk-003-ui", "compact-rig");
    await installPlaybacks(api, [definition(44, "UI Actions", { type: "cue_list", cue_list_id: prepared.cueListId })], { 1: 44 });
    await desk.open(bench.baseUrl);
    await openPlaybackMode(page);
    const card = playbackCard(page, 1);
    await expect(card.getByRole("button", { name: "GO −", exact: true })).toBeVisible();
    await expect(card.getByRole("button", { name: "GO +", exact: true })).toBeVisible();
    await expect(card.getByRole("button", { name: "FLASH", exact: true })).toBeVisible();
    await card.getByRole("button", { name: "GO +", exact: true }).click();
    await expect.poll(async () => (await activePlayback(api, 44)).current_cue_number).toBe(1);
    await card.getByRole("button", { name: "GO +", exact: true }).click();
    await expect.poll(async () => (await activePlayback(api, 44)).current_cue_number).toBe(2);
    await card.getByRole("button", { name: "GO −", exact: true }).click();
    await expect.poll(async () => (await activePlayback(api, 44)).current_cue_number).toBe(1);

    await armSet(page);
    await card.getByRole("button", { name: "GO −", exact: true }).click();
    const modal = await expectConfigurationModal(page, 1, 1);
    await modal.getByRole("button", { name: "Layout", exact: true }).click();
    await chooseSelect(page, modal, "Top button", "Select contents");
    await modal.getByRole("button", { name: "Apply", exact: true }).click();
    await expect(modal).toBeHidden();
    const runtimeBefore = await activePlayback(api, 44);
    await card.getByRole("button", { name: "SELECT CONTENTS", exact: true }).click();
    await expect.poll(async () => (await programmer(api)).selection_expression?.type).toBe("playback_contents");
    expect(await activePlayback(api, 44)).toEqual(runtimeBefore);
  });

  pairedScenario<Pbk004State>({
    id: "PBK-004",
    title: "X-fade travel advances one Cue and preserves manual direction and timing",
    arrange: async ({ api, bench }, surface) => {
      const prepared = await prepareShow(api, bench, `pbk-004-paired-${surface}`, "compact-rig", [0, 1, 0.5], 30_000, 10_000);
      await installPlaybacks(api, [definition(47, "Manual Crossfade", { type: "cue_list", cue_list_id: prepared.cueListId }, { fader: "x_fade" })], { 1: 47 });
      await poolAction(api, 47, "on");
      return {
        ...prepared,
        timings: serializedCueTimings(await object<any>(api, "cue_list", prepared.cueListId)),
        checkpoints: [],
      };
    },
    api: async ({ api, bench }, state) => {
      await poolAction(api, 47, "master", { value: 0.25 });
      await bench.tick(0);
      state.checkpoints.push(xfadeObservation(await activePlayback(api, 47), await visualizationLevel(api, state.fixtures[1])));
      await poolAction(api, 47, "master", { value: 1 });
      await bench.tick(0);
      state.checkpoints.push(xfadeObservation(await activePlayback(api, 47), await visualizationLevel(api, state.fixtures[1])));
    },
    ui: async ({ api, bench, desk, page }, state) => {
      await desk.open(bench.baseUrl);
      await openPlaybackMode(page);
      const slider = playbackSlider(page, 1);
      await slider.fill("25");
      await expect.poll(async () => (await activePlayback(api, 47)).manual_xfade_progress).toBeCloseTo(0.25, 3);
      await bench.tick(0);
      state.checkpoints.push(xfadeObservation(await activePlayback(api, 47), await visualizationLevel(api, state.fixtures[1])));
      await slider.fill("100");
      await expect.poll(async () => (await activePlayback(api, 47)).current_cue_number).toBe(2);
      await bench.tick(0);
      state.checkpoints.push(xfadeObservation(await activePlayback(api, 47), await visualizationLevel(api, state.fixtures[1])));
    },
    assert: async ({ api }, state) => {
      expect(state.checkpoints[0]).toEqual({ cue: 1, position: 0.25, progress: 0.25, direction: "towards_high", intensity: 0.25 });
      expect(state.checkpoints[1]).toMatchObject({ cue: 2, position: 1, direction: "towards_low", intensity: 1 });
      expect(await activePlayback(api, 47)).toMatchObject({ current_cue_number: 2, manual_xfade_position: 1, manual_xfade_direction: "towards_low" });
      expect((await playbackAt(api, 1, 1)).body.fader).toBe("x_fade");
      expect(serializedCueTimings(await object<any>(api, "cue_list", state.cueListId))).toBe(state.timings);
    },
  });

  test("PBK-004 @supplemental › Master, bidirectional X-fade, and Temp retain distinct ownership", async ({ api, bench }) => {
    const prepared = await prepareShow(api, bench, "pbk-004-faders", "compact-rig", [0, 1, 0.5], 30_000, 10_000);
    await installPlaybacks(api, [definition(45, "Fader Modes", { type: "cue_list", cue_list_id: prepared.cueListId })], { 1: 45 });
    await poolAction(api, 45, "on");
    for (const level of [0, 0.5, 1]) {
      await poolAction(api, 45, "master", { value: level });
      expect(await activePlayback(api, 45)).toMatchObject({ current_cue_number: 1, master: level, fader_position: level });
    }

    await updatePlayback(api, 1, (current) => ({ ...current, fader: "x_fade" }));
    const timings = JSON.stringify((await object<any>(api, "cue_list", prepared.cueListId)).body.cues.map((cue: any) => [cue.fade_millis, cue.delay_millis]));
    for (const position of [0.25, 0.5, 0.75]) {
      await poolAction(api, 45, "master", { value: position });
      expect(await activePlayback(api, 45)).toMatchObject({ manual_xfade_position: position, manual_xfade_progress: position, manual_xfade_direction: "towards_high", current_cue_number: 1 });
    }
    await poolAction(api, 45, "master", { value: 1 });
    expect(await activePlayback(api, 45)).toMatchObject({ current_cue_number: 2, manual_xfade_direction: "towards_low", manual_xfade_position: 1 });
    await poolAction(api, 45, "master", { value: 1 });
    expect((await activePlayback(api, 45)).current_cue_number).toBe(2);
    for (const position of [0.75, 0.5, 0.25]) {
      await poolAction(api, 45, "master", { value: position });
      expect(await activePlayback(api, 45)).toMatchObject({ manual_xfade_position: position, manual_xfade_progress: 1 - position, manual_xfade_direction: "towards_low", current_cue_number: 2 });
    }
    await poolAction(api, 45, "master", { value: 0 });
    expect(await activePlayback(api, 45)).toMatchObject({ current_cue_number: 3, manual_xfade_direction: "towards_high", manual_xfade_position: 0 });
    expect(JSON.stringify((await object<any>(api, "cue_list", prepared.cueListId)).body.cues.map((cue: any) => [cue.fade_millis, cue.delay_millis]))).toBe(timings);

    const underneathId = await createCueList(api, prepared.fixtures, "Underlying", [0.3], 0, 0, [2]);
    await installPlaybacks(api, [
      { ...(await playbackAt(api, 1, 1)).body },
      definition(46, "Underlying", { type: "cue_list", cue_list_id: underneathId }),
    ], { 1: 45, 2: 46 });
    await poolAction(api, 46, "on");
    const underneathBefore = await activePlayback(api, 46);
    await updatePlayback(api, 1, (current) => ({ ...current, fader: "temp" }));
    for (const level of [0.25, 0.5, 1]) {
      await poolAction(api, 45, "master", { value: level });
      expect(await activePlayback(api, 45)).toMatchObject({ temporary_active: true, temporary_master: level });
      expect(await activePlayback(api, 46)).toMatchObject({ enabled: true, cue_index: underneathBefore.cue_index, activated_at: underneathBefore.activated_at });
    }
    await poolAction(api, 45, "master", { value: 0 });
    expect((await playbackSnapshot(api)).active.some((item: any) => item.playback_number === 45 && item.temporary_active)).toBe(false);
    expect(await activePlayback(api, 46)).toMatchObject({ enabled: true, cue_index: underneathBefore.cue_index, activated_at: underneathBefore.activated_at });

    await updatePlayback(api, 1, (current) => ({ ...current, fader: "x_fade" }));
    await poolAction(api, 45, "on");
    await poolAction(api, 45, "go-to", { cue_number: 1 });
    await poolAction(api, 45, "master", { value: 0.5 });
    const beforeReload = await activePlayback(api, 45);
    await bench.tick(0);
    expect(await activePlayback(api, 45)).toMatchObject({ manual_xfade_direction: beforeReload.manual_xfade_direction, manual_xfade_position: 0.5, manual_xfade_progress: 0.5 });
  });

  test("PBK-004 @supplemental-ui › X-fade progress survives browser reload as visible feedback", async ({ api, bench, desk, page }) => {
    const prepared = await prepareShow(api, bench, "pbk-004-ui", "compact-rig", [0, 1, 0.5], 30_000, 10_000);
    await installPlaybacks(api, [definition(47, "Manual Crossfade", { type: "cue_list", cue_list_id: prepared.cueListId }, { fader: "x_fade" })], { 1: 47 });
    await poolAction(api, 47, "on");
    await desk.open(bench.baseUrl);
    await openPlaybackMode(page);
    let slider = playbackCard(page, 1).getByRole("slider", { name: "X-fade" });
    await slider.fill("25");
    await expect.poll(async () => (await activePlayback(api, 47)).manual_xfade_progress).toBeCloseTo(0.25, 3);
    await expect(playbackCard(page, 1)).toContainText("Cue 1 → 2 · 25%");
    await page.reload();
    await openPlaybackMode(page);
    await expect(playbackCard(page, 1)).toContainText("Cue 1 → 2 · 25%");
    slider = playbackCard(page, 1).getByRole("slider", { name: "X-fade" });
    await slider.fill("100");
    await expect.poll(async () => (await activePlayback(api, 47)).current_cue_number).toBe(2);
    await expect(playbackCard(page, 1)).toContainText("Travel towards low");
  });

  pairedScenario<Pbk005State>({
    id: "PBK-005",
    title: "Temp and held Swap have explicit lifetimes and restore the underlying playback",
    arrange: async ({ api, bench }, surface) => {
      const prepared = await prepareShow(api, bench, `pbk-005-paired-${surface}`, "default-stage");
      const configuration = await api.request<any>("GET", "/api/v1/configuration");
      await api.request("PUT", "/api/v1/configuration", { ...configuration.configuration, sequence_master_fade_millis: 0 });
      const underlyingId = await createCueList(api, prepared.fixtures, "Underlying", [0.3], 0, 0, [1], false);
      const temporaryId = await createCueList(api, prepared.fixtures, "Temporary", [0.8], 0, 0, [1], false);
      const unprotectedId = await createCueList(api, prepared.fixtures, "Unprotected", [0.6], 0, 0, [2], false);
      const protectedId = await createCueList(api, prepared.fixtures, "Protected", [0.4], 0, 0, [3], false);
      await installPlaybacks(api, [
        definition(54, "Underlying", { type: "cue_list", cue_list_id: underlyingId }),
        definition(55, "Temporary", { type: "cue_list", cue_list_id: temporaryId }, { buttons: ["swap", "temp", "flash"] }),
        definition(56, "Unprotected", { type: "cue_list", cue_list_id: unprotectedId }, { auto_off: false }),
        definition(57, "Protected", { type: "cue_list", cue_list_id: protectedId }, { auto_off: false, protect_from_swap: true }),
      ], { 1: 54, 2: 55, 3: 56, 4: 57 });
      await poolAction(api, 54, "on");
      await poolAction(api, 56, "on");
      await poolAction(api, 57, "on");
      await bench.tick(0);
      return {
        ...prepared,
        permanentBefore: {
          54: await activePlayback(api, 54),
          56: await activePlayback(api, 56),
          57: await activePlayback(api, 57),
        },
        levelsBefore: await intensityLevels(api, prepared.fixtures, [1, 2, 3]),
        observations: {},
      };
    },
    api: async ({ api, bench }, state) => {
      await poolAction(api, 55, "temp");
      state.observations.tempDuring = Boolean((await activePlayback(api, 55)).temporary_active);
      await bench.tick(0);
      state.observations.tempLevels = await intensityLevels(api, state.fixtures, [1, 2, 3]);
      await poolAction(api, 55, "temp");
      state.observations.tempAfter = hasTemporaryRuntime(await playbackSnapshot(api), 55);
      await poolAction(api, 55, "swap", { pressed: true });
      state.observations.swapDuring = Boolean((await activePlayback(api, 55)).swap_active);
      await bench.tick(0);
      state.observations.swapLevels = await intensityLevels(api, state.fixtures, [1, 2, 3]);
      await poolAction(api, 55, "swap", { pressed: false });
      state.observations.swapAfter = hasSwapRuntime(await playbackSnapshot(api), 55);
    },
    ui: async ({ api, bench, desk, page }, state) => {
      await desk.open(bench.baseUrl);
      await openPlaybackMode(page);
      const temp = playbackCard(page, 2).getByRole("button", { name: "TEMP", exact: true });
      await temp.click();
      await expect.poll(async () => (await activePlayback(api, 55)).temporary_active).toBe(true);
      state.observations.tempDuring = true;
      await bench.tick(0);
      state.observations.tempLevels = await intensityLevels(api, state.fixtures, [1, 2, 3]);
      await temp.click();
      await expect.poll(async () => hasTemporaryRuntime(await playbackSnapshot(api), 55)).toBe(false);
      state.observations.tempAfter = false;
      const swap = playbackCard(page, 2).getByRole("button", { name: "SWAP", exact: true });
      await swap.hover();
      await page.mouse.down();
      try {
        await expect.poll(async () => hasSwapRuntime(await playbackSnapshot(api), 55)).toBe(true);
        state.observations.swapDuring = true;
        await bench.tick(0);
        state.observations.swapLevels = await intensityLevels(api, state.fixtures, [1, 2, 3]);
      } finally {
        await page.mouse.up();
      }
      await expect.poll(async () => hasSwapRuntime(await playbackSnapshot(api), 55)).toBe(false);
      state.observations.swapAfter = false;
    },
    assert: async ({ api }, state) => {
      expect(state.observations).toMatchObject({ tempDuring: true, tempAfter: false, swapDuring: true, swapAfter: false });
      expect(state.observations.tempLevels?.[1]).toBeCloseTo(0.8, 5);
      expect(state.observations.tempLevels?.[2]).toBeCloseTo(state.levelsBefore[2], 5);
      expect(state.observations.tempLevels?.[3]).toBeCloseTo(state.levelsBefore[3], 5);
      expect(state.observations.swapLevels?.[1]).toBeCloseTo(0.8, 5);
      expect(state.observations.swapLevels?.[2]).toBeCloseTo(0, 5);
      expect(state.observations.swapLevels?.[3]).toBeCloseTo(state.levelsBefore[3], 5);
      expect(await activePlayback(api, 54)).toEqual(state.permanentBefore[54]);
      expect(await activePlayback(api, 56)).toEqual(state.permanentBefore[56]);
      expect(await activePlayback(api, 57)).toEqual(state.permanentBefore[57]);
      const snapshot = await playbackSnapshot(api);
      expect(hasTemporaryRuntime(snapshot, 55)).toBe(false);
      expect(hasSwapRuntime(snapshot, 55)).toBe(false);
      const finalLevels = await intensityLevels(api, state.fixtures, [1, 2, 3]);
      for (const number of [1, 2, 3]) expect(finalLevels[number]).toBeCloseTo(state.levelsBefore[number], 5);
    },
  });

  test("PBK-005 @supplemental › Flash modes, auto-Off, Swap, and protection preserve permanent runtime", async ({ api, bench }) => {
    const prepared = await prepareShow(api, bench, "pbk-005-stack", "default-stage");
    const aId = await createCueList(api, prepared.fixtures, "A", [0.2], 0, 0, [1, 2], false);
    const bId = await createCueList(api, prepared.fixtures, "B", [0.8], 0, 0, [1, 2, 3], false);
    const cId = await createCueList(api, prepared.fixtures, "C", [0.6], 0, 0, [4], false);
    const dId = await createCueList(api, prepared.fixtures, "D protected", [0.4], 0, 0, [5], false);
    const a = definition(51, "A", { type: "cue_list", cue_list_id: aId }, { buttons: ["go", "flash", "none"] });
    const b = definition(52, "B", { type: "cue_list", cue_list_id: bId }, { buttons: ["flash", "temp", "swap"] });
    const c = definition(53, "C", { type: "cue_list", cue_list_id: cId }, { auto_off: false });
    const d = definition(54, "D protected", { type: "cue_list", cue_list_id: dId }, { auto_off: false, protect_from_swap: true });
    await installPlaybacks(api, [a, b, c, d], { 1: 51, 2: 52, 3: 53, 4: 54 });
    await poolAction(api, 51, "on");
    const aBefore = await activePlayback(api, 51);

    await poolAction(api, 52, "flash", { pressed: true });
    expect((await activePlayback(api, 51)).enabled).toBe(true);
    expect(await activePlayback(api, 52)).toMatchObject({ flash: true, temporary_active: true });
    await poolAction(api, 52, "flash", { pressed: false });
    expect(await activePlayback(api, 51)).toMatchObject({ enabled: true, cue_index: aBefore.cue_index, activated_at: aBefore.activated_at });
    expect((await playbackSnapshot(api)).active.some((item: any) => item.playback_number === 52 && item.temporary_active)).toBe(false);

    await poolAction(api, 52, "temp");
    expect(await activePlayback(api, 52)).toMatchObject({ temporary_active: true });
    expect((await activePlayback(api, 51)).enabled).toBe(true);
    await poolAction(api, 52, "temp");
    expect((await playbackSnapshot(api)).active.some((item: any) => item.playback_number === 52 && item.temporary_active)).toBe(false);
    expect(await activePlayback(api, 51)).toMatchObject({ enabled: true, activated_at: aBefore.activated_at });

    await updatePlayback(api, 2, (current) => ({ ...current, flash_release: "release_intensity_only" }));
    await poolAction(api, 52, "flash", { pressed: true });
    await poolAction(api, 52, "flash", { pressed: false });
    expect(await activePlayback(api, 52)).toMatchObject({ enabled: true, master: 0, temporary_active: false });
    await updatePlayback(api, 2, (current) => ({ ...current, flash_release: "release_all" }));
    await poolAction(api, 52, "off");
    await poolAction(api, 52, "flash", { pressed: true });
    await poolAction(api, 52, "flash", { pressed: false });
    expect((await playbackSnapshot(api)).active.some((item: any) => item.playback_number === 52 && item.enabled)).toBe(false);

    await poolAction(api, 51, "on");
    await updatePlayback(api, 1, (current) => ({ ...current, auto_off: true }));
    await poolAction(api, 52, "on");
    expect((await activePlayback(api, 51)).enabled).toBe(false);
    await updatePlayback(api, 1, (current) => ({ ...current, auto_off: false }));
    await poolAction(api, 51, "on");
    await poolAction(api, 52, "off");
    await poolAction(api, 52, "on");
    expect((await activePlayback(api, 51)).enabled).toBe(true);

    await poolAction(api, 53, "on");
    expect(await activePlayback(api, 53)).toMatchObject({ enabled: true, master: 1 });
    await poolAction(api, 54, "on");
    expect(await activePlayback(api, 53)).toMatchObject({ enabled: true, master: 1 });
    expect(await activePlayback(api, 54)).toMatchObject({ enabled: true, master: 1 });
    const aSwapBefore = await activePlayback(api, 51);
    const cSwapBefore = await activePlayback(api, 53);
    const dSwapBefore = await activePlayback(api, 54);
    const beforeSwap = logicalUniverse(await bench.tick(3_000));
    const beforeResolved = await api.request<any>("GET", "/api/v1/visualization");
    const resolvedLevel = (fixtureNumber: number) => beforeResolved.values.find((item: any) => item.fixture_id === prepared.fixtures[fixtureNumber] && item.attribute === "intensity")?.value?.value;
    expect(resolvedLevel(4)).toBeCloseTo(0.6, 5);
    expect(resolvedLevel(5)).toBeCloseTo(0.4, 5);
    expect(beforeSwap[3]).toBeGreaterThan(0);
    expect(beforeSwap[4]).toBeGreaterThan(0);
    await poolAction(api, 52, "swap", { pressed: true });
    expect(await activePlayback(api, 52)).toMatchObject({ swap_active: true, temporary_active: true });
    expect((await activePlayback(api, 51)).enabled).toBe(true);
    expect((await activePlayback(api, 53)).enabled).toBe(true);
    expect((await activePlayback(api, 54)).enabled).toBe(true);
    const duringSwap = logicalUniverse(await bench.tick(0));
    expect(duringSwap[3]).toBe(0);
    expect(duringSwap[4]).toBeGreaterThan(0);
    await poolAction(api, 52, "swap", { pressed: false });
    expect(await activePlayback(api, 51)).toMatchObject({ cue_index: aSwapBefore.cue_index, fader_position: aSwapBefore.fader_position, activated_at: aSwapBefore.activated_at });
    expect(await activePlayback(api, 53)).toMatchObject({ enabled: true, cue_index: cSwapBefore.cue_index, fader_position: cSwapBefore.fader_position, activated_at: cSwapBefore.activated_at });
    expect(await activePlayback(api, 54)).toMatchObject({ enabled: true, cue_index: dSwapBefore.cue_index, fader_position: dSwapBefore.fader_position, activated_at: dSwapBefore.activated_at });
    const afterSwap = logicalUniverse(await bench.tick(0));
    expect(afterSwap[3]).toBeGreaterThan(0);
    expect(afterSwap[4]).toBeGreaterThan(0);
    await api.request("POST", `/api/v1/shows/${prepared.showId}/open`, { transition: "hold_current" });
    expect((await playbackAt(api, 1, 4)).body.protect_from_swap).toBe(true);
    expect((await playbackAt(api, 1, 1)).body.auto_off).toBe(false);
  });

  test("PBK-005 @supplemental-ui › held Swap and toggled Temp show detailed lifetime feedback", async ({ api, bench, desk, page }) => {
    const prepared = await prepareShow(api, bench, "pbk-005-ui", "default-stage");
    const aId = await createCueList(api, prepared.fixtures, "Underlying", [0.3], 0, 0, [1]);
    const bId = await createCueList(api, prepared.fixtures, "Temporary", [0.8], 0, 0, [1]);
    await installPlaybacks(api, [
      definition(54, "Underlying", { type: "cue_list", cue_list_id: aId }),
      definition(55, "Temporary", { type: "cue_list", cue_list_id: bId }, { buttons: ["swap", "temp", "flash"] }),
    ], { 1: 54, 2: 55 });
    await poolAction(api, 54, "on");
    await desk.open(bench.baseUrl);
    await openPlaybackMode(page);
    const swap = playbackCard(page, 2).getByRole("button", { name: "SWAP", exact: true });
    await swap.hover();
    await page.mouse.down();
    await expect.poll(async () => (await playbackSnapshot(api)).active.find((item: any) => item.playback_number === 55)?.swap_active ?? false).toBe(true);
    await expect(playbackCard(page, 2)).toHaveClass(/swap-active/);
    await page.mouse.up();
    await expect.poll(async () => (await playbackSnapshot(api)).active.some((item: any) => item.playback_number === 55 && item.swap_active)).toBe(false);
    expect((await activePlayback(api, 54)).enabled).toBe(true);

    const temp = playbackCard(page, 2).getByRole("button", { name: "TEMP", exact: true });
    const [tempRequest] = await Promise.all([
      page.waitForRequest((request) => request.url().endsWith("/api/v1/cuelists/55/button")),
      temp.click(),
    ]);
    expect(tempRequest.postDataJSON()).toMatchObject({ button: 2, pressed: true, surface: "physical" });
    await expect.poll(async () => (await activePlayback(api, 55)).temporary_active).toBe(true);
    await expect(temp).toHaveClass(/playback-button-active/);
    await temp.click();
    await expect.poll(async () => (await playbackSnapshot(api)).active.some((item: any) => item.playback_number === 55 && item.temporary_active)).toBe(false);
    expect((await activePlayback(api, 54)).enabled).toBe(true);
  });

  pairedScenario<PreparedShow>({
    id: "PBK-006",
    title: "specialized layouts control their authoritative Speed, Group, Grand, and Fade masters",
    arrange: async ({ api, bench }, surface) => {
      const prepared = await prepareShow(api, bench, `pbk-006-paired-${surface}`, "default-stage");
      await setSpeedRates(api, [120, 96, 72, 60, 48]);
      for (const [group, bpm] of [[1, 120], [2, 96], [3, 72], [4, 60], [5, 48]] as const)
        await api.executeLegacyCommandLine(`SPD GRP ${group} AT ${bpm}`);
      await installPlaybacks(api, [
        definition(61, "Speed A", { type: "speed_group", group: "A" }, { buttons: ["double", "half", "learn"], fader: "learned_percentage", color: "#8b5cf6" }),
        definition(62, "Group 1", { type: "group", group_id: "1" }, { buttons: ["select", "select_dereferenced", "flash"] }),
        definition(63, "Grand", { type: "grand_master" }, { buttons: ["blackout", "pause_dynamics", "flash"] }),
        definition(64, "Programmer Fade", { type: "programmer_fade" }, { buttons: ["double", "half", "off"] }),
        definition(65, "Cue Fade", { type: "cue_fade" }, { buttons: ["double", "half", "off"] }),
      ], { 1: 61, 2: 62, 3: 63, 4: 64, 5: 65 });
      return prepared;
    },
    api: async ({ api }) => {
      await pressButton(api, 61, 1);
      await poolAction(api, 61, "master", { value: 0.5 });
      await poolAction(api, 62, "master", { value: 0.4 });
      await pressButton(api, 62, 1);
      await poolAction(api, 63, "master", { value: 0.3 });
      await pressButton(api, 63, 1);
      await poolAction(api, 64, "master", { value: 0.25 });
      await poolAction(api, 65, "master", { value: 0.25 });
    },
    ui: async ({ bench, desk, page }) => {
      await desk.open(bench.baseUrl);
      await openPlaybackMode(page);
      await playbackCard(page, 1).getByRole("button", { name: "DOUBLE", exact: true }).click();
      await playbackSlider(page, 1).fill("50");
      await playbackSlider(page, 2).fill("40");
      await playbackCard(page, 2).getByRole("button", { name: "SELECT", exact: true }).click();
      await playbackSlider(page, 3).fill("30");
      await playbackCard(page, 3).getByRole("button", { name: "BLACKOUT", exact: true }).click();
      await playbackSlider(page, 4).fill("25");
      await playbackSlider(page, 5).fill("25");
    },
    assert: async ({ api }) => {
      await expect.poll(async () => authoritativeMasterObservation(await controls(api))).toEqual({
        speed: { manualBpm: 240, effectiveBpm: 120, paused: false },
        neighborBpms: [96, 72, 60, 48],
        group: { master: 0.4, flashLevel: 0 },
        grand: { level: 0.3, effectiveLevel: 0.3, blackout: true, dynamicsPaused: false },
        programmerFadeMillis: 5_000,
        cueFadeMillis: 15_000,
      });
      await expect.poll(async () => (await programmer(api)).selection_expression).toMatchObject({ type: "live_group", group_id: "1" });
      expect((await playbackAt(api, 1, 1)).body).toMatchObject({ target: { type: "speed_group", group: "A" }, buttons: ["double", "half", "learn"], fader: "learned_percentage", color: "#8b5cf6" });
      expect((await playbackAt(api, 1, 2)).body.target).toEqual({ type: "group", group_id: "1" });
      expect((await playbackAt(api, 1, 3)).body.target).toEqual({ type: "grand_master" });
      expect((await playbackAt(api, 1, 4)).body).toMatchObject({ target: { type: "programmer_fade" }, buttons: ["double", "half", "off"] });
      expect((await playbackAt(api, 1, 5)).body).toMatchObject({ target: { type: "cue_fade" }, buttons: ["double", "half", "off"] });
    },
  });

  test("PBK-006 @supplemental › specialized layouts cover every action and exact fader checkpoint", async ({ api, bench }) => {
    const prepared = await prepareShow(api, bench, "pbk-006-masters", "default-stage");
    await setSpeedRates(api, [120, 96, 72, 60, 48]);
    await installPlaybacks(api, [
      definition(61, "Speed A", { type: "speed_group", group: "A" }, { buttons: ["double", "half", "learn"], fader: "learned_percentage" }),
      definition(62, "Group 1", { type: "group", group_id: "1" }, { buttons: ["select", "select_dereferenced", "flash"] }),
      definition(63, "Grand", { type: "grand_master" }, { buttons: ["blackout", "pause_dynamics", "flash"] }),
      definition(64, "Programmer Fade", { type: "programmer_fade" }, { buttons: ["double", "half", "off"] }),
      definition(65, "Cue Fade", { type: "cue_fade" }, { buttons: ["double", "half", "off"] }),
    ], { 1: 61, 2: 62, 3: 63, 4: 64, 5: 65 });

    expect((await playbackAt(api, 1, 1)).body).toMatchObject({ buttons: ["double", "half", "learn"], fader: "learned_percentage" });
    await pressButton(api, 61, 1);
    expect((await controls(api)).speed_groups[0].manual_bpm).toBe(240);
    await pressButton(api, 61, 2);
    expect((await controls(api)).speed_groups[0].manual_bpm).toBe(120);
    await updatePlayback(api, 1, (current) => ({ ...current, buttons: ["pause", "half", "learn"] }));
    await pressButton(api, 61, 1);
    expect((await controls(api)).speed_groups[0].paused).toBe(true);
    await pressButton(api, 61, 1);
    expect((await controls(api)).speed_groups[0].paused).toBe(false);
    await pressButton(api, 61, 3);
    await bench.tick(500);
    await pressButton(api, 61, 3);
    expect((await controls(api)).speed_groups[0].manual_bpm).toBeCloseTo(120, 3);

    await updatePlayback(api, 1, (current) => ({ ...current, fader: "direct_bpm" }));
    for (const [position, bpm] of [[0, 0], [0.5, 150], [1, 300]] as const) {
      await poolAction(api, 61, "master", { value: position });
      const speed = (await controls(api)).speed_groups[0];
      expect(speed.effective_bpm).toBeCloseTo(bpm, 3);
      expect(speed.paused).toBe(position === 0);
    }
    await setSpeedRates(api, [120, 96, 72, 60, 48]);
    await updatePlayback(api, 1, (current) => ({ ...current, fader: "learned_percentage" }));
    for (const [position, bpm] of [[0, 0], [0.5, 60], [1, 120]] as const) {
      await poolAction(api, 61, "master", { value: position });
      expect((await controls(api)).speed_groups[0].effective_bpm).toBeCloseTo(bpm, 3);
    }
    await updatePlayback(api, 1, (current) => ({ ...current, fader: "centered_relative" }));
    for (const [position, bpm] of [[0, 30], [0.5, 120], [1, 480]] as const) {
      await poolAction(api, 61, "master", { value: position });
      expect((await controls(api)).speed_groups[0].effective_bpm).toBeCloseTo(bpm, 3);
    }
    const neighbors = (await controls(api)).speed_groups.slice(1).map((speed: any) => speed.manual_bpm);
    expect(neighbors).toEqual([96, 72, 60, 48]);

    await poolAction(api, 62, "master", { value: 0.4 });
    let groupControl = (await controls(api)).groups.find((group: any) => group.id === "1");
    expect(groupControl.flash_level).toBe(0);
    expect(groupControl.master).toBeCloseTo(0.4, 5);
    await pressButton(api, 62, 1);
    expect((await programmer(api)).selection_expression).toMatchObject({ type: "live_group", group_id: "1" });
    await pressButton(api, 62, 2);
    expect((await programmer(api)).selection_expression).toEqual({ type: "static" });
    await pressButton(api, 62, 3, true);
    groupControl = (await controls(api)).groups.find((group: any) => group.id === "1");
    expect(groupControl.flash_level).toBe(1);
    expect(groupControl.master).toBeCloseTo(0.4, 5);
    await pressButton(api, 62, 3, false);
    groupControl = (await controls(api)).groups.find((group: any) => group.id === "1");
    expect(groupControl.flash_level).toBe(0);
    expect(groupControl.master).toBeCloseTo(0.4, 5);

    await poolAction(api, 63, "master", { value: 0.3 });
    await pressButton(api, 63, 1);
    let grandMaster = (await controls(api)).grand_master;
    expect(grandMaster.blackout).toBe(true);
    expect(grandMaster.level).toBeCloseTo(0.3, 5);
    expect(grandMaster.effective_level).toBeCloseTo(0.3, 5);
    await pressButton(api, 63, 3, true);
    grandMaster = (await controls(api)).grand_master;
    expect(grandMaster.flash_active).toBe(true);
    expect(grandMaster.level).toBeCloseTo(0.3, 5);
    expect(grandMaster.effective_level).toBe(1);
    await pressButton(api, 63, 3, false);
    grandMaster = (await controls(api)).grand_master;
    expect(grandMaster.flash_active).toBe(false);
    expect(grandMaster.level).toBeCloseTo(0.3, 5);
    expect(grandMaster.effective_level).toBeCloseTo(0.3, 5);
    await pressButton(api, 63, 2);
    expect((await controls(api)).grand_master.dynamics_paused).toBe(true);
    await pressButton(api, 63, 2);
    expect((await controls(api)).grand_master.dynamics_paused).toBe(false);

    await poolAction(api, 64, "master", { value: 0.25 });
    await poolAction(api, 65, "master", { value: 0.25 });
    expect(await controls(api)).toMatchObject({ programmer_fade_millis: 5_000, cue_fade_millis: 15_000 });
    await pressButton(api, 64, 1);
    await pressButton(api, 65, 1);
    expect(await controls(api)).toMatchObject({ programmer_fade_millis: 10_000, cue_fade_millis: 30_000 });
    await pressButton(api, 64, 2);
    await pressButton(api, 65, 2);
    expect(await controls(api)).toMatchObject({ programmer_fade_millis: 5_000, cue_fade_millis: 15_000 });
    await pressButton(api, 64, 3);
    await pressButton(api, 65, 3);
    expect(await controls(api)).toMatchObject({ programmer_fade_millis: 0, cue_fade_millis: 0 });
  });

  test("PBK-006 @supplemental-ui › specialized controls render fixed layouts and detailed feedback", async ({ api, bench, desk, page }) => {
    await prepareShow(api, bench, "pbk-006-ui", "default-stage");
    await setSpeedRates(api, [120, 96, 72, 60, 48]);
    await installPlaybacks(api, [
      definition(66, "Speed A", { type: "speed_group", group: "A" }, { buttons: ["double", "half", "learn"], fader: "learned_percentage", color: "#8b5cf6" }),
      definition(67, "Group 1", { type: "group", group_id: "1" }, { buttons: ["select", "select_dereferenced", "flash"] }),
      definition(68, "Grand", { type: "grand_master" }, { buttons: ["blackout", "pause_dynamics", "flash"] }),
      definition(69, "Programmer Fade", { type: "programmer_fade" }, { buttons: ["double", "half", "off"] }),
      definition(70, "Cue Fade", { type: "cue_fade" }, { buttons: ["double", "half", "off"] }),
    ], { 1: 66, 2: 67, 3: 68, 4: 69, 5: 70 });
    await desk.open(bench.baseUrl);
    await setSpeedRates(api, [120, 96, 72, 60, 48]);
    await page.reload();
    await openPlaybackMode(page);
    await expect.poll(async () => (await controls(api)).speed_groups[0].effective_bpm).toBeCloseTo(120, 3);
    await expect(playbackCard(page, 1)).toContainText("120 BPM");
    await expect(playbackCard(page, 1)).toHaveCSS("--playback-color", "#8b5cf6");
    const threeButtonLayout = playbackCard(page, 1).locator(".vertical-touch-fader-actions > .ui-button");
    await expect(threeButtonLayout).toHaveCount(3);
    const [firstButtonBox, secondButtonBox, bottomButtonBox] = await Promise.all([0, 1, 2].map((index) => threeButtonLayout.nth(index).boundingBox()));
    expect(firstButtonBox).not.toBeNull(); expect(secondButtonBox).not.toBeNull(); expect(bottomButtonBox).not.toBeNull();
    expect(Math.abs(firstButtonBox!.y - secondButtonBox!.y)).toBeLessThan(2);
    expect(bottomButtonBox!.y).toBeGreaterThan(firstButtonBox!.y + firstButtonBox!.height);
    expect(bottomButtonBox!.width).toBeGreaterThan(firstButtonBox!.width * 1.9);
    await playbackCard(page, 1).getByRole("button", { name: "DOUBLE", exact: true }).click();
    await expect.poll(async () => (await controls(api)).speed_groups[0].manual_bpm).toBe(240);
    await expect(playbackCard(page, 1)).toContainText("240 BPM");

    await poolAction(api, 67, "master", { value: 0.4 });
    await expect(playbackCard(page, 2)).toContainText("40% master");
    await expect(playbackCard(page, 2).getByRole("slider", { name: "Group master" })).toHaveValue("40");
    await poolAction(api, 68, "master", { value: 0.3 });
    await expect(playbackCard(page, 3).getByRole("slider", { name: "Grand Master" })).toHaveValue("30");
    await poolAction(api, 69, "master", { value: 0.25 });
    await poolAction(api, 70, "master", { value: 0.25 });
    await expect(playbackCard(page, 4)).toContainText("5.0 s");
    await expect(playbackCard(page, 5)).toContainText("15.0 s");
    for (const slot of [4, 5]) {
      await expect(playbackCard(page, slot).getByRole("button", { name: "DOUBLE", exact: true })).toBeVisible();
      await expect(playbackCard(page, slot).getByRole("button", { name: "HALF", exact: true })).toBeVisible();
      await expect(playbackCard(page, slot).getByRole("button", { name: "OFF", exact: true })).toBeVisible();
    }

    await armSet(page);
    await page.getByRole("button", { name: "Playback representation page 1 playback 1" }).click();
    const modal = await expectConfigurationModal(page, 1, 1);
    await modal.getByRole("button", { name: "Layout", exact: true }).click();
    await expect(selectTrigger(modal, "Top button")).toContainText("Double");
    await expect(selectTrigger(modal, "Middle button")).toContainText("Half");
    await expect(selectTrigger(modal, "Bottom button")).toContainText("Learn");
    await expect(selectTrigger(modal, "Fader")).toContainText("Learned-speed percentage");
    await modal.getByRole("button", { name: "Close playback configuration", exact: true }).click();
  });

  test("PBK-006 @osc › external controls and LED/fader/action feedback share the authoritative master state", async ({ api, bench }) => {
    await prepareShow(api, bench, "pbk-006-osc", "default-stage");
    await setSpeedRates(api, [120, 96, 72, 60, 48]);
    await installPlaybacks(api, [definition(71, "OSC Speed", { type: "speed_group", group: "A" }, {
      buttons: ["double", "half", "learn"], fader: "learned_percentage", color: "#8b5cf6",
    })], { 1: 71 });
    const hardware = await bench.osc();
    const alias = api.session!.desk.osc_alias;
    try {
      await hardware.subscribe(`pbk-006-${crypto.randomUUID()}`, alias);
      let mark = hardware.mark();
      await bench.tick(0);
      const fader = await hardware.expectAfter(mark, `/light/${alias}/feedback/page-playback/1/fader`);
      expect(fader.arguments[0]).toBeCloseTo(1, 4);
      const led = await hardware.expectAfter(mark, `/light/${alias}/feedback/page-playback/1/button/1`);
      expect(led.arguments.slice(0, 3)).toEqual(expect.arrayContaining([
        expect.closeTo(0x8b / 255 * 0.35, 4),
        expect.closeTo(0x5c / 255 * 0.35, 4),
        expect.closeTo(0xf6 / 255 * 0.35, 4),
      ]));
      expect(led.arguments[3]).toBe("off");
      expect((await hardware.expectAfter(mark, `/light/${alias}/feedback/page-playback/1/button/1/action`)).arguments).toEqual(["double"]);

      mark = hardware.mark();
      await hardware.send(`/light/${alias}/page-playback/1/button/1`, [true]);
      await expect.poll(async () => (await controls(api)).speed_groups[0].manual_bpm).toBe(240);
      await bench.tick(0);
      const speedFeedbackAddress = `/light/${alias}/feedback/speed-group/1`;
      await expect.poll(() => hardware.messages.slice(mark).some((message) =>
        message.address === speedFeedbackAddress && message.arguments[0] === 240,
      )).toBe(true);

      mark = hardware.mark();
      await hardware.send(`/light/${alias}/page-playback/1/fader`, [0.5]);
      await expect.poll(async () => (await controls(api)).speed_groups[0].effective_bpm).toBeCloseTo(120, 3);
      await bench.tick(0);
      const faderFeedbackAddress = `/light/${alias}/feedback/page-playback/1/fader`;
      await expect.poll(() => hardware.messages.slice(mark).some((message) =>
        message.address === faderFeedbackAddress
          && typeof message.arguments[0] === "number"
          && Math.abs(message.arguments[0] - 0.5) < 0.0001,
      )).toBe(true);
    } finally {
      await hardware.close();
    }
  });
});

async function playbackConfigurationObservation(api: ApiDriver, page: number, slot: number, expectedCueListId: string): Promise<PlaybackConfigurationObservation> {
  const playback = await playbackAt(api, page, slot);
  return {
    page,
    slot,
    number: playback.body.number,
    targetType: playback.body.target.type,
    targetMatchesExpected: playback.body.target.type === "cue_list" && playback.body.target.cue_list_id === expectedCueListId,
    buttons: [...playback.body.buttons],
    buttonCount: playback.body.button_count,
    fader: playback.body.fader,
    hasFader: playback.body.has_fader,
    color: playback.body.color,
  };
}

function serializedCueTimings(cueList: { body: { cues: Array<{ fade_millis: number; delay_millis: number }> } }): string {
  return JSON.stringify(cueList.body.cues.map((cue) => [cue.fade_millis, cue.delay_millis]));
}

function xfadeObservation(runtime: any, intensity: number): Pbk004State["checkpoints"][number] {
  return {
    cue: runtime.current_cue_number,
    position: runtime.manual_xfade_position,
    progress: runtime.manual_xfade_progress,
    direction: runtime.manual_xfade_direction,
    intensity,
  };
}

async function visualizationLevel(api: ApiDriver, fixtureId: string): Promise<number> {
  const snapshot = await api.request<any>("GET", "/api/v1/visualization");
  const value = snapshot.values.find((entry: any) => entry.fixture_id === fixtureId && entry.attribute === "intensity")?.value;
  return typeof value === "number" ? value : value?.value ?? 0;
}

async function intensityLevels(api: ApiDriver, fixtures: Record<number, string>, numbers: number[]): Promise<Record<number, number>> {
  return Object.fromEntries(await Promise.all(numbers.map(async (number) => [number, await visualizationLevel(api, fixtures[number])] as const)));
}

function hasTemporaryRuntime(snapshot: any, number: number): boolean {
  return snapshot.active.some((item: any) => item.playback_number === number && item.temporary_active);
}

function hasSwapRuntime(snapshot: any, number: number): boolean {
  return snapshot.active.some((item: any) => item.playback_number === number && item.swap_active);
}

function authoritativeMasterObservation(state: any) {
  const group = state.groups.find((candidate: any) => candidate.id === "1");
  const speed = state.speed_groups[0];
  const rounded = (value: number) => Math.round(value * 1_000) / 1_000;
  return {
    speed: { manualBpm: rounded(speed.manual_bpm), effectiveBpm: rounded(speed.effective_bpm), paused: speed.paused },
    neighborBpms: state.speed_groups.slice(1).map((candidate: any) => rounded(candidate.manual_bpm)),
    group: { master: rounded(group.master), flashLevel: rounded(group.flash_level) },
    grand: {
      level: rounded(state.grand_master.level),
      effectiveLevel: rounded(state.grand_master.effective_level),
      blackout: state.grand_master.blackout,
      dynamicsPaused: state.grand_master.dynamics_paused,
    },
    programmerFadeMillis: state.programmer_fade_millis,
    cueFadeMillis: state.cue_fade_millis,
  };
}

function definition(number: number, name: string, target: PlaybackTarget, overrides: Partial<PlaybackDefinition> = {}): PlaybackDefinition {
  const defaults = target.type === "speed_group"
    ? { buttons: ["double", "half", "learn"] as [string, string, string], fader: "learned_percentage" }
    : target.type === "group"
      ? { buttons: ["select", "select_dereferenced", "flash"] as [string, string, string], fader: "master" }
      : target.type === "grand_master"
        ? { buttons: ["blackout", "pause_dynamics", "flash"] as [string, string, string], fader: "master" }
        : target.type === "programmer_fade" || target.type === "cue_fade"
          ? { buttons: ["double", "half", "off"] as [string, string, string], fader: "master" }
          : { buttons: ["go_minus", "go", "flash"] as [string, string, string], fader: "master" };
  return {
    number,
    name,
    target,
    buttons: defaults.buttons,
    button_count: 3,
    fader: defaults.fader,
    has_fader: true,
    go_activates: true,
    auto_off: true,
    xfade_millis: 0,
    color: "#20c997",
    flash_release: "release_all",
    protect_from_swap: false,
    ...overrides,
  };
}

async function prepareShow(
  api: ApiDriver,
  bench: any,
  name: string,
  fixture: "compact-rig" | "default-stage",
  levels = [0.2, 0.8, 0.4],
  fadeMillis = 0,
  delayMillis = 0,
): Promise<PreparedShow> {
  const show = await loadCanonicalCopy(api, bench, name, fixture);
  const fixtures = await fixtureIdsByNumber(api);
  const existingGroups = await objects<any>(api, "group");
  for (const [id, groupName, members] of [
    ["1", "All Fixtures", Object.values(fixtures)],
    ["3", "Front Fixtures", Object.values(fixtures).slice(0, 4)],
  ] as const) {
    if (existingGroups.some((group) => group.id === id)) continue;
    await putObject(api, "group", id, {
      id,
      name: groupName,
      fixtures: members,
      derived_from: null,
      frozen_from: null,
      programming: {},
      master: 1,
      playback_fader: Number(id),
    });
  }
  const cueListId = await createCueList(api, fixtures, "Configured Sequence", levels, fadeMillis, delayMillis, [1, 2]);
  return { showId: show.id, cueListId, fixtures };
}

async function createCueList(
  api: ApiDriver,
  fixtures: Record<number, string>,
  name: string,
  levels: number[],
  fadeMillis: number,
  delayMillis: number,
  fixtureNumbers: number[],
  includeGroupChange = true,
): Promise<string> {
  const id = crypto.randomUUID();
  await putObject(api, "cue_list", id, {
    id,
    name,
    priority: 0,
    mode: "sequence",
    looped: false,
    chaser_step_millis: 1_000,
    speed_group: null,
    cues: levels.map((level, index) => ({
      id: crypto.randomUUID(),
      number: index + 1,
      name: `Cue ${index + 1}`,
      changes: fixtureNumbers.map((number) => ({
        fixture_id: fixtures[number],
        attribute: "intensity",
        value: { kind: "normalized", value: level },
        automatic_restore: false,
      })),
      group_changes: includeGroupChange && index === 0 ? [{
        group_id: "3",
        attribute: "intensity",
        value: { kind: "normalized", value: level },
        fade_millis: fadeMillis,
        delay_millis: delayMillis,
      }] : [],
      fade_millis: fadeMillis,
      delay_millis: delayMillis,
      trigger: { type: "manual" },
      phasers: [],
    })),
  });
  return id;
}

async function installPlaybacks(api: ApiDriver, definitions: PlaybackDefinition[], slots: Record<number, number>): Promise<void> {
  for (const playback of definitions) {
    const current = (await objects(api, "playback")).find((item) => item.id === String(playback.number));
    await putObject(api, "playback", String(playback.number), playback, current?.revision ?? 0);
  }
  await writePage(api, 1, Object.fromEntries(Object.entries(slots).map(([slot, number]) => [String(slot), number])));
}

async function writePage(api: ApiDriver, number: number, slots: Record<string, number>): Promise<void> {
  const current = (await objects<any>(api, "playback_page")).find((item) => item.id === String(number));
  await putObject(api, "playback_page", String(number), { number, name: number === 1 ? "Main" : `Page ${number}`, slots }, current?.revision ?? 0);
}

async function pageObject(api: ApiDriver, page: number) {
  return object<any>(api, "playback_page", String(page));
}

async function playbackAt(api: ApiDriver, page: number, slot: number) {
  const pageState = await pageObject(api, page);
  const number = pageState.body.slots[String(slot)];
  expect(number).toBeDefined();
  return object<PlaybackDefinition>(api, "playback", String(number));
}

async function saveSlot(api: ApiDriver, page: number, slot: number, playback: PlaybackDefinition) {
  const pageState = await pageObject(api, page);
  const currentNumber = pageState.body.slots[String(slot)];
  const currentPlayback = currentNumber == null ? undefined : (await objects<PlaybackDefinition>(api, "playback")).find((item) => item.id === String(currentNumber));
  return api.request<any>("PUT", `/api/v1/playback-pages/${page}/slots/${slot}`, {
    playback,
    expected_playback_revision: currentPlayback?.revision ?? 0,
    expected_page_revision: pageState.revision,
  });
}

async function clearSlot(api: ApiDriver, page: number, slot: number) {
  const pageState = await pageObject(api, page);
  const playback = await playbackAt(api, page, slot);
  return api.request<any>("DELETE", `/api/v1/playback-pages/${page}/slots/${slot}`, {
    expected_playback_revision: playback.revision,
    expected_page_revision: pageState.revision,
  });
}

async function updatePlayback(api: ApiDriver, slot: number, mutate: (current: PlaybackDefinition) => PlaybackDefinition) {
  const current = await playbackAt(api, 1, slot);
  return saveSlot(api, 1, slot, mutate(current.body));
}

async function setFirstButton(api: ApiDriver, slot: number, action: string): Promise<void> {
  await updatePlayback(api, slot, (current) => ({ ...current, buttons: [action, current.buttons[1], current.buttons[2]] }));
}

async function poolAction<T = any>(api: ApiDriver, number: number, action: string, body: Record<string, unknown> = {}): Promise<T> {
  return api.request<T>(action === "master" ? "PUT" : "POST", `/api/v1/playback-pool/${number}/${action}`, body);
}

async function pressButton(api: ApiDriver, number: number, button = 1, pressed = true) {
  return poolAction(api, number, "button", { button, pressed, surface: "physical" });
}

async function playbackSnapshot(api: ApiDriver) {
  return api.request<any>("GET", "/api/v1/playbacks");
}

async function activePlayback(api: ApiDriver, number: number) {
  const active = (await playbackSnapshot(api)).active.find((item: any) => item.playback_number === number);
  expect(active).toBeDefined();
  return active;
}

async function controls(api: ApiDriver) {
  return (await playbackSnapshot(api)).authoritative_controls;
}

async function logicalDmx(api: ApiDriver): Promise<number[]> {
  const snapshot = await api.request<any>("GET", "/api/v1/dmx", undefined, false);
  return logicalUniverse(snapshot);
}

function logicalUniverse(snapshot: { universes: Array<{ universe: number; slots: number[] }> }): number[] {
  return snapshot.universes.find((universe) => universe.universe === 1)?.slots ?? [];
}

async function audit(api: ApiDriver): Promise<any[]> {
  return api.request<any[]>("GET", "/api/v1/audit?after=0");
}

async function inertSnapshot(api: ApiDriver, number: number) {
  const playback = await object<PlaybackDefinition>(api, "playback", String(number));
  const state = await playbackSnapshot(api);
  return {
    object: playback,
    pool: state.pool,
    pages: state.pages,
    active: state.active,
    selected_playback: state.selected_playback,
    audit: await audit(api),
    dmx: await logicalDmx(api),
  };
}

async function setSpeedRates(api: ApiDriver, rates: number[]): Promise<void> {
  const response = await api.request<any>("GET", "/api/v1/configuration");
  await api.request("PUT", "/api/v1/configuration", {
    ...response.configuration,
    speed_groups_bpm: rates,
    speed_group_sound_to_light: response.configuration.speed_group_sound_to_light.map((sound: any) => ({ ...sound, enabled: false })),
  });
}

async function openPlaybackMode(page: Page): Promise<void> {
  if (await page.locator(".playback-fader-bank").isVisible()) return;
  await page.locator(".mode-toggle").click();
  await expect(page.locator(".playback-fader-bank")).toBeVisible();
}

async function armSet(page: Page): Promise<void> {
  await page.getByRole("button", { name: "SET", exact: true }).click();
}

function playbackCard(page: Page, slot: number): Locator {
  return page.locator(`.playback-fader-bank article[data-playback-slot="${slot}"]`);
}

function playbackSlider(page: Page, slot: number): Locator {
  return playbackCard(page, slot).getByRole("slider");
}

async function expectConfigurationModal(page: Page, playbackPage: number, slot: number): Promise<Locator> {
  const modal = page.getByRole("dialog", { name: "Playback Configuration" });
  await expect(modal).toHaveCount(1);
  await expect(modal).toBeVisible();
  await expect(modal).toHaveAttribute("data-page", String(playbackPage));
  await expect(modal).toHaveAttribute("data-slot", String(slot));
  return modal;
}

function selectTrigger(container: Locator, label: string): Locator {
  const escaped = label.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return container.locator(".ui-form-field").filter({ hasText: new RegExp(`^\\s*${escaped}`) }).locator(".ui-select-trigger");
}

async function chooseSelect(page: Page, container: Locator, label: string, option: string): Promise<void> {
  const trigger = selectTrigger(container, label);
  await trigger.click();
  if (await trigger.getAttribute("aria-haspopup") === "dialog") {
    const dialog = page.getByRole("dialog", { name: `Choose ${label} function` });
    await dialog.getByRole("button").filter({ hasText: new RegExp(`^${option.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}`) }).click();
    return;
  }
  await page.getByRole("option", { name: option, exact: true }).click();
}

async function choosePlaybackColor(page: Page, container: Locator, color: string): Promise<void> {
  const before = await container.boundingBox();
  await container.locator(".ui-form-field", { hasText: "Playback color" }).locator(".ui-color-input-trigger").click();
  await expect(page.locator("body > .ui-color-dropdown-backdrop .ui-color-dropdown-panel")).toBeVisible();
  const after = await container.boundingBox();
  expect(after?.width).toBeCloseTo(before?.width ?? 0, 0);
  expect(after?.height).toBeCloseTo(before?.height ?? 0, 0);
  await page.getByRole("option", { name: `Use color ${color}`, exact: true }).click();
}

async function addVirtualPlaybackPane(page: Page): Promise<Locator> {
  await page.getByRole("button", { name: "DESKTOPS", exact: true }).click();
  await page.getByRole("button", { name: /New desktop/ }).click();
  const grid = page.locator(".desk-grid");
  const box = await grid.boundingBox();
  expect(box).not.toBeNull();
  await page.mouse.click(box!.x + Math.min(120, box!.width / 4), box!.y + Math.min(90, box!.height / 4));
  await expect(page.getByRole("heading", { name: "Open Window" })).toBeVisible();
  await page.getByRole("button", { name: "Virtual Playbacks", exact: true }).click();
  const pane = page.locator(".desk-pane").filter({ hasText: "Virtual Playbacks" });
  await expect(pane).toBeVisible();
  return pane;
}
