import type { Locator, Page } from "../apps/control-ui/node_modules/@playwright/test/index.js";
import type { ApiDriver, Session } from "../apps/control-ui/e2e/bench/api";
import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import { pairedScenario } from "../apps/control-ui/e2e/bench/pairedScenario";
import {
  fixtureIdsByNumber,
  loadCanonicalCopy,
  object,
  objects,
  programmer,
  putObject,
} from "./support/catalog";

type Configuration = Record<string, any> & {
  programmer_fade_millis: number;
  sequence_master_fade_millis: number;
  preload_programmer_changes: boolean;
  preload_physical_playback_actions: boolean;
  preload_virtual_playback_actions: boolean;
};

type PlaybackSpec = {
  number: number;
  fixture: number;
  levels?: number[];
  name?: string;
  buttons?: [string, string, string];
  buttonCount?: number;
  hasFader?: boolean;
};

type Prepared = {
  fixtures: Record<number, string>;
  cueLists: Record<number, string>;
};

type Preload003State = Prepared & {
  firstNumber: number;
  secondNumber: number;
  layoutDeskId: string;
};

type PreloadProgrammerPairState = Prepared & {
  groupFixtures: [string, string];
  beforeLevels: [number, number];
  pending?: {
    blind: boolean;
    groupIds: string[];
    groupValues: string[];
    firstFadeMillis: number | null;
    secondFadeMillis: number | null;
    playbackActions: string[];
    liveLevels: [number, number];
  };
  applicationTimestamp?: string;
};

type PreloadPlaybackPairState = Prepared & {
  pendingActions?: string[];
  applicationTimestamp?: string;
  committedState?: ReturnType<typeof summarizePlaybackState>;
  releasedState?: ReturnType<typeof summarizePlaybackState>;
};

type PreloadVirtualPairState = Prepared & {
  pendingActions?: Array<[number, string, string]>;
  applicationTimestamp?: string;
};

type PreloadMaskPairState = {
  savedMasks: Array<[boolean, boolean, boolean]>;
};

type PreloadCombinedPairState = Prepared & {
  groupFixture: string;
  pending?: {
    groupIds: string[];
    playbackActions: Array<[number, string, string]>;
  };
  applicationTimestamp?: string;
};

type VirtualZonePairState = Prepared & {
  savedZones?: Array<{ name: string; slots: number[] }>;
  creationState?: [boolean, boolean];
};

