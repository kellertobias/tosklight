import type { Locator, Page } from "../apps/control-ui/node_modules/@playwright/test/index.js";
import type { ApiDriver } from "../apps/control-ui/e2e/bench/api";
import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import { pairedScenario } from "../apps/control-ui/e2e/bench/pairedScenario";
import { setProgrammerFixtureValue } from "../apps/control-ui/e2e/bench/programmerValues";
import { fixtureIdsByNumber, loadCanonicalCopy, object, objects, programmer, putObject } from "./support/catalog";

type Prepared = { firstCuelist: string; secondCuelist: string; showId: string };

pairedScenario<Prepared>({
  id: "PLAYBACK-SELECT-001",
  title: "A hardware-connected playback card selects its concrete Cuelist playback",
  arrange: async ({ api, bench }, surface) => prepare(api, bench, `playback-select-001-${surface}`),
  api: async ({ api }) => {
    await selectPlayback(api, 41);
  },
  ui: async ({ api, bench, desk, page }) => {
    await desk.open(api.baseUrl);
    await openPlaybackMode(page);
    const hardware = await connectHardware(api, bench);
    try {
      const card = playbackCard(page, 1);
      await expect(card.getByRole("button", { name: /Playback representation/ })).toHaveCount(0);
      await card.locator("header b").click();
      await expect(page.locator(".ui-window-header")).toContainText("Cuelist View · Cuelist 41 · Front Cuelist");
    } finally {
      await disconnectHardware(hardware);
    }
  },
  assert: async ({ api }) => {
    expect((await playbackState(api)).selected_playback).toBe(41);
  },
});

test("PLAYBACK-SELECT-001 @supplemental-ui › controls, Record, Group selection, and explicit pages retain separate ownership", async ({ api, bench, desk, page }) => {
  const prepared = await prepare(api, bench, "playback-select-001-boundaries");
  const fixtures = await fixtureIdsByNumber(api);
  await api.command("selection.set", { fixtures: [fixtures[1]] });
  await setProgrammerFixtureValue(api, {
    surface: "api",
    showId: prepared.showId,
    fixtureId: fixtures[1],
    attribute: "intensity",
    value: { kind: "normalized", value: 0.73 },
    timing: { fade: true, fadeMillis: 3_000, delayMillis: null },
  });

  await desk.open(api.baseUrl);
  await openPlaybackMode(page);
  const hardware = await connectHardware(api, bench);
  try {
    const first = playbackCard(page, 1);
    await first.locator("header b").click();
    await expect.poll(async () => (await playbackState(api)).selected_playback).toBe(41);

    await first.getByRole("button", { name: "GO +", exact: true }).click();
    await first.getByRole("slider").evaluate((element: HTMLInputElement) => {
      element.value = "37";
      element.dispatchEvent(new Event("input", { bubbles: true }));
    });
    expect((await playbackState(api)).selected_playback).toBe(41);

    const beforeFirst = (await object<any>(api, "cue_list", prepared.firstCuelist)).body.cues.length;
    const beforeSecond = (await object<any>(api, "cue_list", prepared.secondCuelist)).body.cues.length;
    await page.getByRole("button", { name: "REC", exact: true }).click();
    await first.getByRole("button", { name: "GO +", exact: true }).click();
    await expect.poll(async () => (await object<any>(api, "cue_list", prepared.firstCuelist)).body.cues.length).toBe(beforeFirst + 1);
    expect((await object<any>(api, "cue_list", prepared.secondCuelist)).body.cues).toHaveLength(beforeSecond);
    expect((await playbackState(api)).selected_playback).toBe(41);

    await returnToPlaybackDesk(page);
    await selectHardwarePage(page, 1, 2, "Page 2");
    await playbackCard(page, 1).locator("header b").click();
    await expect.poll(async () => (await playbackState(api)).selected_playback).toBe(42);
    await expect(page.locator(".ui-window-header")).toContainText("Cuelist View · Cuelist 42 · Rear Cuelist");
    await returnToPlaybackDesk(page);
    await selectHardwarePage(page, 2, 1, "Main");
    expect((await playbackState(api)).selected_playback).toBe(42);

    await playbackCard(page, 2).locator(".hardware-cue-list").click();
    await expect.poll(async () => (await playbackState(api)).selected_playback).toBe(43);
    await expect.poll(async () => (await programmer(api)).selected).toContain(fixtures[1]);
    await expect(page.locator(".playback-fader-bank")).toBeVisible();

    await selectPlayback(api, 41);
    await hardware.send(`/light/${api.session!.desk.osc_alias}/page-playback/2/button/1`, [true]);
    await expect.poll(async () => (await playbackState(api)).selected_playback).toBe(43);
  } finally {
    await disconnectHardware(hardware);
  }
});