test.describe("docs/testing/06-preload-modes-and-virtual-playbacks.md", () => {
  test.describe.configure({ mode: "serial" });

  pairedScenario<PreloadProgrammerPairState>({
    id: "PRELOAD-001",
    title: "programmer-only Preload is blind, timed from GO, and releasable",
    arrange: async ({ api, bench }, surface) => {
      const prepared = await prepare(api, bench, `preload-001-paired-${surface}`, [
        { number: 31, fixture: 12, levels: [0.4], name: "Live physical sequence", buttons: ["go", "none", "none"], buttonCount: 1, hasFader: false },
      ], { 1: 31 });
      await setCaptureMask(api, true, false, false, 3_000, 7_000);
      const groupFixtures = await distinctGroupFixtures(api, "1", "2");
      return {
        ...prepared,
        groupFixtures,
        beforeLevels: [
          await visualizationLevel(api, groupFixtures[0]),
          await visualizationLevel(api, groupFixtures[1]),
        ],
      };
    },
    api: async ({ api, bench }, state) => {
      await api.command("preload.enter", {});
      await api.command("programmer.execute", { value: "GROUP 1 AT 50" });
      await api.command("programmer.execute", { value: "GROUP 2 AT 70 TIME 1" });
      await poolAction(api, 31, "button", { button: 1, pressed: true, surface: "physical" });
      state.pending = await preloadProgrammerObservation(api, state.groupFixtures);
      state.applicationTimestamp = (await api.command<any>("preload.go", {})).payload!.application_timestamp;
      await bench.tick(3_000);
      await api.command("preload.release", {});
    },
    ui: async ({ api, bench, desk, page }, state) => {
      await desk.open(bench.baseUrl);
      await page.getByRole("button", { name: "PRELOAD", exact: true }).click();
      await desk.command("GROUP 1 AT 50", "G1 AT 50");
      await desk.command("GROUP 2 AT 70 TIME 1", "G2 AT 70 TIME 1");
      await openPlaybackMode(page);
      await playbackCard(page, 1).getByRole("button", { name: "GO +", exact: true }).click();
      state.pending = await preloadProgrammerObservation(api, state.groupFixtures);
      await page.getByRole("button", { name: /^PRELOAD GO\b/ }).click();
      state.applicationTimestamp = (await programmer(api)).preload_group_active["1"].intensity.changed_at;
      await bench.tick(3_000);
      await longPressPreload(page);
    },
    assert: async ({ api }, state) => {
      expect(state.pending).toEqual({
        blind: true,
        groupIds: ["1", "2"],
        groupValues: [],
        firstFadeMillis: 3_000,
        secondFadeMillis: 1_000,
        playbackActions: [],
        liveLevels: state.beforeLevels,
      });
      expect(state.applicationTimestamp).toEqual(expect.any(String));
      expect(await activePlayback(api, 31)).toMatchObject({ enabled: true, current_cue_number: 1 });
      const finalProgrammer = await programmer(api);
      expect(finalProgrammer.preload_group_pending).toEqual({});
      expect(finalProgrammer.preload_group_active).toEqual({});
      expect(await visualizationLevel(api, state.groupFixtures[0])).toBeCloseTo(state.beforeLevels[0], 5);
      expect(await visualizationLevel(api, state.groupFixtures[1])).toBeCloseTo(state.beforeLevels[1], 5);
    },
  });

  test("PRELOAD-001 @supplemental › API timing and source ownership at exact virtual-time checkpoints", async ({ api, bench }) => {
    const prepared = await prepare(api, bench, "preload-001-programmer", [
      { number: 30, fixture: 12, levels: [0.2, 0.8], name: "Live physical sequence" },
    ], { 1: 30 });
    await setCaptureMask(api, true, false, false, 3_000, 7_000);
    const [group1Fixture, group2Fixture] = await distinctGroupFixtures(api, "1", "2");
    const before1 = await visualizationLevel(api, group1Fixture);
    const before2 = await visualizationLevel(api, group2Fixture);

    await api.command("preload.enter", {});
    await api.command("programmer.execute", { value: "GROUP 1 AT 50" });
    await api.command("programmer.execute", { value: "GROUP 2 AT 70 TIME 1" });
    const pending = await programmer(api);
    expect(pending.blind).toBe(true);
    expect(pending.preload_group_pending["1"].intensity.fade_millis).toBe(3_000);
    expect(pending.preload_group_pending["2"].intensity).toMatchObject({ fade_millis: 1_000 });
    expect(pending.group_values).toEqual({});
    expect(await visualizationLevel(api, group1Fixture)).toBeCloseTo(before1, 5);
    expect(await visualizationLevel(api, group2Fixture)).toBeCloseTo(before2, 5);

    await poolAction(api, 30, "go", { surface: "physical" });
    expect((await activePlayback(api, 30))?.current_cue_number).toBe(1);
    expect((await programmer(api)).preload_playback_pending).toEqual([]);
    await bench.tick(2_000);
    expect(await visualizationLevel(api, group1Fixture)).toBeCloseTo(before1, 5);
    expect(await visualizationLevel(api, group2Fixture)).toBeCloseTo(before2, 5);

    const committed = (await api.command<any>("preload.go", {})).payload!;
    const activeProgrammer = await programmer(api);
    expect(activeProgrammer.preload_group_active["1"].intensity.changed_at).toBe(committed.application_timestamp);
    expect(activeProgrammer.preload_group_active["2"].intensity.changed_at).toBe(committed.application_timestamp);
    expect(committed.playback_actions).toEqual([]);
    await bench.tick(1_000);
    expect(await visualizationLevel(api, group1Fixture)).toBeCloseTo(1 / 6, 2);
    expect(await visualizationLevel(api, group2Fixture)).toBeCloseTo(0.7, 2);
    await bench.tick(2_000);
    expect(await visualizationLevel(api, group1Fixture)).toBeCloseTo(0.5, 2);
    expect(await visualizationLevel(api, group2Fixture)).toBeCloseTo(0.7, 2);

    expect((await api.command<any>("preload.release", {})).payload).toMatchObject({ released: true });
    expect(await visualizationLevel(api, group1Fixture)).toBeCloseTo(before1, 5);
    expect(await visualizationLevel(api, group2Fixture)).toBeCloseTo(before2, 5);
    expect((await activePlayback(api, 30))?.current_cue_number).toBe(1);
    expect(prepared.fixtures[12]).toBeTruthy();
  });

  test("PRELOAD-001 @supplemental-ui › the command line exposes detailed pending programmer timing", async ({ api, bench, desk, page }) => {
    await prepare(api, bench, "preload-001-ui", [
      { number: 31, fixture: 12, levels: [0.4], name: "Live GO", buttons: ["go", "none", "none"], buttonCount: 1, hasFader: false },
    ], { 1: 31 });
    await setCaptureMask(api, true, false, false, 3_000, 7_000);
    await desk.open(bench.baseUrl);
    await desk.recordStep("ARM PROGRAMMER PRELOAD", "Only programmer changes are blind; the physical GO remains live.");
    await page.getByRole("button", { name: "PRELOAD", exact: true }).click();
    await desk.command("GROUP 1 AT 50", "G1 AT 50");
    await desk.command("GROUP 2 AT 70 TIME 1", "G2 AT 70 TIME 1");
    await expect(page.getByLabel(/Pending Preload: PROG 2/)).toBeVisible();
    await openPlaybackMode(page);
    await playbackCard(page, 1).getByRole("button", { name: "GO +", exact: true }).click();
    await expect.poll(async () => (await activePlayback(api, 31))?.current_cue_number).toBe(1);
    await desk.recordStep("COMMIT AT ONE MARK", "The explicit 1 s value and the 3 s Programmer Fade fallback start now.");
    await page.getByRole("button", { name: /^PRELOAD GO\b/ }).click();
    await bench.tick(3_000);
    const state = await programmer(api);
    expect(state.preload_group_active["1"].intensity.value.value).toBeCloseTo(0.5, 5);
    expect(state.preload_group_active["2"].intensity.fade_millis).toBe(1_000);
  });

  pairedScenario<PreloadPlaybackPairState>({
    id: "PRELOAD-002",
    title: "physical-playback-only Preload preserves the seven ordered action verbs",
    arrange: async ({ api, bench }, surface) => {
      const actions = ["toggle", "go", "go_minus", "off", "on", "temp"] as const;
      const specs = actions.map((action, index): PlaybackSpec => ({
        number: index + 1,
        fixture: index + 3,
        levels: [0.3, 0.7],
        name: `Paired ${action}`,
        buttons: [action, "none", "none"],
        buttonCount: 1,
        hasFader: false,
      }));
      const prepared = await prepare(api, bench, `preload-002-paired-${surface}`, specs, Object.fromEntries(specs.map((spec, index) => [index + 1, spec.number])));
      await setCaptureMask(api, false, true, false, 2_000, 7_000);
      await poolAction(api, 3, "go");
      await poolAction(api, 3, "go");
      await poolAction(api, 4, "go");
      return prepared;
    },
    api: async ({ api, bench }, state) => {
      await api.command("preload.enter", {});
      for (const [number, action] of [
        [1, "toggle"], [2, "go"], [3, "go-minus"], [4, "off"], [5, "on"], [6, "temp-on"], [6, "temp-off"],
      ] as const)
        await poolAction(api, number, action, { surface: "physical" });
      state.pendingActions = (await programmer(api)).preload_playback_pending.map((entry: any) => entry.action);
      state.applicationTimestamp = (await api.command<any>("preload.go", {})).payload!.application_timestamp;
      await bench.tick(2_000);
      state.committedState = summarizePlaybackState(await playbacks(api), [1, 2, 3, 4, 5, 6]);
      await api.command("preload.release", {});
      state.releasedState = summarizePlaybackState(await playbacks(api), [1, 2, 3, 4, 5, 6]);
    },
    ui: async ({ api, bench, desk, page }, state) => {
      await desk.open(bench.baseUrl);
      await openPlaybackMode(page);
      await page.getByRole("button", { name: "PRELOAD", exact: true }).click();
      for (const [slot, label] of [[1, "TOGGLE"], [2, "GO +"], [3, "GO −"], [4, "OFF"], [5, "ON"], [6, "TEMP"], [6, "TEMP"]] as const)
        await playbackCard(page, slot).getByRole("button", { name: label, exact: true }).click();
      state.pendingActions = (await programmer(api)).preload_playback_pending.map((entry: any) => entry.action);
      await page.getByRole("button", { name: /^PRELOAD GO\b/ }).click();
      state.applicationTimestamp = (await activePlayback(api, 1))?.activated_at;
      await bench.tick(2_000);
      state.committedState = summarizePlaybackState(await playbacks(api), [1, 2, 3, 4, 5, 6]);
      await longPressPreload(page);
      state.releasedState = summarizePlaybackState(await playbacks(api), [1, 2, 3, 4, 5, 6]);
    },
    assert: async ({ api }, state) => {
      expect(state.pendingActions).toEqual(["toggle", "go", "go-minus", "off", "on", "temp-on", "temp-off"]);
      expect(state.applicationTimestamp).toEqual(expect.any(String));
      expect(await activePlayback(api, 1)).toMatchObject({ enabled: true, current_cue_number: 1, activated_at: state.applicationTimestamp });
      expect(await activePlayback(api, 2)).toMatchObject({ enabled: true, current_cue_number: 1, activated_at: state.applicationTimestamp });
      expect(await activePlayback(api, 3)).toMatchObject({ enabled: true, current_cue_number: 1 });
      expect(await activePlayback(api, 4)).toMatchObject({ enabled: false });
      expect(await activePlayback(api, 5)).toMatchObject({ enabled: true });
      expect((await activePlayback(api, 6))?.temporary_active ?? false).toBe(false);
      expect(state.releasedState).toEqual(state.committedState);
    },
  });

  test("PRELOAD-002 @supplemental › API queue ordering, live Flash/fader exclusions, and timing", async ({ api, bench }) => {
    const specs: PlaybackSpec[] = Array.from({ length: 9 }, (_, index) => ({
      number: index + 1,
      fixture: index + 3,
      levels: [0.25, 0.5, 0.75, 1],
      name: `Physical ${index + 1}`,
      buttons: index === 7 ? ["go", "go_minus", "flash"] : ["go", "none", "none"],
      buttonCount: index === 7 ? 3 : 1,
      hasFader: index === 7,
    }));
    const prepared = await prepare(api, bench, "preload-002-physical", specs, Object.fromEntries(specs.slice(0, 8).map((spec, index) => [index + 1, spec.number])));
    await setCaptureMask(api, false, true, false, 2_000, 7_000);
    await poolAction(api, 2, "go"); await poolAction(api, 2, "go");
    await poolAction(api, 3, "go"); await poolAction(api, 3, "off");
    await poolAction(api, 4, "go");
    await poolAction(api, 5, "go"); await poolAction(api, 5, "off");
    await poolAction(api, 7, "temp-on");
    await poolAction(api, 9, "go");

    await api.command("preload.enter", {});
    await api.command("programmer.execute", { value: "GROUP 1 AT 40" });
    expect((await programmer(api)).preload_group_pending).toEqual({});
    expect((await programmer(api)).group_values["1"]).toBeDefined();
    // The disabled-domain assertion is complete; clear its live value so the playback timing
    // checkpoint below measures only the queued GO result.
    await api.command("programmer.clear", {});
    const verbs = [
      [1, "go"], [2, "go-minus"], [3, "on"], [4, "off"], [5, "toggle"], [6, "temp-on"], [7, "temp-off"],
    ] as const;
    for (const [number, action] of verbs) await poolAction(api, number, action, { surface: "physical" });
    expect((await programmer(api)).preload_playback_pending.map((entry: any) => entry.action)).toEqual(verbs.map(([, action]) => action));

    await poolAction(api, 8, "button", { button: 3, pressed: true, surface: "physical" });
    expect((await activePlayback(api, 8))?.flash).toBe(true);
    await poolAction(api, 8, "button", { button: 3, pressed: false, surface: "physical" });
    await poolAction(api, 8, "master", { value: 0.4, surface: "physical" });
    expect(await activePlayback(api, 8)).toMatchObject({ enabled: true });
    expect((await activePlayback(api, 8))?.fader_position).toBeCloseTo(0.4, 5);
    expect((await programmer(api)).preload_playback_pending.map((entry: any) => entry.action)).toEqual(verbs.map(([, action]) => action));

    await poolAction(api, 9, "go", { surface: "physical" });
    await poolAction(api, 9, "go", { surface: "physical" });
    await poolAction(api, 9, "go", { surface: "virtual" });
    expect((await activePlayback(api, 9))?.current_cue_number).toBe(2);
    expect((await programmer(api)).preload_playback_pending.slice(-2).map((entry: any) => entry.action)).toEqual(["go", "go"]);

    await bench.tick(100);
    const committed = (await api.command<any>("preload.go", {})).payload!;
    expect(committed.programmer_fade_millis).toBe(2_000);
    expect(committed.playback_actions.map((entry: any) => entry.action)).toEqual([...verbs.map(([, action]) => action), "go", "go"]);
    expect((await activePlayback(api, 1))?.activated_at).toBe(committed.application_timestamp);
    await bench.tick(1_000);
    expect(await visualizationLevel(api, prepared.fixtures[3])).toBeCloseTo(0.125, 2);
    await bench.tick(1_000);
    expect(await activePlayback(api, 1)).toMatchObject({ enabled: true, current_cue_number: 1 });
    expect(await activePlayback(api, 2)).toMatchObject({ enabled: true, current_cue_number: 1 });
    expect(await activePlayback(api, 3)).toMatchObject({ enabled: true });
    expect(await activePlayback(api, 4)).toMatchObject({ enabled: false });
    expect(await activePlayback(api, 5)).toMatchObject({ enabled: true });
    expect(await activePlayback(api, 6)).toMatchObject({ temporary_active: true });
    // Temp off against an otherwise inactive playback removes the transient runtime entry.
    expect(await activePlayback(api, 7)).toBeUndefined();
    expect(await activePlayback(api, 9)).toMatchObject({ current_cue_number: 4 });

    const playbackState = summarizePlaybackState(await playbacks(api), [1, 2, 3, 4, 5, 6, 7, 9]);
    await api.command("preload.release", {});
    expect(summarizePlaybackState(await playbacks(api), [1, 2, 3, 4, 5, 6, 7, 9])).toEqual(playbackState);
  });

  test("PRELOAD-002 @supplemental-ui › physical controls expose all verbs and live exclusions", async ({ api, bench, desk, page }) => {
    const actions = ["toggle", "go", "go_minus", "off", "on", "temp"] as const;
    const specs = actions.map((action, index): PlaybackSpec => ({
      number: index + 1,
      fixture: index + 3,
      levels: [0.3, 0.7],
      name: `UI ${action}`,
      buttons: index === 0 ? [action, "flash", "none"] : [action, "none", "none"],
      buttonCount: index === 0 ? 2 : 1,
      hasFader: index === 0,
    }));
    await prepare(api, bench, "preload-002-ui", specs, Object.fromEntries(specs.map((spec, index) => [index + 1, spec.number])));
    await setCaptureMask(api, false, true, false, 2_000, 7_000);
    await poolAction(api, 3, "go"); await poolAction(api, 3, "go");
    await poolAction(api, 4, "go");
    await poolAction(api, 5, "go"); await poolAction(api, 5, "off");
    await desk.open(bench.baseUrl);
    await openPlaybackMode(page);
    await page.getByRole("button", { name: "PRELOAD", exact: true }).click();
    await desk.recordStep("QUEUE PHYSICAL VERBS", "Toggle, GO, GO minus, Off, On, Temp press, and Temp release are retained in operator order.");
    for (const [slot, label] of [[1, "TOGGLE"], [2, "GO +"], [3, "GO −"], [4, "OFF"], [5, "ON"]] as const)
      await playbackCard(page, slot).getByRole("button", { name: label, exact: true }).click();
    const temp = playbackCard(page, 6).getByRole("button", { name: "TEMP", exact: true });
    await temp.click();
    await expect.poll(async () => (await programmer(api)).preload_playback_pending.at(-1)?.action).toBe("temp-on");
    await temp.click();
    const pending = await programmer(api);
    expect(pending.preload_playback_pending.map((entry: any) => entry.action)).toEqual(["toggle", "go", "go-minus", "off", "on", "temp-on", "temp-off"]);
    const flash = playbackCard(page, 1).getByRole("button", { name: "FLASH", exact: true });
    await flash.hover(); await page.mouse.down(); await page.mouse.up();
    await playbackCard(page, 1).getByRole("slider", { name: "Master" }).fill("40");
    expect((await programmer(api)).preload_playback_pending).toHaveLength(7);
    await expect(page.getByLabel(/Pending Preload: .*TOGGLE 1.*GO 2.*GO MINUS 3.*OFF 4.*ON 5.*TEMP ON 6.*TEMP OFF 6/)).toBeVisible();
    await page.getByRole("button", { name: /^PRELOAD GO\b/ }).click();
    await bench.tick(2_000);
    expect(await activePlayback(api, 4)).toMatchObject({ enabled: false });
    expect(await activePlayback(api, 5)).toMatchObject({ enabled: true });
  });

  pairedScenario<Preload003State>({
    id: "PRELOAD-003",
    title: "Virtual Playbacks use a persisted pane-native 2×2 grid and real GO/TOGGLE playbacks",
    arrange: async ({ api, bench }, surface) => {
      const specs: PlaybackSpec[] = [
        { number: 101, fixture: 3, levels: [0.2, 0.8], name: "Virtual Source A", buttons: ["go", "none", "none"], buttonCount: 1, hasFader: false },
        { number: 102, fixture: 4, levels: [0.3, 0.9], name: "Virtual Source B", buttons: ["go", "none", "none"], buttonCount: 1, hasFader: false },
      ];
      const prepared = await prepare(api, bench, `preload-003-virtual-${surface}`, specs, {});
      return { ...prepared, firstNumber: 101, secondNumber: 102, layoutDeskId: `preload-003-${surface}` };
    },
    api: async ({ api }, state) => {
      const layoutId = api.session!.user.id;
      const existing = (await objects<any>(api, "user_layout")).find((entry) => entry.id === layoutId);
      await putObject(api, "user_layout", layoutId, {
        desks: [{
          id: state.layoutDeskId,
          name: "Virtual Playback Desktop",
          panes: [{
            id: "virtual-playbacks-api",
            kind: "virtual_playbacks",
            title: "Virtual Playbacks",
            x: 1,
            y: 1,
            width: 12,
            height: 10,
            virtualPlaybackRows: 2,
            virtualPlaybackColumns: 2,
          }],
        }],
        activeDeskId: state.layoutDeskId,
      }, existing?.revision ?? 0);
      await writePage(api, 1, { "1": state.firstNumber, "2": state.secondNumber });
      const second = await object<any>(api, "playback", String(state.secondNumber));
      await putObject(api, "playback", String(state.secondNumber), {
        ...second.body,
        buttons: ["toggle", "none", "none"],
      }, second.revision);
      await poolAction(api, state.firstNumber, "button", { button: 1, pressed: true, surface: "virtual" });
      await poolAction(api, state.secondNumber, "button", { button: 1, pressed: true, surface: "virtual" });
      expect(await activePlayback(api, state.firstNumber)).toMatchObject({ enabled: true, current_cue_number: 1 });
      expect(await activePlayback(api, state.secondNumber)).toMatchObject({ enabled: true, current_cue_number: 1 });
      const bootstrap = await api.request<any>("GET", "/api/v1/bootstrap", undefined, false);
      await api.request("POST", `/api/v1/shows/${bootstrap.active_show.id}/open`, { transition: "hold_current" });
    },
    ui: async ({ api, bench, desk, page }, state) => {
      await desk.open(bench.baseUrl);
      await desk.recordStep("CREATE VIRTUAL PLAYBACK PANE", "Add a normal configurable pane and set its grid to two rows by two columns.");
      let pane = await addVirtualPlaybackPane(page);
      await pane.getByRole("button", { name: "Settings", exact: true }).click();
      const settings = page.getByRole("dialog", { name: "Pane Settings" });
      await settings.getByRole("tab", { name: "Virtual Playbacks", exact: true }).click();
      await settings.getByLabel("Rows").fill("2");
      await settings.getByLabel("Columns").fill("2");
      await settings.getByRole("button", { name: "Close settings" }).click();
      await expect(pane.locator(".virtual-playback-cell")).toHaveCount(4);

      await assignVirtualSource(page, pane, "Virtual Source A", 1);
      pane = await activeVirtualPane(page);
      await assignVirtualSource(page, pane, "Virtual Source B", 2);
      pane = await activeVirtualPane(page);
      const pageState = await pageObject(api, 1);
      state.firstNumber = pageState.body.slots["1"];
      state.secondNumber = pageState.body.slots["2"];

      await page.getByRole("button", { name: "SET", exact: true }).click();
      await pane.getByRole("button", { name: /Virtual playback page 1 cell 2/ }).click();
      const modal = page.getByRole("dialog", { name: "Playback Configuration" });
      await expect(modal).toHaveAttribute("data-topology", "1 button · faderless");
      await modal.getByRole("button", { name: "Playback Layout", exact: true }).click();
      await chooseSelect(page, modal, "Top button", "Toggle");
      await modal.getByRole("button", { name: "Apply", exact: true }).click();
      await expect(modal).toBeHidden();
      await pane.getByRole("button", { name: /Virtual playback page 1 cell 1/ }).click();
      await pane.getByRole("button", { name: /Virtual playback page 1 cell 2/ }).click();
      await expect.poll(async () => (await activePlayback(api, state.firstNumber))?.enabled).toBe(true);
      await expect.poll(async () => (await activePlayback(api, state.secondNumber))?.enabled).toBe(true);

      await page.waitForTimeout(900);
      await page.reload();
      await expect(page.locator(".connection-cover")).toBeHidden({ timeout: 10_000 });
      pane = await activeVirtualPane(page);
      await expect(pane.locator(".virtual-playback-cell")).toHaveCount(4);
      await expect(pane.getByRole("button", { name: /Virtual playback page 1 cell 1/ })).toContainText("GO");
      await expect(pane.getByRole("button", { name: /Virtual playback page 1 cell 2/ })).toContainText("TOGGLE");
    },
    assert: async ({ api }, state) => {
      const pageState = await pageObject(api, 1);
      expect(pageState.body.slots).toMatchObject({ "1": state.firstNumber, "2": state.secondNumber });
      expect(await object<any>(api, "playback", String(state.firstNumber))).toMatchObject({ body: { button_count: 1, has_fader: false, buttons: ["go", "none", "none"] } });
      expect(await object<any>(api, "playback", String(state.secondNumber))).toMatchObject({ body: { button_count: 1, has_fader: false, buttons: ["toggle", "none", "none"] } });
      const layouts = await objects<any>(api, "user_layout");
      const pane = layouts.flatMap((layout) => layout.body.desks ?? [])
        .flatMap((desk: any) => desk.panes ?? [])
        .find((candidate: any) => candidate.kind === "virtual_playbacks");
      expect(pane).toEqual(expect.objectContaining({ virtualPlaybackRows: 2, virtualPlaybackColumns: 2 }));
    },
  });

  pairedScenario<PreloadVirtualPairState>({
    id: "PRELOAD-004",
    title: "virtual GO and TOGGLE alone remain pending and share Programmer Fade",
    arrange: async ({ api, bench }, surface) => {
      const prepared = await prepare(api, bench, `preload-004-paired-${surface}`, [
        { number: 44, fixture: 3, levels: [1], name: "Virtual GO", buttons: ["go", "none", "none"], buttonCount: 1, hasFader: false },
        { number: 45, fixture: 4, levels: [0.8], name: "Virtual TOGGLE", buttons: ["toggle", "none", "none"], buttonCount: 1, hasFader: false },
        { number: 46, fixture: 5, levels: [0.6], name: "Physical live", buttons: ["go", "none", "none"], buttonCount: 1, hasFader: false },
      ], { 1: 44, 2: 45, 3: 46 });
      await setCaptureMask(api, false, false, true, 2_500, 8_000);
      return prepared;
    },
    api: async ({ api, bench }, state) => {
      await api.command("preload.enter", {});
      // Keep the disabled-domain programmer proof on a distinct fixture: programmer priority is
      // intentionally higher than these Cuelists and must not mask the playback fade under test.
      await api.command("programmer.execute", { value: "FIXTURE 1 AT 35" });
      await poolAction(api, 46, "button", { button: 1, pressed: true, surface: "physical" });
      await poolAction(api, 44, "button", { button: 1, pressed: true, surface: "virtual" });
      await poolAction(api, 45, "button", { button: 1, pressed: true, surface: "virtual" });
      state.pendingActions = playbackPendingObservation(await programmer(api));
      state.applicationTimestamp = (await api.command<any>("preload.go", {})).payload!.application_timestamp;
      await bench.tick(2_500);
      await api.command("preload.release", {});
    },
    ui: async ({ api, bench, desk, page }, state) => {
      await desk.open(bench.baseUrl);
      const pane = await addVirtualPlaybackPane(page);
      await page.getByRole("button", { name: "PRELOAD", exact: true }).click();
      await desk.command("1 AT 35", "F1 AT 35");
      await openPlaybackMode(page);
      await playbackCard(page, 3).getByRole("button", { name: "GO +", exact: true }).click();
      await pane.getByRole("button", { name: /Virtual playback page 1 cell 1 Virtual GO/ }).click();
      await pane.getByRole("button", { name: /Virtual playback page 1 cell 2 Virtual TOGGLE/ }).click();
      state.pendingActions = playbackPendingObservation(await programmer(api));
      await page.getByRole("button", { name: /^PRELOAD GO\b/ }).click();
      state.applicationTimestamp = (await activePlayback(api, 44))?.activated_at;
      await bench.tick(2_500);
      await longPressPreload(page);
    },
    assert: async ({ api }, state) => {
      expect(state.pendingActions).toEqual([[44, "go", "virtual"], [45, "toggle", "virtual"]]);
      expect(state.applicationTimestamp).toEqual(expect.any(String));
      expect(await activePlayback(api, 44)).toMatchObject({ enabled: true, current_cue_number: 1, activated_at: state.applicationTimestamp });
      expect(await activePlayback(api, 45)).toMatchObject({ enabled: true, current_cue_number: 1, activated_at: state.applicationTimestamp });
      expect(await activePlayback(api, 46)).toMatchObject({ enabled: true, current_cue_number: 1 });
      const finalProgrammer = await programmer(api);
      expect(finalProgrammer.preload_group_pending).toEqual({});
      expect(finalProgrammer.preload_group_active).toEqual({});
      expect(finalProgrammer.values).toEqual(expect.arrayContaining([
        expect.objectContaining({ fixture_id: state.fixtures[1], attribute: "intensity" }),
      ]));
      expect(await visualizationLevel(api, state.fixtures[1])).toBeCloseTo(0.35, 2);
      expect(await visualizationLevel(api, state.fixtures[3])).toBeCloseTo(1, 2);
      expect(await visualizationLevel(api, state.fixtures[4])).toBeCloseTo(0.8, 2);
    },
  });

  test("PRELOAD-004 @supplemental › API disabled-domain behavior and exact virtual transition timing", async ({ api, bench }) => {
    const prepared = await prepare(api, bench, "preload-004-virtual-api", [
      { number: 41, fixture: 3, levels: [1], buttons: ["go", "none", "none"], buttonCount: 1, hasFader: false },
      { number: 42, fixture: 4, levels: [0.8], buttons: ["toggle", "none", "none"], buttonCount: 1, hasFader: false },
      { number: 43, fixture: 5, levels: [0.6], buttons: ["go", "none", "none"], buttonCount: 1, hasFader: false },
    ], { 1: 41, 2: 42, 3: 43 });
    await setCaptureMask(api, false, false, true, 2_500, 8_000);
    await api.command("preload.enter", {});
    await api.command("programmer.execute", { value: "GROUP 1 AT 35" });
    await poolAction(api, 43, "button", { button: 1, pressed: true, surface: "physical" });
    await poolAction(api, 41, "button", { button: 1, pressed: true, surface: "virtual" });
    await poolAction(api, 42, "button", { button: 1, pressed: true, surface: "virtual" });
    const pending = await programmer(api);
    expect(pending.preload_group_pending).toEqual({});
    expect(pending.preload_playback_pending.map((entry: any) => [entry.action, entry.surface])).toEqual([["go", "virtual"], ["toggle", "virtual"]]);
    expect(await activePlayback(api, 41)).toBeUndefined();
    expect(await activePlayback(api, 42)).toBeUndefined();
    expect(await activePlayback(api, 43)).toMatchObject({ enabled: true });
    // The disabled programmer domain was proven live above; remove it before measuring the
    // independently captured virtual playback transition.
    await api.command("programmer.clear", {});
    await bench.tick(100);
    const committed = (await api.command<any>("preload.go", {})).payload!;
    expect(committed.playback_actions.every((entry: any) => entry.fallback_millis === 2_500)).toBe(true);
    expect((await activePlayback(api, 41))?.activated_at).toBe(committed.application_timestamp);
    expect((await activePlayback(api, 42))?.activated_at).toBe(committed.application_timestamp);
    await bench.tick(2_500);
    expect(await visualizationLevel(api, prepared.fixtures[3])).toBeCloseTo(1, 2);
    expect(await visualizationLevel(api, prepared.fixtures[4])).toBeCloseTo(0.8, 2);
    await api.command("preload.release", {});
    expect(await activePlayback(api, 41)).toMatchObject({ enabled: true });
    expect(await activePlayback(api, 42)).toMatchObject({ enabled: true });
  });

  test("PRELOAD-004 @supplemental-ui › virtual cells expose detailed pending feedback and release behavior", async ({ api, bench, desk, page }) => {
    await prepare(api, bench, "preload-004-virtual-ui", [
      { number: 44, fixture: 3, levels: [1], name: "Virtual GO", buttons: ["go", "none", "none"], buttonCount: 1, hasFader: false },
      { number: 45, fixture: 4, levels: [0.8], name: "Virtual TOGGLE", buttons: ["toggle", "none", "none"], buttonCount: 1, hasFader: false },
      { number: 46, fixture: 5, levels: [0.6], name: "Physical live", buttons: ["go", "none", "none"], buttonCount: 1, hasFader: false },
    ], { 1: 44, 2: 45, 3: 46 });
    await setCaptureMask(api, false, false, true, 2_500, 8_000);
    await desk.open(bench.baseUrl);
    const pane = await addVirtualPlaybackPane(page);
    await page.getByRole("button", { name: "PRELOAD", exact: true }).click();
    await api.command("programmer.execute", { value: "GROUP 1 AT 35" });
    await poolAction(api, 46, "button", { button: 1, pressed: true, surface: "physical" });
    await desk.recordStep("QUEUE VIRTUAL CELLS", "Click the real GO and TOGGLE cells; their underlying playbacks must remain unchanged.");
    await pane.getByRole("button", { name: /Virtual playback page 1 cell 1 Virtual GO/ }).click();
    await pane.getByRole("button", { name: /Virtual playback page 1 cell 2 Virtual TOGGLE/ }).click();
    await expect(page.getByLabel(/Pending Preload: .*GO 44.*TOGGLE 45/)).toBeVisible();
    expect(await activePlayback(api, 44)).toBeUndefined();
    expect(await activePlayback(api, 45)).toBeUndefined();
    expect(await activePlayback(api, 46)).toMatchObject({ enabled: true });
    await page.getByRole("button", { name: /^PRELOAD GO\b/ }).click();
    await bench.tick(2_500);
    expect(await activePlayback(api, 44)).toMatchObject({ enabled: true });
    expect(await activePlayback(api, 45)).toMatchObject({ enabled: true });
    await longPressPreload(page);
    expect(await activePlayback(api, 44)).toMatchObject({ enabled: true });
    expect(await activePlayback(api, 45)).toMatchObject({ enabled: true });
  });

  pairedScenario<PreloadMaskPairState>({
    id: "PRELOAD-005",
    title: "all eight capture-domain switch masks persist independently",
    arrange: async ({ api, bench }, surface) => {
      await loadCanonicalCopy(api, bench, `preload-005-paired-${surface}`);
      return { savedMasks: [] };
    },
    api: async ({ api }, state) => {
      for (let mask = 0; mask < 8; mask++) {
        await setCaptureMask(api, Boolean(mask & 1), Boolean(mask & 2), Boolean(mask & 4));
        state.savedMasks.push(captureMask(await configuration(api)));
      }
    },
    ui: async ({ api, bench, desk, page }, state) => {
      await desk.open(bench.baseUrl);
      await openPreloadInputSettings(page);
      for (let mask = 0; mask < 8; mask++) {
        await setPreloadMaskThroughUi(api, page, mask);
        state.savedMasks.push(captureMask(await configuration(api)));
        await page.getByRole("button", { name: "Outputs", exact: true }).click();
        await page.getByRole("button", { name: "Inputs", exact: true }).click();
        await expectPreloadMaskControls(page, mask);
      }
    },
    assert: async ({ api }, state) => {
      const expected = Array.from({ length: 8 }, (_, mask): [boolean, boolean, boolean] => [Boolean(mask & 1), Boolean(mask & 2), Boolean(mask & 4)]);
      expect(state.savedMasks).toEqual(expected);
      expect(captureMask(await configuration(api))).toEqual([true, true, true]);
    },
  });

  test("PRELOAD-005 @supplemental › every mask keeps disabled domains live and enabled domains blind", async ({ api, bench }) => {
    const rows: Array<[boolean, boolean, boolean]> = Array.from({ length: 8 }, (_, mask) => [Boolean(mask & 1), Boolean(mask & 2), Boolean(mask & 4)]);
    for (const [programmerCapture, physicalCapture, virtualCapture] of rows) {
      await prepare(api, bench, `preload-005-${Number(programmerCapture)}${Number(physicalCapture)}${Number(virtualCapture)}`, [
        { number: 51, fixture: 3, levels: [0.6], buttons: ["go", "none", "none"], buttonCount: 1, hasFader: false },
        { number: 52, fixture: 4, levels: [0.8], buttons: ["go", "none", "none"], buttonCount: 1, hasFader: false },
      ], { 1: 51, 2: 52 });
      await setCaptureMask(api, programmerCapture, physicalCapture, virtualCapture, 1_250, 9_000);
      const saved = await configuration(api);
      expect([saved.preload_programmer_changes, saved.preload_physical_playback_actions, saved.preload_virtual_playback_actions]).toEqual([programmerCapture, physicalCapture, virtualCapture]);
      await api.command("preload.enter", {});
      await api.command("programmer.execute", { value: "GROUP 1 AT 45" });
      await poolAction(api, 51, "button", { button: 1, pressed: true, surface: "physical" });
      await poolAction(api, 52, "button", { button: 1, pressed: true, surface: "virtual" });
      const pending = await programmer(api);
      expect(Boolean(pending.preload_group_pending["1"])).toBe(programmerCapture);
      expect(Boolean(pending.group_values["1"])).toBe(!programmerCapture);
      expect(pending.preload_playback_pending.map((entry: any) => entry.surface)).toEqual([
        ...(physicalCapture ? ["physical"] : []),
        ...(virtualCapture ? ["virtual"] : []),
      ]);
      expect(Boolean(await activePlayback(api, 51))).toBe(!physicalCapture);
      expect(Boolean(await activePlayback(api, 52))).toBe(!virtualCapture);
      await bench.tick(100);
      const committed = (await api.command<any>("preload.go", {})).payload!;
      expect(committed.playback_actions).toHaveLength(Number(physicalCapture) + Number(virtualCapture));
      const after = await programmer(api);
      expect(Boolean(after.preload_group_active["1"])).toBe(programmerCapture);
      expect(after.preload_group_pending).toEqual({});
      expect(await activePlayback(api, 51)).toMatchObject({ enabled: true });
      expect(await activePlayback(api, 52)).toMatchObject({ enabled: true });
      if (physicalCapture) expect((await activePlayback(api, 51))?.activated_at).toBe(committed.application_timestamp);
      if (virtualCapture) expect((await activePlayback(api, 52))?.activated_at).toBe(committed.application_timestamp);
    }
  });

  test("PRELOAD-005 @supplemental-ui › Settings visibly reloads every independent switch mask", async ({ api, bench, desk, page }) => {
    await loadCanonicalCopy(api, bench, "preload-005-settings");
    await desk.open(bench.baseUrl);
    await page.getByRole("button", { name: /Open show menu/ }).click();
    await page.getByRole("button", { name: "Enter Setup", exact: true }).click();
    await page.getByRole("button", { name: "Inputs", exact: true }).click();
    const labels = ["Preload programmer changes", "Preload physical playback actions", "Preload virtual playback actions"] as const;
    for (let mask = 0; mask < 8; mask++) {
      await desk.recordStep(`SAVE MASK ${mask + 1} / 8`, labels.map((label, index) => `${label.replace("Preload ", "")}: ${mask & (1 << index) ? "On" : "Off"}`).join(" · "));
      for (let index = 0; index < labels.length; index++) {
        const desired = Boolean(mask & (1 << index));
        const control = page.getByRole("switch", { name: labels[index] });
        if ((await control.isChecked()) !== desired)
          await control.locator("..").locator(".ui-switch-track").click();
      }
      await page.getByRole("button", { name: "Save changes", exact: true }).click();
      await expect.poll(async () => {
        const current = await configuration(api);
        return [current.preload_programmer_changes, current.preload_physical_playback_actions, current.preload_virtual_playback_actions];
      }).toEqual([Boolean(mask & 1), Boolean(mask & 2), Boolean(mask & 4)]);
      await page.getByRole("button", { name: "Outputs", exact: true }).click();
      await page.getByRole("button", { name: "Inputs", exact: true }).click();
      for (let index = 0; index < labels.length; index++)
        await expect(page.getByLabel(labels[index])).toBeChecked({ checked: Boolean(mask & (1 << index)) });
    }
  });

  pairedScenario<PreloadCombinedPairState>({
    id: "PRELOAD-006",
    title: "combined Preload commits atomically and releases only programmer data",
    arrange: async ({ api, bench }, surface) => {
      await loadCanonicalCopy(api, bench, `preload-006-paired-${surface}`);
      const fixtures = await fixtureIdsByNumber(api);
      const groupFixture = await firstGroupFixture(api, "1");
      const groupFixtureNumber = Number(Object.entries(fixtures).find(([, id]) => id === groupFixture)?.[0]);
      const prepared = await installOnCurrentShow(api, fixtures, [
        { number: 60, fixture: groupFixtureNumber, levels: [0.25], name: "Underlying source" },
        { number: 61, fixture: 3, levels: [0.6], name: "Physical combined", buttons: ["go", "none", "none"], buttonCount: 1, hasFader: false },
        { number: 62, fixture: 4, levels: [0.8], name: "Virtual combined", buttons: ["toggle", "none", "none"], buttonCount: 1, hasFader: false },
      ], { 1: 61, 2: 62 });
      await setCaptureMask(api, true, true, true, 1_500, 8_000);
      await poolAction(api, 60, "go");
      await bench.tick(8_000);
      return { ...prepared, groupFixture };
    },
    api: async ({ api, bench }, state) => {
      await api.command("preload.enter", {});
      await api.command("programmer.execute", { value: "GROUP 1 AT 80" });
      await poolAction(api, 61, "button", { button: 1, pressed: true, surface: "physical" });
      await poolAction(api, 62, "button", { button: 1, pressed: true, surface: "virtual" });
      state.pending = preloadCombinedObservation(await programmer(api));
      state.applicationTimestamp = (await api.command<any>("preload.go", {})).payload!.application_timestamp;
      await bench.tick(1_500);
      await api.command("preload.release", {});
    },
    ui: async ({ api, bench, desk, page }, state) => {
      await desk.open(bench.baseUrl);
      const pane = await addVirtualPlaybackPane(page);
      await page.getByRole("button", { name: "PRELOAD", exact: true }).click();
      await desk.command("GROUP 1 AT 80", "G1 AT 80");
      await openPlaybackMode(page);
      await playbackCard(page, 1).getByRole("button", { name: "GO +", exact: true }).click();
      await pane.getByRole("button", { name: /Virtual playback page 1 cell 2 Virtual combined/ }).click();
      state.pending = preloadCombinedObservation(await programmer(api));
      await page.getByRole("button", { name: /^PRELOAD GO\b/ }).click();
      state.applicationTimestamp = (await programmer(api)).preload_group_active["1"].intensity.changed_at;
      await bench.tick(1_500);
      await longPressPreload(page);
    },
    assert: async ({ api }, state) => {
      expect(state.pending).toEqual({
        groupIds: ["1"],
        playbackActions: [[61, "go", "physical"], [62, "toggle", "virtual"]],
      });
      expect(state.applicationTimestamp).toEqual(expect.any(String));
      expect(await activePlayback(api, 61)).toMatchObject({ enabled: true, current_cue_number: 1, activated_at: state.applicationTimestamp });
      expect(await activePlayback(api, 62)).toMatchObject({ enabled: true, current_cue_number: 1, activated_at: state.applicationTimestamp });
      expect((await programmer(api)).preload_group_active).toEqual({});
      expect(await visualizationLevel(api, state.groupFixture)).toBeCloseTo(0.25, 2);
    },
  });

  test("PRELOAD-006 @supplemental › API timestamp boundaries, source ownership, and event idempotency", async ({ api, bench }) => {
    const groupFixture = await (async () => {
      await loadCanonicalCopy(api, bench, "preload-006-combined-release");
      return firstGroupFixture(api, "1");
    })();
    const fixtures = await fixtureIdsByNumber(api);
    const groupFixtureNumber = Number(Object.entries(fixtures).find(([, id]) => id === groupFixture)?.[0]);
    const specs: PlaybackSpec[] = [
      { number: 60, fixture: groupFixtureNumber, levels: [0.25], name: "Underlying source" },
      { number: 61, fixture: 3, levels: [0.6], name: "Physical result", buttons: ["go", "none", "none"], buttonCount: 1, hasFader: false },
      { number: 62, fixture: 4, levels: [0.8], name: "Virtual result", buttons: ["toggle", "none", "none"], buttonCount: 1, hasFader: false },
    ];
    await installOnCurrentShow(api, fixtures, specs, { 1: 61, 2: 62 });
    await setCaptureMask(api, true, true, true, 1_500, 8_000);
    await poolAction(api, 60, "go");
    // This is an ordinary live GO, so its zero-time cue correctly uses the 8 s Cue Fade.
    await bench.tick(8_000);
    expect(await visualizationLevel(api, groupFixture)).toBeCloseTo(0.25, 2);
    await api.command("preload.enter", {});
    await api.command("programmer.execute", { value: "GROUP 1 AT 80" });
    await poolAction(api, 61, "button", { button: 1, pressed: true, surface: "physical" });
    await poolAction(api, 62, "button", { button: 1, pressed: true, surface: "virtual" });
    const pending = await programmer(api);
    expect(pending.preload_group_pending["1"]).toBeDefined();
    expect(pending.preload_playback_pending.map((entry: any) => entry.surface)).toEqual(["physical", "virtual"]);
    expect(await activePlayback(api, 61)).toBeUndefined();
    expect(await activePlayback(api, 62)).toBeUndefined();

    await bench.tick(200);
    const committed = (await api.command<any>("preload.go", {})).payload!;
    const committedProgrammer = await programmer(api);
    expect(committedProgrammer.preload_group_active["1"].intensity.changed_at).toBe(committed.application_timestamp);
    expect((await activePlayback(api, 61))?.activated_at).toBe(committed.application_timestamp);
    expect((await activePlayback(api, 62))?.activated_at).toBe(committed.application_timestamp);
    await bench.tick(0);
    expect(await visualizationLevel(api, groupFixture)).toBeCloseTo(0.25, 2);
    expect(await visualizationLevel(api, fixtures[3])).toBeCloseTo(0, 5);
    expect(await visualizationLevel(api, fixtures[4])).toBeCloseTo(0, 5);
    await bench.tick(1_500);
    expect(await visualizationLevel(api, groupFixture)).toBeCloseTo(0.8, 2);
    expect(await activePlayback(api, 61)).toMatchObject({ enabled: true });
    expect(await activePlayback(api, 62)).toMatchObject({ enabled: true });

    expect((await api.command<any>("preload.release", {})).payload).toMatchObject({ released: true });
    expect(await visualizationLevel(api, groupFixture)).toBeCloseTo(0.25, 2);
    expect(await activePlayback(api, 61)).toMatchObject({ enabled: true });
    expect(await activePlayback(api, 62)).toMatchObject({ enabled: true });
    const eventsBefore = await audit(api);
    const frameBefore = await bench.tick(0);
    expect((await api.command<any>("preload.release", {})).payload).toMatchObject({ released: false });
    expect(await audit(api, Math.max(0, ...eventsBefore.map((event) => event.revision)))).toEqual([]);
    expect(await bench.tick(0)).toEqual(frameBefore);
  });

  test("PRELOAD-006 @supplemental-ui › combined controls expose pending state and asymmetric long-press release", async ({ api, bench, desk, page }) => {
    await prepare(api, bench, "preload-006-ui", [
      { number: 63, fixture: 3, levels: [0.6], name: "Physical combined", buttons: ["go", "none", "none"], buttonCount: 1, hasFader: false },
      { number: 64, fixture: 4, levels: [0.8], name: "Virtual combined", buttons: ["toggle", "none", "none"], buttonCount: 1, hasFader: false },
    ], { 1: 63, 2: 64 });
    await setCaptureMask(api, true, true, true, 1_500, 8_000);
    await desk.open(bench.baseUrl);
    const pane = await addVirtualPlaybackPane(page);
    await page.getByRole("button", { name: "PRELOAD", exact: true }).click();
    await desk.command("GROUP 1 AT 80", "G1 AT 80");
    await openPlaybackMode(page);
    await playbackCard(page, 1).getByRole("button", { name: "GO +", exact: true }).click();
    await pane.getByRole("button", { name: /Virtual playback page 1 cell 2 Virtual combined/ }).click();
    await expect(page.getByLabel(/Pending Preload: PROG 1.*GO 63.*TOGGLE 64/)).toBeVisible();
    await desk.recordStep("ATOMIC PRELOAD GO", "Publish the temporary programmer and both real playbacks at one application timestamp.");
    await page.getByRole("button", { name: /^PRELOAD GO\b/ }).click();
    await bench.tick(1_500);
    const state = await playbacks(api);
    const physical = state.active.find((entry: any) => entry.playback_number === 63);
    const virtual = state.active.find((entry: any) => entry.playback_number === 64);
    expect(physical.activated_at).toBe(virtual.activated_at);
    await desk.recordStep("LONG-PRESS RELEASE", "Remove only the temporary programmer; the physical and virtual playback results remain.");
    await longPressPreload(page);
    expect((await programmer(api)).preload_group_active).toEqual({});
    expect(await activePlayback(api, 63)).toMatchObject({ enabled: true });
    expect(await activePlayback(api, 64)).toMatchObject({ enabled: true });
  });

  pairedScenario<VirtualZonePairState>({
    id: "VPB-007",
    title: "named Virtual Playback exclusion zones are inert on creation and authoritative on activation",
    arrange: async ({ api, bench }, surface) => {
      const prepared = await prepare(api, bench, `vpb-007-paired-${surface}`, [
        { number: 74, fixture: 3, levels: [0.25], name: "Touring A", buttons: ["toggle", "none", "none"], buttonCount: 1, hasFader: false },
        { number: 75, fixture: 4, levels: [0.5], name: "Touring B", buttons: ["toggle", "none", "none"], buttonCount: 1, hasFader: false },
        { number: 76, fixture: 5, levels: [0.75], name: "Touring C", buttons: ["toggle", "none", "none"], buttonCount: 1, hasFader: false },
      ], { 1: 74, 2: 75, 3: 76 });
      await api.request("PUT", "/api/v1/configuration", { ...await configuration(api), sequence_master_fade_millis: 0 });
      await poolAction(api, 74, "on", { surface: "virtual" });
      await poolAction(api, 75, "on", { surface: "virtual" });
      return prepared;
    },
    api: async ({ api }, state) => {
      await api.request("PUT", "/api/v1/virtual-playback-exclusion-zones/vpb-paired-surface", {
        zones: [{ id: "touring-pair", name: "Touring pair", slots: [1, 2] }],
      });
      state.savedZones = await normalizedVirtualZones(api);
      state.creationState = [Boolean((await activePlayback(api, 74))?.enabled), Boolean((await activePlayback(api, 75))?.enabled)];
      for (const number of [74, 75, 74, 75])
        await poolAction(api, number, "toggle", { surface: "virtual" });
    },
    ui: async ({ api, bench, desk, page }, state) => {
      await desk.open(bench.baseUrl);
      const pane = await addVirtualPlaybackPane(page);
      await page.keyboard.down("Shift");
      await pane.getByRole("button", { name: /Virtual playback page 1 cell 1 Touring A/ }).click();
      await pane.getByRole("button", { name: /Virtual playback page 1 cell 2 Touring B/ }).click();
      await page.keyboard.up("Shift");
      await pane.getByRole("button", { name: "Create Exclusion Zone" }).click();
      const create = page.getByRole("dialog", { name: "Create Exclusion Zone" });
      await create.getByLabel("Zone name").fill("Touring pair");
      await create.getByRole("button", { name: "Create zone" }).click();
      await expect(create).toBeHidden();
      state.savedZones = await normalizedVirtualZones(api);
      state.creationState = [Boolean((await activePlayback(api, 74))?.enabled), Boolean((await activePlayback(api, 75))?.enabled)];
      for (const cell of [1, 2, 1, 2])
        await pane.getByRole("button", { name: new RegExp(`Virtual playback page 1 cell ${cell}`) }).click();
    },
    assert: async ({ api, bench }, state) => {
      expect(state.savedZones).toEqual([{ name: "Touring pair", slots: [1, 2] }]);
      expect(state.creationState).toEqual([true, true]);
      expect(await activePlayback(api, 74)).toMatchObject({ enabled: false });
      expect(await activePlayback(api, 75)).toMatchObject({ enabled: true });
      expect((await activePlayback(api, 76))?.enabled ?? false).toBe(false);
      await bench.tick(0);
      expect(await visualizationLevel(api, state.fixtures[3])).toBeCloseTo(0, 5);
      expect(await visualizationLevel(api, state.fixtures[4])).toBeCloseTo(0.5, 5);
      expect(await visualizationLevel(api, state.fixtures[5])).toBeCloseTo(0, 5);
    },
  });

  test("VPB-007 @supplemental @osc @restart › overlapping zones are serialized, desk-scoped, and durable on every transport", async ({ api, bench }) => {
    const prepared = await prepare(api, bench, "vpb-007-authoritative", [
      { number: 71, fixture: 3, levels: [0.2], name: "Zone A" },
      { number: 72, fixture: 4, levels: [0.4], name: "Zone B" },
      { number: 73, fixture: 5, levels: [0.6], name: "Zone C" },
    ], { 1: 71, 2: 72, 3: 73 });
    await writePage(api, 2, { "1": 73, "2": 71, "3": 72 });
    await api.request("PUT", "/api/v1/configuration", { ...await configuration(api), sequence_master_fade_millis: 0 });
    const firstDefinition = await object<any>(api, "playback", "71");
    await putObject(api, "playback", "71", { ...firstDefinition.body, auto_off: false }, firstDefinition.revision);
    const firstDesk = api.session!.desk;
    const zones = [
      { id: "front-pair", name: "Front pair", slots: [1, 2] },
      { id: "overlap", name: "Overlap pair", slots: [2, 3] },
    ];

    await poolAction(api, 71, "go", { surface: "virtual" });
    await poolAction(api, 72, "go", { surface: "virtual" });
    await poolAction(api, 73, "go", { surface: "virtual" });
    expect(await activePlayback(api, 71)).toMatchObject({ enabled: true });
    expect(await activePlayback(api, 72)).toMatchObject({ enabled: true });
    expect(await activePlayback(api, 73)).toMatchObject({ enabled: true });
    await api.request("PUT", "/api/v1/virtual-playback-exclusion-zones/vpb-api-surface", { zones });
    expect(await activePlayback(api, 71)).toMatchObject({ enabled: true });
    expect(await activePlayback(api, 72)).toMatchObject({ enabled: true });
    expect(await activePlayback(api, 73)).toMatchObject({ enabled: true });
    expect(await api.request<any>("GET", "/api/v1/virtual-playback-exclusion-zones")).toMatchObject({
      desk_id: firstDesk.id,
      surfaces: { "vpb-api-surface": zones },
    });

    await bench.stopServerGracefully(api.session!.token);
    await bench.startServer();
    api.session = await api.request<Session>("POST", "/api/v1/sessions", { username: "Operator", desk_id: firstDesk.id }, false);
    expect((await object<any>(api, "playback", "71")).body.auto_off).toBe(false);
    expect(await activePlayback(api, 71)).toMatchObject({ enabled: false });
    expect(await activePlayback(api, 72)).toMatchObject({ enabled: false });
    expect(await activePlayback(api, 73)).toMatchObject({ enabled: true });
    expect((await api.request<any>("GET", "/api/v1/virtual-playback-exclusion-zones")).surfaces["vpb-api-surface"]).toEqual(zones);

    for (const number of [71, 72, 73]) await poolAction(api, number, "off");
    await Promise.all([poolAction(api, 71, "go", { surface: "virtual" }), poolAction(api, 72, "go", { surface: "virtual" })]);
    const concurrent = (await playbacks(api)).active.filter((entry: any) => [71, 72].includes(entry.playback_number) && entry.enabled);
    expect(concurrent).toHaveLength(1);
    await poolAction(api, 73, "go", { surface: "virtual" });
    await poolAction(api, 72, "go", { surface: "virtual" });
    expect(await activePlayback(api, 71)).toMatchObject({ enabled: false });
    expect(await activePlayback(api, 72)).toMatchObject({ enabled: true });
    expect(await activePlayback(api, 73)).toMatchObject({ enabled: false });
    await bench.tick(0);
    expect(await visualizationLevel(api, prepared.fixtures[3])).toBeCloseTo(0, 5);
    expect(await visualizationLevel(api, prepared.fixtures[4])).toBeCloseTo(0.4, 5);
    expect(await visualizationLevel(api, prepared.fixtures[5])).toBeCloseTo(0, 5);
    await poolAction(api, 72, "off", { surface: "virtual" });
    expect(await activePlayback(api, 71)).toMatchObject({ enabled: false });
    expect(await activePlayback(api, 73)).toMatchObject({ enabled: false });

    await api.request("PUT", `/api/v1/control-desks/${api.session.desk.id}/page`, { page: 2 });
    for (const number of [71, 72, 73]) await poolAction(api, number, "off");
    await api.request("POST", `/api/v1/control-desks/${api.session.desk.id}/page-playbacks/1/button`, { button: 1, pressed: true, surface: "virtual" });
    await api.request("POST", `/api/v1/control-desks/${api.session.desk.id}/page-playbacks/2/button`, { button: 1, pressed: true, surface: "virtual" });
    expect(await activePlayback(api, 73)).toMatchObject({ enabled: false });
    expect(await activePlayback(api, 71)).toMatchObject({ enabled: true });
    await api.request("PUT", `/api/v1/control-desks/${api.session.desk.id}/page`, { page: 1 });
    for (const number of [71, 72, 73]) await poolAction(api, number, "off");

    const firstHardware = await bench.osc();
    try {
      await firstHardware.subscribe("vpb-007-first", api.session.desk.osc_alias);
      await firstHardware.send(`/light/${api.session.desk.osc_alias}/page-playback/1/button/1`, [true]);
      await firstHardware.send(`/light/${api.session.desk.osc_alias}/page-playback/2/button/1`, [true]);
      await expect.poll(async () => (await activePlayback(api, 71))?.enabled).toBe(false);
      await expect.poll(async () => (await activePlayback(api, 72))?.enabled).toBe(true);
      expect((await audit(api)).some((event) => event.kind === "playback_exclusion_applied" && event.payload?.source === "osc" && event.payload?.activated_playback === 72)).toBe(true);
    } finally {
      await firstHardware.close();
    }

    const second = await api.request<Session>("POST", "/api/v1/sessions", { username: "Operator", client_id: crypto.randomUUID() }, false);
    api.session = second;
    expect((await api.request<any>("GET", "/api/v1/virtual-playback-exclusion-zones")).surfaces).toEqual({});
    for (const number of [71, 72, 73]) await poolAction(api, number, "off");
    const secondHardware = await bench.osc();
    try {
      await secondHardware.subscribe("vpb-007-second", second.desk.osc_alias);
      await secondHardware.send(`/light/${second.desk.osc_alias}/page-playback/1/button/1`, [true]);
      await secondHardware.send(`/light/${second.desk.osc_alias}/page-playback/2/button/1`, [true]);
      await expect.poll(async () => (await activePlayback(api, 71))?.enabled).toBe(true);
      await expect.poll(async () => (await activePlayback(api, 72))?.enabled).toBe(true);
      await bench.tick(0);
      expect(await visualizationLevel(api, prepared.fixtures[3])).toBeCloseTo(0.2, 5);
      expect(await visualizationLevel(api, prepared.fixtures[4])).toBeCloseTo(0.4, 5);
    } finally {
      await secondHardware.close();
    }
  });

  test("VPB-007 @supplemental-ui › Settings edits hidden membership and reload restores it", async ({ api, bench, desk, page }) => {
    await prepare(api, bench, "vpb-007-ui", [
      { number: 74, fixture: 3, levels: [0.25], name: "Touring A" },
      { number: 75, fixture: 4, levels: [0.5], name: "Touring B" },
      { number: 76, fixture: 5, levels: [0.75], name: "Touring C" },
    ], { 1: 74, 2: 75, 3: 76 });
    await desk.open(bench.baseUrl);
    let pane = await addVirtualPlaybackPane(page);
    await desk.recordStep("SELECT EXCLUSION MEMBERS", "Hold Shift and choose cells 1 and 2. Selection must not operate either playback.");
    await page.keyboard.down("Shift");
    await pane.getByRole("button", { name: /Virtual playback page 1 cell 1 Touring A/ }).click();
    await pane.getByRole("button", { name: /Virtual playback page 1 cell 2 Touring B/ }).click();
    await page.keyboard.up("Shift");
    expect(await activePlayback(api, 74)).toBeUndefined();
    expect(await activePlayback(api, 75)).toBeUndefined();
    await pane.getByRole("button", { name: "Create Exclusion Zone" }).click();
    const create = page.getByRole("dialog", { name: "Create Exclusion Zone" });
    await create.getByLabel("Zone name").fill("Touring pair");
    await create.getByRole("button", { name: "Create zone" }).click();
    await expect(create).toBeHidden();

    await desk.recordStep("NEW ACTIVATION WINS", "Turn on cell 1, then cell 2. Cell 2 remains On and cell 1 is released by the server.");
    await pane.getByRole("button", { name: /Virtual playback page 1 cell 1 Touring A/ }).click();
    await pane.getByRole("button", { name: /Virtual playback page 1 cell 2 Touring B/ }).click();
    await expect.poll(async () => (await activePlayback(api, 74))?.enabled).toBe(false);
    await expect.poll(async () => (await activePlayback(api, 75))?.enabled).toBe(true);

    await poolAction(api, 74, "off");
    await poolAction(api, 75, "off");
    await desk.recordStep("KEYBOARD USES THE SAME ZONE", "F1 followed by F2 operates the current-page cells through the shared server path; F2 wins.");
    await page.keyboard.press("F1");
    await page.keyboard.press("F2");
    await expect.poll(async () => (await activePlayback(api, 74))?.enabled).toBe(false);
    await expect.poll(async () => (await activePlayback(api, 75))?.enabled).toBe(true);

    await pane.getByRole("button", { name: "Settings", exact: true }).click();
    let settings = page.getByRole("dialog", { name: "Pane Settings" });
    await settings.getByRole("tab", { name: "Virtual Playbacks", exact: true }).click();
    await settings.getByLabel("Name for Touring pair").fill("Touring alternates");
    await settings.getByRole("button", { name: "Save name" }).click();
    await settings.getByRole("button", { name: "Touring alternates cell 3" }).click();
    await settings.getByLabel("Rows").fill("1");
    await settings.getByLabel("Columns").fill("2");
    await expect(settings.getByText("1 hidden grid cell is retained:")).toBeVisible();
    await expect(settings.getByRole("button", { name: "Touring alternates hidden cell 3" })).toBeVisible();
    await settings.getByRole("button", { name: "Close settings" }).click();

    await page.waitForTimeout(1_000);
    await page.reload();
    await expect(page.locator(".connection-cover")).toBeHidden({ timeout: 10_000 });
    pane = await activeVirtualPane(page);
    await expect(pane.locator(".virtual-playback-cell")).toHaveCount(2);
    await pane.getByRole("button", { name: "Settings", exact: true }).click();
    settings = page.getByRole("dialog", { name: "Pane Settings" });
    await settings.getByRole("tab", { name: "Virtual Playbacks", exact: true }).click();
    await expect(settings.getByLabel("Name for Touring alternates")).toHaveValue("Touring alternates");
    await expect(settings.getByRole("button", { name: "Touring alternates hidden cell 3" })).toBeVisible();
  });
});

async function preloadProgrammerObservation(api: ApiDriver, fixtures: [string, string]): Promise<NonNullable<PreloadProgrammerPairState["pending"]>> {
  const pending = await programmer(api);
  return {
    blind: pending.blind,
    groupIds: Object.keys(pending.preload_group_pending).sort(),
    groupValues: Object.keys(pending.group_values).sort(),
    firstFadeMillis: pending.preload_group_pending["1"].intensity.fade_millis ?? null,
    secondFadeMillis: pending.preload_group_pending["2"].intensity.fade_millis ?? null,
    playbackActions: pending.preload_playback_pending.map((entry: any) => entry.action),
    liveLevels: [
      await visualizationLevel(api, fixtures[0]),
      await visualizationLevel(api, fixtures[1]),
    ],
  };
}

function playbackPendingObservation(state: any): Array<[number, string, string]> {
  return state.preload_playback_pending.map((entry: any) => [entry.playback_number, entry.action, entry.surface]);
}

function captureMask(config: Configuration): [boolean, boolean, boolean] {
  return [
    config.preload_programmer_changes,
    config.preload_physical_playback_actions,
    config.preload_virtual_playback_actions,
  ];
}

const preloadMaskLabels = [
  "Preload programmer changes",
  "Preload physical playback actions",
  "Preload virtual playback actions",
] as const;