async function prepare(api: ApiDriver, bench: any, name: string): Promise<Prepared> {
  const show = await loadCanonicalCopy(api, bench, name, "default-stage");
  const fixtures = await fixtureIdsByNumber(api);
  const firstCuelist = crypto.randomUUID();
  const secondCuelist = crypto.randomUUID();
  await putObject(api, "cue_list", firstCuelist, cueList(firstCuelist, "Front Cuelist"));
  await putObject(api, "cue_list", secondCuelist, cueList(secondCuelist, "Rear Cuelist"));
  const existingGroup = (await objects<any>(api, "group")).find((candidate) => candidate.id === "1");
  await putObject(api, "group", "1", {
    id: "1",
    name: "Hardware Group",
    fixtures: [fixtures[1]],
    derived_from: null,
    frozen_from: null,
    programming: {},
    master: 1,
    playback_fader: 1,
  }, existingGroup?.revision ?? 0);
  await putObject(api, "playback", "41", playback(41, "Front Cuelist", { type: "cue_list", cue_list_id: firstCuelist }));
  await putObject(api, "playback", "42", playback(42, "Rear Cuelist", { type: "cue_list", cue_list_id: secondCuelist }));
  await putObject(api, "playback", "43", playback(43, "Hardware Group", { type: "group", group_id: "1" }, ["select", "flash", "select_dereferenced"]));
  await putObject(api, "playback_page", "1", { number: 1, name: "Main", slots: { "1": 41, "2": 43 } });
  await putObject(api, "playback_page", "2", { number: 2, name: "Page 2", slots: { "1": 42 } });
  return { firstCuelist, secondCuelist, showId: show.id };
}

function cueList(id: string, name: string) {
  return {
    id,
    name,
    priority: 0,
    mode: "sequence",
    looped: false,
    chaser_step_millis: 1_000,
    speed_group: null,
    cues: [{
      id: crypto.randomUUID(),
      number: 1,
      name: "Opening",
      changes: [],
      group_changes: [],
      fade_millis: 0,
      delay_millis: 0,
      trigger: { type: "manual" },
      phasers: [],
    }],
  };
}

function playback(number: number, name: string, target: any, buttons: [string, string, string] = ["go_minus", "go", "flash"]) {
  return { number, name, target, buttons, button_count: 3, fader: "master", has_fader: true, go_activates: true, auto_off: true, xfade_millis: 0, color: "#20c997", flash_release: "release_all", protect_from_swap: false };
}

async function selectPlayback(api: ApiDriver, number: number) {
  await api.request("POST", `/api/v1/playback-pool/${number}/select`, {});
}

async function playbackState(api: ApiDriver) {
  return api.request<any>("GET", "/api/v1/playbacks");
}

async function openPlaybackMode(page: Page) {
  if (await page.locator(".playback-fader-bank").isVisible()) return;
  await page.locator(".mode-toggle").click();
  await expect(page.locator(".playback-fader-bank")).toBeVisible();
}

async function returnToPlaybackDesk(page: Page) {
  await page.getByRole("button", { name: "DESKTOPS", exact: true }).click();
  await page.locator(".dock-list .dock-entry").first().click();
  await expect(page.locator(".playback-fader-bank")).toBeVisible();
}

async function selectHardwarePage(page: Page, current: number, target: number, name: string) {
  await page.getByRole("button", { name: `Page ${current}`, exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Playback pages" });
  await dialog.getByRole("button", { name: `${target} ${name}`, exact: true }).click();
  await expect(page.getByRole("button", { name: `Page ${target}`, exact: true })).toBeVisible();
}

function playbackCard(page: Page, slot: number): Locator {
  return page.locator(`.playback-fader-bank article[data-playback-slot="${slot}"]`);
}

async function connectHardware(api: ApiDriver, bench: any) {
  const hardware = await bench.osc();
  await hardware.subscribe(`playback-select-${crypto.randomUUID()}`, api.session!.desk.osc_alias);
  await expect.poll(async () => (await api.request<any>("GET", "/api/v1/bootstrap", undefined, false)).hardware_connected).toBe(true);
  return hardware;
}

async function disconnectHardware(hardware: any) {
  await hardware.close();
}