async function openPreloadInputSettings(page: Page) {
  await page.getByRole("button", { name: /Open show menu/ }).click();
  await page.getByRole("button", { name: "Enter Setup", exact: true }).click();
  await page.getByRole("button", { name: "Inputs", exact: true }).click();
}

async function setPreloadMaskThroughUi(api: ApiDriver, page: Page, mask: number) {
  for (let index = 0; index < preloadMaskLabels.length; index++) {
    const desired = Boolean(mask & (1 << index));
    const control = page.getByRole("switch", { name: preloadMaskLabels[index] });
    if ((await control.isChecked()) !== desired)
      await control.locator("..").locator(".ui-switch-track").click();
  }
  await page.getByRole("button", { name: "Save changes", exact: true }).click();
  await expect.poll(async () => captureMask(await configuration(api))).toEqual([
    Boolean(mask & 1),
    Boolean(mask & 2),
    Boolean(mask & 4),
  ]);
}

async function expectPreloadMaskControls(page: Page, mask: number) {
  for (let index = 0; index < preloadMaskLabels.length; index++)
    await expect(page.getByLabel(preloadMaskLabels[index])).toBeChecked({ checked: Boolean(mask & (1 << index)) });
}

function preloadCombinedObservation(state: any): NonNullable<PreloadCombinedPairState["pending"]> {
  return {
    groupIds: Object.keys(state.preload_group_pending).sort(),
    playbackActions: playbackPendingObservation(state),
  };
}

async function normalizedVirtualZones(api: ApiDriver): Promise<Array<{ name: string; slots: number[] }>> {
  const response = await api.request<any>("GET", "/api/v1/virtual-playback-exclusion-zones");
  return Object.values(response.surfaces as Record<string, Array<{ name: string; slots: number[] }>>)
    .flat()
    .map((zone) => ({ name: zone.name, slots: [...zone.slots] }))
    .sort((left, right) => left.name.localeCompare(right.name));
}

async function prepare(api: ApiDriver, bench: any, name: string, specs: PlaybackSpec[], slots: Record<number, number>): Promise<Prepared> {
  await loadCanonicalCopy(api, bench, name);
  const fixtures = await fixtureIdsByNumber(api);
  return installOnCurrentShow(api, fixtures, specs, slots);
}

async function installOnCurrentShow(api: ApiDriver, fixtures: Record<number, string>, specs: PlaybackSpec[], slots: Record<number, number>): Promise<Prepared> {
  const cueLists: Record<number, string> = {};
  for (const spec of specs) {
    const cueListId = crypto.randomUUID();
    cueLists[spec.number] = cueListId;
    await putObject(api, "cue_list", cueListId, {
      id: cueListId,
      name: spec.name ?? `Preload ${spec.number}`,
      priority: 0,
      mode: "sequence",
      looped: false,
      chaser_step_millis: 1_000,
      speed_group: null,
      cues: (spec.levels ?? [1]).map((level, index) => ({
        id: crypto.randomUUID(), number: index + 1, name: `Cue ${index + 1}`,
        changes: [{ fixture_id: fixtures[spec.fixture], attribute: "intensity", value: { kind: "normalized", value: level }, automatic_restore: false }],
        group_changes: [], fade_millis: 0, delay_millis: 0, trigger: { type: "manual" }, phasers: [],
      })),
    });
    await putObject(api, "playback", String(spec.number), playbackDefinition(spec, cueListId));
  }
  await writePage(api, 1, Object.fromEntries(Object.entries(slots).map(([slot, number]) => [String(slot), number])));
  return { fixtures, cueLists };
}

function playbackDefinition(spec: PlaybackSpec, cueListId: string) {
  return {
    number: spec.number,
    name: spec.name ?? `Preload ${spec.number}`,
    target: { type: "cue_list", cue_list_id: cueListId },
    buttons: spec.buttons ?? ["go", "go_minus", "flash"],
    button_count: spec.buttonCount ?? 3,
    fader: "master",
    has_fader: spec.hasFader ?? true,
    go_activates: true,
    auto_off: true,
    xfade_millis: 0,
    color: "#20c997",
    flash_release: "release_all",
    protect_from_swap: false,
  };
}

async function writePage(api: ApiDriver, number: number, slots: Record<string, number>) {
  const current = (await objects<any>(api, "playback_page")).find((entry) => entry.id === String(number));
  await putObject(api, "playback_page", String(number), { number, name: number === 1 ? "Main" : `Page ${number}`, slots }, current?.revision ?? 0);
}

async function pageObject(api: ApiDriver, page: number) {
  return object<any>(api, "playback_page", String(page));
}

async function configuration(api: ApiDriver): Promise<Configuration> {
  return (await api.request<any>("GET", "/api/v1/configuration")).configuration;
}

async function setCaptureMask(api: ApiDriver, programmerCapture: boolean, physicalCapture: boolean, virtualCapture: boolean, programmerFade = 3_000, cueFade = 3_000) {
  const current = await configuration(api);
  await api.request("PUT", "/api/v1/configuration", {
    ...current,
    programmer_fade_millis: programmerFade,
    sequence_master_fade_millis: cueFade,
    preload_programmer_changes: programmerCapture,
    preload_physical_playback_actions: physicalCapture,
    preload_virtual_playback_actions: virtualCapture,
  });
}

async function poolAction<T = any>(api: ApiDriver, number: number, action: string, body: Record<string, unknown> = {}): Promise<T> {
  return api.request<T>(action === "master" ? "PUT" : "POST", `/api/v1/playback-pool/${number}/${action}`, body);
}

async function playbacks(api: ApiDriver): Promise<any> {
  return api.request("GET", "/api/v1/playbacks");
}

async function activePlayback(api: ApiDriver, number: number): Promise<any | undefined> {
  return (await playbacks(api)).active.find((entry: any) => entry.playback_number === number);
}

async function firstGroupFixture(api: ApiDriver, id: string): Promise<string> {
  const group = await object<any>(api, "group", id);
  expect(group.body.fixtures.length).toBeGreaterThan(0);
  return group.body.fixtures[0];
}

async function distinctGroupFixtures(api: ApiDriver, broadId: string, subsetId: string): Promise<[string, string]> {
  const broad = (await object<any>(api, "group", broadId)).body.fixtures as string[];
  const subset = (await object<any>(api, "group", subsetId)).body.fixtures as string[];
  const broadOnly = broad.find((fixture) => !subset.includes(fixture));
  expect(broadOnly).toBeDefined();
  expect(subset[0]).toBeDefined();
  return [broadOnly!, subset[0]];
}

async function visualizationLevel(api: ApiDriver, fixtureId: string, attribute = "intensity"): Promise<number> {
  const snapshot = await api.request<any>("GET", "/api/v1/visualization");
  const value = snapshot.values.find((entry: any) => entry.fixture_id === fixtureId && entry.attribute === attribute)?.value;
  return typeof value === "number" ? value : value?.value ?? 0;
}

async function audit(api: ApiDriver, after = 0): Promise<any[]> {
  return api.request("GET", `/api/v1/audit?after=${after}`);
}

function summarizePlaybackState(snapshot: any, numbers: number[]) {
  return snapshot.active
    .filter((entry: any) => numbers.includes(entry.playback_number))
    .map((entry: any) => ({ number: entry.playback_number, cue: entry.current_cue_number, enabled: entry.enabled, temporary: entry.temporary_active }))
    .sort((left: any, right: any) => left.number - right.number);
}

async function openPlaybackMode(page: Page) {
  if (await page.locator(".playback-fader-bank").isVisible()) return;
  await page.locator(".mode-toggle").click();
  await expect(page.locator(".playback-fader-bank")).toBeVisible();
}

function playbackCard(page: Page, slotNumber: number): Locator {
  return page.locator(`.playback-fader-bank article[data-playback-slot="${slotNumber}"]`);
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
  return activeVirtualPane(page);
}

async function activeVirtualPane(page: Page): Promise<Locator> {
  await page.getByRole("button", { name: "DESKTOPS", exact: true }).click();
  const activeDesk = page.locator(".dock-list .dock-entry.active");
  if (!(await activeDesk.isVisible().catch(() => false)))
    await page.locator(".dock-list .dock-entry").filter({ hasText: /Desk \d+/ }).last().click();
  const pane = page.locator(".desk-pane").filter({ hasText: "Virtual Playbacks" });
  await expect(pane).toBeVisible();
  return pane;
}

async function assignVirtualSource(page: Page, pane: Locator, sourceName: string, cell: number) {
  await pane.getByRole("button", { name: "Set Source", exact: true }).click();
  await page.getByRole("button", { name: "BUILT-INS", exact: true }).click();
  await page.locator(".dock-entry").filter({ hasText: "Cuelists" }).click();
  await page.locator(".cuelist-card").filter({ hasText: sourceName }).click();
  const restored = await activeVirtualPane(page);
  await restored.getByRole("button", { name: new RegExp(`Virtual playback page 1 cell ${cell} empty`) }).click();
  await expect.poll(async () => (await pageObjectFromUi(page, cell))).not.toBeUndefined();
}

async function pageObjectFromUi(page: Page, cell: number) {
  return page.evaluate(async (slot) => {
    const session = JSON.parse(localStorage.getItem("light.primary-session") ?? "null");
    if (!session?.token) return undefined;
    const response = await fetch("/api/v1/playbacks", { headers: { Authorization: `Bearer ${session.token}` } });
    if (!response.ok) return undefined;
    const state = await response.json();
    return state.pages.find((candidate: any) => candidate.number === state.active_page)?.slots?.[String(slot)];
  }, cell);
}

function selectTrigger(container: Locator, label: string): Locator {
  const escaped = label.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return container.locator(".ui-form-field").filter({ hasText: new RegExp(`^\\s*${escaped}`) }).locator(".ui-select-trigger");
}

async function chooseSelect(page: Page, container: Locator, label: string, option: string) {
  await selectTrigger(container, label).click();
  await page.getByRole("option", { name: option, exact: true }).click();
}

async function longPressPreload(page: Page) {
  const button = page.getByRole("button", { name: /^PRELOAD/ });
  await button.hover();
  await page.mouse.down();
  await page.waitForTimeout(750);
  await page.mouse.up();
  await expect(page.getByRole("button", { name: "PRELOAD", exact: true })).toBeVisible();
}
