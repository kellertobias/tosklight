import { expect, test, type BenchUiContext } from "../apps/control-ui/e2e/bench/fixtures";
import { ApiDriver, closeWebSocket } from "../apps/control-ui/e2e/bench/api";
import { pairedScenario } from "../apps/control-ui/e2e/bench/pairedScenario";
import { fixtureIdsByNumber, loadCanonicalCopy, object, objects, pressCommand, putObject } from "./support/catalog";

const cue001Ui = async ({ api, bench, desk, page }: BenchUiContext, state: { completed: boolean }) => {
  await emptyPlaybackPage(api);
  const beforeCuelists = new Set((await objects(api, "cue_list")).map((item) => item.id));

  await api.command("programmer.execute", { value: "GROUP 1 AT 100" });
  await desk.open(bench.baseUrl);
  await page.locator(".mode-toggle").click();
  await page.getByRole("button", { name: "REC", exact: true }).click();
  await page.getByRole("button", { name: "Playback representation page 1 playback 1" }).click();

  const stored = await expectNewRecordedCuelist(api, beforeCuelists, 1);
  const playbackNumber = await playbackAtSlot(api, 1);
  await expect
    .poll(async () => runtime(api, playbackNumber))
    .toMatchObject({
      current_cue_number: 1,
      enabled: true,
      flash: false,
    });
  expect(stored.body.cues[0].group_changes).toMatchObject([{ group_id: "1", attribute: "intensity", value: { kind: "normalized", value: 1 } }]);
  expect(logicalSlots(await bench.tick(3_000), 8)).toEqual([...Array(4).fill(255), ...Array(4).fill(0)]);

  for (const [button, group, level] of [
    ["GO +", "2", 1],
    ["GO −", "3", 1],
    ["FLASH", "1", 0.5],
  ] as const) {
    await api.command("programmer.clear", {});
    await api.command("programmer.execute", { value: `GROUP ${group} AT ${level * 100}` });
    await page.getByRole("button", { name: "REC", exact: true }).click();
    const card = page.locator(".playback-fader-bank article").filter({ hasText: stored.body.name });
    await card.getByRole("button", { name: button, exact: true }).click();
    const expectedCueCount = button === "GO +" ? 2 : button === "GO −" ? 3 : 4;
    await expect.poll(async () => (await object<any>(api, "cue_list", stored.id)).body.cues.length).toBe(expectedCueCount);
    await expect
      .poll(async () => runtime(api, playbackNumber))
      .toMatchObject({
        current_cue_number: expectedCueCount,
        enabled: true,
        flash: false,
      });
  }

  const definition = await object<any>(api, "playback", String(playbackNumber));
  await putObject(api, "playback", String(playbackNumber), { ...definition.body, buttons: ["toggle", "on", "off"] }, definition.revision);
  for (const [index, [button, group, level]] of [
    ["TOGGLE", "2", 0.2],
    ["ON", "3", 0.3],
    ["OFF", "1", 0.4],
  ].entries()) {
    await api.command("programmer.clear", {});
    await api.command("programmer.execute", { value: `GROUP ${group} AT ${level * 100}` });
    await page.getByRole("button", { name: "REC", exact: true }).click();
    const card = page.locator(".playback-fader-bank article").filter({ hasText: stored.body.name });
    await card.getByRole("button", { name: button, exact: true }).click();
    const expectedCueCount = index + 5;
    await expect.poll(async () => (await object<any>(api, "cue_list", stored.id)).body.cues.length).toBe(expectedCueCount);
    await expect
      .poll(async () => runtime(api, playbackNumber))
      .toMatchObject({
        current_cue_number: expectedCueCount,
        enabled: true,
        flash: false,
      });
  }

  const final = await object<any>(api, "cue_list", stored.id);
  expect(final.body.cues.map((cue: any) => cue.number)).toEqual([1, 2, 3, 4, 5, 6, 7]);
  expect(final.body.cues.map((cue: any) => cue.group_changes.map((change: any) => [change.group_id, rounded(change.value.value)]))).toEqual([
    [["1", 1]],
    [["2", 1]],
    [["3", 1]],
    [["1", 0.5]],
    [["2", 0.2]],
    [["3", 0.3]],
    [["1", 0.4]],
  ]);
  state.completed = true;
};

test.describe("docs/testing/02-cues-tracking-and-arbitration.md", () => {
  pairedScenario<{ completed: boolean }>({
    id: "CUE-008",
    title: "blind Preload records the same Cue without activating playback or output",
    arrange: async ({ api, bench }, surface) => {
      await loadCanonicalCopy(api, bench, `cue-008-preload-record-${surface}`, "compact-rig");
      await installCompactGroups(api);
      return { completed: false };
    },
    api: async ({ api, bench }, state) => {
      await api.command("preload.enter", {});
      await api.command("programmer.execute", { value: "GROUP 1 AT 100" });
      const pending = await currentProgrammer(api);
      expect(pending.preload_group_pending["1"].intensity.value).toMatchObject({ kind: "normalized", value: 1 });
      const installed = await installPlaybackSequence(api, 1, [groupCue(1, [["1", "intensity", 1]])]);
      const stored = await object<any>(api, "cue_list", installed.id);
      expect(stored.body.cues[0].group_changes).toMatchObject([{ group_id: "1", attribute: "intensity", value: { kind: "normalized", value: 1 } }]);
      expect((await playbackState(api)).active.some((item: any) => item.playback_number === 1 && item.enabled)).toBe(false);
      expect(logicalSlots(await bench.tick(0), 4)).toEqual(Array(4).fill(0));
      await api.command("preload.release", {});
      await api.request("POST", "/api/v1/cuelists/1/go", {});
      expect(logicalSlots(await bench.tick(3_000), 4)).toEqual(Array(4).fill(255));
      state.completed = true;
    },
    ui: async ({ api, bench, desk, page }, state) => {
      await emptyPlaybackPage(api);
      const beforeCuelists = new Set((await objects(api, "cue_list")).map((item) => item.id));
      await api.command("preload.enter", {});
      await api.command("programmer.execute", { value: "GROUP 1 AT 100" });

      await desk.open(bench.baseUrl);
      await page.locator(".mode-toggle").click();
      await page.getByRole("button", { name: "REC", exact: true }).click();
      await page.getByRole("button", { name: "Playback representation page 1 playback 1" }).click();

      const stored = await expectNewRecordedCuelist(api, beforeCuelists, 1);
      const playbackNumber = await playbackAtSlot(api, 1);
      expect(stored.body.cues[0].group_changes).toMatchObject([{ group_id: "1", attribute: "intensity", value: { kind: "normalized", value: 1 } }]);
      expect((await playbackState(api)).active.some((item: any) => item.playback_number === playbackNumber && item.enabled)).toBe(false);
      expect(logicalSlots(await bench.tick(0), 4)).toEqual(Array(4).fill(0));

      await api.command("preload.release", {});
      await api.request("POST", `/api/v1/cuelists/${playbackNumber}/go`, {});
      await expect.poll(async () => runtime(api, playbackNumber)).toMatchObject({ current_cue_number: 1, enabled: true });
      expect(logicalSlots(await bench.tick(3_000), 4)).toEqual(Array(4).fill(255));
      state.completed = true;
    },
    assert: async (_context, state) => expect(state.completed).toBe(true),
  });

  pairedScenario<{ completed: boolean }>({
    id: "CUE-001",
    title: "Record targets playbacks while decimal insertion and Record operations preserve tracking",
    arrange: async ({ api, bench }, surface) => {
      await loadCanonicalCopy(api, bench, `cue-001-record-and-replay-${surface}`, "compact-rig");
      await installCompactGroups(api);
      return { completed: false };
    },
    api: async ({ api, bench }, state) => {
      await setSequenceMasterFade(api, 0);
      const installed = await installPlaybackSequence(api, 1, [
        groupCue(1, [["1", "intensity", 1]]),
        groupCue(2, [
          ["2", "intensity", 1],
          ["2", "red", 0.2],
        ]),
      ]);

      await api.command("programmer.group.set", { group_id: "3", attribute: "intensity", value: 1 });
      await api.command("programmer.execute", { value: "RECORD SET 1 CUE 1.5" });
      let stored = await object<any>(api, "cue_list", installed.id);
      expect(stored.body.cues.map((cue: any) => cue.number)).toEqual([1, 1.5, 2]);
      expect(groupValues(stored.body.cues[0])).toEqual({ "1:intensity": 1 });
      expect(groupValues(stored.body.cues[1])).toEqual({ "3:intensity": 1 });
      expect(groupValues(stored.body.cues[2])).toEqual({ "2:intensity": 1, "2:red": 0.2 });

      const pageAddressed = await installPlaybackSequence(api, 2, [
        groupCue(1, [["1", "intensity", 1]]),
        groupCue(2, [
          ["2", "intensity", 1],
          ["2", "red", 0.2],
        ]),
      ]);
      await api.command("programmer.execute", { value: "RECORD SET 1 . 2 CUE 1.5" });
      const pageStored = await object<any>(api, "cue_list", pageAddressed.id);
      const cueSemantics = (body: any) => body.cues.map((cue: any) => ({ number: cue.number, values: groupValues(cue) }));
      expect(cueSemantics(pageStored.body)).toEqual(cueSemantics(stored.body));

      await api.command("programmer.clear", {});
      const trackedSequence = [
        [1, 0, 0],
        [1, 0, 1],
        [1, 1, 1],
      ];
      for (const groups of trackedSequence) {
        await api.request("POST", "/api/v1/cuelists/1/go", {});
        expect(logicalSlots(await bench.tick(0), 12)).toEqual(groups.flatMap((value) => Array(4).fill(value * 255)));
      }
      await api.request("POST", "/api/v1/cuelists/1/off", {});

      await api.command("programmer.group.set", { group_id: "2", attribute: "intensity", value: 0.8 });
      await api.command("programmer.execute", { value: "RECORD + SET 1 CUE 2" });
      stored = await object<any>(api, "cue_list", installed.id);
      expect(groupValues(stored.body.cues.find((cue: any) => cue.number === 2))).toEqual({ "2:intensity": 0.8, "2:red": 0.2 });

      await api.command("programmer.clear", {});
      await api.command("programmer.group.set", { group_id: "2", attribute: "red", value: 0.9 });
      await api.command("programmer.execute", { value: "RECORD - SET 1 CUE 2" });
      stored = await object<any>(api, "cue_list", installed.id);
      expect(groupValues(stored.body.cues.find((cue: any) => cue.number === 2))).toEqual({ "2:intensity": 0.8 });

      await api.command("programmer.clear", {});
      const beforeDelete = stored.body;
      const stream = await openEventStream(api);
      try {
        let mark = stream.events.length;
        await api.command("programmer.execute", { value: "RECORD - SET 1 CUE 2" });
        const recordMinusEvent = await showObjectEventAfter(stream.events, mark, installed.id);
        const afterRecordMinus = await object<any>(api, "cue_list", installed.id);
        const recordMinusRuntime = await playbackState(api);
        const recordMinusFrame = logicalSlots(await bench.tick(0), 12);
        expect(afterRecordMinus.body.cues.map((cue: any) => cue.number)).toEqual([1, 1.5]);

        await putObject(api, "cue_list", installed.id, beforeDelete, afterRecordMinus.revision);
        mark = stream.events.length;
        await api.command("programmer.execute", { value: "DELETE SET 1 CUE 2" });
        const deleteEvent = await showObjectEventAfter(stream.events, mark, installed.id);
        stored = await object<any>(api, "cue_list", installed.id);
        expect(stored.body).toEqual(afterRecordMinus.body);
        expect(eventIdentity(deleteEvent)).toEqual(eventIdentity(recordMinusEvent));
        expect(await playbackState(api)).toMatchObject({ active: recordMinusRuntime.active });
        expect(logicalSlots(await bench.tick(0), 12)).toEqual(recordMinusFrame);
      } finally {
        await closeWebSocket(stream.socket, "CUE-001 event stream");
      }

      await api.request("POST", "/api/v1/cuelists/1/go", {});
      expect(logicalSlots(await bench.tick(0), 12)).toEqual([...Array(4).fill(255), ...Array(8).fill(0)]);
      await api.request("POST", "/api/v1/cuelists/1/go", {});
      expect(logicalSlots(await bench.tick(0), 12)).toEqual([...Array(4).fill(255), ...Array(4).fill(0), ...Array(4).fill(255)]);
      state.completed = true;
    },
    ui: cue001Ui,
    assert: async (_context, state) => expect(state.completed).toBe(true),
  });

  pairedScenario<{ completed: boolean }>({
    id: "CUE-002",
    title: "Cue-only restoration reconstructs identically for sequential GO and direct jumps",
    arrange: () => ({ completed: false }),
    api: async ({ api, bench }, state) => {
      await loadCanonicalCopy(api, bench, "cue-002-cue-only", "compact-rig");
      await setSequenceMasterFade(api, 0);
      const fixtures = await fixtureIdsByNumber(api);
      const installed = await installPlaybackSequence(api, 1, [
        fixtureCue(1, [[fixtures[1], "intensity", 0.3]]),
        fixtureCue(2, [[fixtures[1], "intensity", 0.8]]),
        fixtureCue(3, [
          [fixtures[1], "intensity", 0.3, { automatic_restore: true }],
          [fixtures[2], "intensity", 0.6],
        ]),
      ]);
      expect((await object<any>(api, "cue_list", installed.id)).body.cues[2].changes[0].automatic_restore).toBe(true);

      const sequential: Array<[number, number]> = [];
      for (let index = 0; index < 3; index += 1) {
        await api.request("POST", "/api/v1/cuelists/1/go", {});
        await bench.tick(0);
        sequential.push([await visualizationLevel(api, fixtures[1], "intensity"), await visualizationLevel(api, fixtures[2], "intensity")]);
      }
      expect(sequential).toEqual([
        [0.3, 0],
        [0.8, 0],
        [0.3, 0.6],
      ]);

      const direct: Array<[number, number]> = [];
      for (const cueNumber of [1, 2, 3]) {
        await api.request("POST", "/api/v1/cuelists/1/off", {});
        await api.request("POST", "/api/v1/cuelists/1/go-to", { cue_number: cueNumber });
        await bench.tick(0);
        direct.push([await visualizationLevel(api, fixtures[1], "intensity"), await visualizationLevel(api, fixtures[2], "intensity")]);
      }
      expect(direct).toEqual(sequential);
      state.completed = true;
    },
    ui: async ({ api, bench, desk, page }, state) => {
      const show = await loadCanonicalCopy(api, bench, "cue-002-visible-cue-only", "compact-rig");
      await installCompactGroups(api);
      await setSequenceMasterFade(api, 0);
      const installed = await installPlaybackSequence(api, 1, [groupCue(1, [["1", "intensity", 0.3]])], { priority: 100 });
      await api.command("programmer.group.set", { group_id: "1", attribute: "intensity", value: 0.8 });
      await desk.open(bench.baseUrl);
      await page.locator(".mode-toggle").click();
      await setCueOnlyFromUi(page, true);
      await page.getByRole("button", { name: "REC", exact: true }).click();
      const card = page.locator(".playback-fader-bank article").filter({ hasText: "Playback 1" });
      await card.getByRole("button", { name: "Playback representation page 1 playback 1" }).click();
      await expect.poll(async () => (await object<any>(api, "cue_list", installed.id)).body.cues.length).toBe(2);
      await api.request("POST", `/api/v1/shows/${show.id}/open`, { transition: "hold_current" });
      await expect.poll(async () => (await object<any>(api, "cue_list", installed.id)).body.cues[1].cue_only).toBe(true);

      await api.command("programmer.clear", {});
      await api.command("programmer.group.set", { group_id: "2", attribute: "intensity", value: 0.6 });
      await setCueOnlyFromUi(page, false);
      await page.getByRole("button", { name: "REC", exact: true }).click();
      await card.getByRole("button", { name: "Playback representation page 1 playback 1" }).click();
      await expect.poll(async () => (await object<any>(api, "cue_list", installed.id)).body.cues.length).toBe(3);

      const stored = await object<any>(api, "cue_list", installed.id);
      expect(stored.body.cues[1].cue_only).toBe(true);
      expect(stored.body.cues[2].cue_only).toBe(false);
      expect(groupValues(stored.body.cues[2])).toEqual({ "1:intensity": 0.3, "2:intensity": 0.6 });
      expect(stored.body.cues[2].group_changes.find((change: any) => change.group_id === "1")).toMatchObject({ automatic_restore: true });

      await api.command("programmer.clear", {});
      await api.request("POST", "/api/v1/cuelists/1/off", {});
      const states: number[][] = [];
      for (let index = 0; index < 3; index += 1) {
        await api.request("POST", "/api/v1/cuelists/1/go", {});
        states.push(logicalSlots(await bench.tick(0), 8));
      }
      expect(states).toEqual([
        [...Array(4).fill(77), ...Array(4).fill(0)],
        [...Array(4).fill(204), ...Array(4).fill(0)],
        [...Array(4).fill(77), ...Array(4).fill(153)],
      ]);
      state.completed = true;
    },
    assert: async (_context, state) => expect(state.completed).toBe(true),
  });

  pairedScenario<{ completed: boolean }>({
    id: "CUE-003",
    title: "GO, pause, resume, back, and release use exact application-time boundaries",
    arrange: () => ({ completed: false }),
    api: async ({ api, bench }, state) => {
      await loadCanonicalCopy(api, bench, "cue-003-exact-timing", "compact-rig");
      await setSequenceMasterFade(api, 0);
      const fixture = (await fixtureIdsByNumber(api))[1];
      const installed = await installPlaybackSequence(api, 1, [
        fixtureCue(1, [[fixture, "intensity", 0]], { fade_millis: 0 }),
        fixtureCue(2, [[fixture, "intensity", 1]], { fade_millis: 4_000 }),
      ]);
      await api.request("POST", "/api/v1/cuelists/1/go", {});
      await api.request("POST", "/api/v1/cuelists/1/go", {});
      expect(slot(await bench.tick(0), 1)).toBe(0);
      expect(slot(await bench.tick(2_000), 1)).toBe(128);
      expect(slot(await bench.tick(2_000), 1)).toBe(255);

      await api.request("POST", "/api/v1/cuelists/1/off", {});
      await api.request("POST", "/api/v1/cuelists/1/go", {});
      await api.request("POST", "/api/v1/cuelists/1/go", {});
      expect(slot(await bench.tick(1_000), 1)).toBe(64);
      await api.request("POST", `/api/v1/playbacks/${installed.id}/pause`, {});
      const paused = await runtime(api, 1);
      expect(paused.paused).toBe(true);
      expect(slot(await bench.tick(10_000), 1)).toBe(64);
      expect((await runtime(api, 1)).activated_at).toBe(paused.activated_at);
      await api.request("POST", `/api/v1/playbacks/${installed.id}/go`, {});
      expect((await runtime(api, 1)).paused).toBe(false);
      expect(slot(await bench.tick(3_000), 1)).toBe(255);
      await api.request("POST", `/api/v1/playbacks/${installed.id}/back`, {});
      expect((await runtime(api, 1)).current_cue_number).toBe(1);
      expect(slot(await bench.tick(0), 1)).toBe(0);
      await api.request("POST", `/api/v1/playbacks/${installed.id}/release`, {});
      expect((await playbackState(api)).active).toHaveLength(0);
      state.completed = true;
    },
    ui: async ({ api, bench, desk, page }, state) => {
      await loadCanonicalCopy(api, bench, "cue-003-visible-pause", "compact-rig");
      await setSequenceMasterFade(api, 0);
      const fixture = (await fixtureIdsByNumber(api))[1];
      await installPlaybackSequence(api, 1, [fixtureCue(1, [[fixture, "intensity", 0]]), fixtureCue(2, [[fixture, "intensity", 1]], { fade_millis: 4_000 })]);
      await desk.open(bench.baseUrl);
      await page.locator(".mode-toggle").click();
      const card = page.locator(".playback-fader-bank article").filter({ hasText: "Playback 1" });
      await page.getByRole("button", { name: "SET", exact: true }).click();
      await card.getByRole("button", { name: "Playback representation page 1 playback 1" }).click();
      const configuration = page.getByRole("dialog", { name: "Playback Configuration" });
      await expect(configuration).toBeVisible();
      await configuration.getByRole("button", { name: "Playback Layout", exact: true }).click();
      await configuration
        .locator(".ui-form-field")
        .filter({ hasText: /^\s*Bottom button/ })
        .locator(".ui-select-trigger")
        .click();
      await page.getByRole("option", { name: "Pause", exact: true }).click();
      await configuration.getByRole("button", { name: "Apply", exact: true }).click();
      await expect(configuration).toBeHidden();
      await expect.poll(async () => (await object<any>(api, "playback", "1")).body.buttons).toEqual(["go_minus", "go", "pause"]);

      await card.getByRole("button", { name: "GO +", exact: true }).click();
      await card.getByRole("button", { name: "GO +", exact: true }).click();
      expect(slot(await bench.tick(1_000), 1)).toBe(64);
      await card.getByRole("button", { name: "PAUSE", exact: true }).click();
      await expect.poll(async () => runtime(api, 1)).toMatchObject({ current_cue_number: 2, paused: true });
      await expect(card.getByRole("button", { name: "RESUME", exact: true })).toHaveClass(/playback-button-active/);
      expect(slot(await bench.tick(10_000), 1)).toBe(64);
      await card.getByRole("button", { name: "RESUME", exact: true }).click();
      await expect.poll(async () => runtime(api, 1)).toMatchObject({ current_cue_number: 2, paused: false });
      expect(slot(await bench.tick(3_000), 1)).toBe(255);
      state.completed = true;
    },
    assert: async (_context, state) => expect(state.completed).toBe(true),
  });

  pairedScenario<{ completed: boolean }>({
    id: "CUE-004",
    title: "per-value timing overrides Cue fallback and Force Cue Timing is reversible",
    arrange: () => ({ completed: false }),
    api: async ({ api, bench }, state) => {
      await loadCanonicalCopy(api, bench, "cue-004-value-timing", "compact-rig");
      await setSequenceMasterFade(api, 0);
      const fixtures = await fixtureIdsByNumber(api);
      const installed = await installPlaybackSequence(api, 1, [
        fixtureCue(
          1,
          [
            [fixtures[1], "intensity", 0.5, { fade_millis: 2_000 }],
            [fixtures[2], "intensity", 0.7, { fade_millis: 4_000 }],
            [fixtures[3], "intensity", 0.6],
            [fixtures[4], "intensity", 0.8, { fade_millis: 1_000, delay_millis: 1_000 }],
          ],
          { fade_millis: 3_000, delay_millis: 500 },
        ),
      ]);

      await api.request("POST", "/api/v1/cuelists/1/go", {});
      await bench.tick(499);
      expect(await visualizationLevel(api, fixtures[1], "intensity")).toBe(0);
      expect(await visualizationLevel(api, fixtures[4], "intensity")).toBe(0);
      await bench.tick(501);
      expect(await visualizationLevel(api, fixtures[4], "intensity")).toBe(0);
      await bench.tick(1_000);
      expect(await visualizationLevel(api, fixtures[4], "intensity")).toBeCloseTo(0.8, 5);
      await bench.tick(500);
      expect(await visualizationLevel(api, fixtures[1], "intensity")).toBeCloseTo(0.5, 5);
      await bench.tick(1_000);
      expect(await visualizationLevel(api, fixtures[3], "intensity")).toBeCloseTo(0.6, 5);
      await bench.tick(1_000);
      expect(await visualizationLevel(api, fixtures[2], "intensity")).toBeCloseTo(0.7, 5);

      await api.request("POST", "/api/v1/cuelists/1/off", {});
      let stored = await object<any>(api, "cue_list", installed.id);
      const timingBytes = JSON.stringify(stored.body.cues[0].changes);
      await putObject(api, "cue_list", installed.id, { ...stored.body, force_cue_timing: true }, stored.revision);
      await api.request("POST", "/api/v1/cuelists/1/go", {});
      await bench.tick(2_500);
      expect(await visualizationLevel(api, fixtures[1], "intensity")).toBeCloseTo(1 / 3, 2);
      expect(await visualizationLevel(api, fixtures[4], "intensity")).toBeCloseTo((0.8 * 2) / 3, 2);
      await bench.tick(1_000);
      for (const [fixture, target] of [
        [fixtures[1], 0.5],
        [fixtures[2], 0.7],
        [fixtures[3], 0.6],
        [fixtures[4], 0.8],
      ] as const)
        expect(await visualizationLevel(api, fixture, "intensity")).toBeCloseTo(target, 5);
      stored = await object<any>(api, "cue_list", installed.id);
      expect(JSON.stringify(stored.body.cues[0].changes)).toBe(timingBytes);
      await putObject(api, "cue_list", installed.id, { ...stored.body, force_cue_timing: false }, stored.revision);
      await api.request("POST", "/api/v1/cuelists/1/off", {});
      await api.request("POST", "/api/v1/cuelists/1/go", {});
      await bench.tick(2_000);
      expect(await visualizationLevel(api, fixtures[1], "intensity")).toBeCloseTo(0.375, 5);
      expect(await visualizationLevel(api, fixtures[4], "intensity")).toBeCloseTo(0.8, 5);
      state.completed = true;
    },
    ui: async ({ api, bench, desk, page }, state) => {
      await loadCanonicalCopy(api, bench, "cue-004-005-visible-command-timing", "compact-rig");
      await installCompactGroups(api);
      await installPlaybackSequence(api, 1, [groupCue(10, [])]);
      const configuration = await api.request<any>("GET", "/api/v1/configuration");
      await api.request("PUT", "/api/v1/configuration", {
        ...configuration,
        programmer_fade_millis: 9_000,
        sequence_master_fade_millis: 0,
      });
      await desk.open(bench.baseUrl);

      await pressCommand(page, "GROUP 1 AT 50 TIME 2", "G1 AT 50 TIME 2");
      await expect.poll(async () => (await currentProgrammer(api)).group_values["1"].intensity.fade_millis).toBe(2_000);
      expect((await currentProgrammer(api)).group_values["1"].intensity.delay_millis).toBeUndefined();
      await pressCommand(page, "RECORD SET 1 CUE 1 TIME 3", "RECORD SET 1 CUE 1 TIME 3");
      await expect.poll(async () => (await object<any>(api, "cue_list", await cueListIdForPlayback(api, 1))).body.cues.length).toBe(2);
      let stored = await object<any>(api, "cue_list", await cueListIdForPlayback(api, 1));
      expect(stored.body.cues.find((cue: any) => cue.number === 1)).toMatchObject({
        number: 1,
        fade_millis: 3_000,
        trigger: { type: "manual" },
        group_changes: [{ group_id: "1", attribute: "intensity", fade_millis: 2_000 }],
      });
      state.completed = true;
    },
    assert: async (_context, state) => expect(state.completed).toBe(true),
  });

  const cue005Ui = async ({ api, bench, desk, page }: BenchUiContext, state: { completed: boolean }) => {
    await loadCanonicalCopy(api, bench, "cue-005-visible-triggers", "compact-rig");
    await installCompactGroups(api);
    await installPlaybackSequence(api, 1, [groupCue(10, [])]);
    await desk.open(bench.baseUrl);
    await pressCommand(page, "GROUP 1 AT 50", "G1 AT 50");
    await pressCommand(page, "RECORD SET 1 CUE 1", "RECORD SET 1 CUE 1");
    await pressCommand(page, "RECORD SET 1 CUE 2 TIME TIME 0", "RECORD SET 1 CUE 2 DELAY 0");
    await pressCommand(page, "RECORD SET 1 CUE 3 TIME TIME 4", "RECORD SET 1 CUE 3 DELAY 4");
    await expect.poll(async () => (await object<any>(api, "cue_list", await cueListIdForPlayback(api, 1))).body.cues.length).toBe(4);
    const stored = await object<any>(api, "cue_list", await cueListIdForPlayback(api, 1));
    expect(stored.body.cues.find((cue: any) => cue.number === 1).trigger).toEqual({ type: "manual" });
    expect(stored.body.cues.find((cue: any) => cue.number === 2).trigger).toEqual({ type: "follow", delay_millis: 0 });
    expect(stored.body.cues.find((cue: any) => cue.number === 3).trigger).toEqual({ type: "wait", delay_millis: 4_000 });
    state.completed = true;
  };

  pairedScenario<{ completed: boolean }>({
    id: "CUE-005",
    title: "GO, FOLLOW, and TIME measure from the preceding Cue's latest value endpoint",
    arrange: () => ({ completed: false }),
    api: async ({ api, bench }, state) => {
      const setup = async (name: string, trigger: any, multiValue = false) => {
        await loadCanonicalCopy(api, bench, name, "compact-rig");
        await setSequenceMasterFade(api, 0);
        const fixtures = await fixtureIdsByNumber(api);
        await installPlaybackSequence(api, 1, [
          fixtureCue(
            1,
            multiValue
              ? [
                  [fixtures[1], "intensity", 0.5, { fade_millis: 1_000 }],
                  [fixtures[2], "intensity", 0.5, { fade_millis: 3_000, delay_millis: 1_000 }],
                ]
              : [[fixtures[1], "intensity", 0.5]],
            { fade_millis: multiValue ? 0 : 2_000 },
          ),
          fixtureCue(2, [[fixtures[1], "intensity", 0.8]], { trigger }),
          fixtureCue(3, [[fixtures[1], "intensity", 0.2]]),
        ]);
      };

      await setup("cue-005-go", { type: "manual" });
      await api.request("POST", "/api/v1/cuelists/1/go", {});
      await bench.tick(2_000);
      await bench.tick(604_800_000);
      expect((await runtime(api, 1)).current_cue_number).toBe(1);
      await api.request("POST", "/api/v1/cuelists/1/go", {});
      expect((await runtime(api, 1)).current_cue_number).toBe(2);

      await setup("cue-005-follow", { type: "follow", delay_millis: 0 });
      await api.request("POST", "/api/v1/cuelists/1/go", {});
      await bench.tick(1_999);
      expect((await runtime(api, 1)).current_cue_number).toBe(1);
      await bench.tick(1);
      expect((await runtime(api, 1)).current_cue_number).toBe(2);

      await setup("cue-005-time", { type: "wait", delay_millis: 4_000 });
      await api.request("POST", "/api/v1/cuelists/1/go", {});
      await bench.tick(5_999);
      expect((await runtime(api, 1)).current_cue_number).toBe(1);
      await bench.tick(1);
      expect((await runtime(api, 1)).current_cue_number).toBe(2);
      await bench.tick(604_800_000);
      expect((await runtime(api, 1)).current_cue_number).toBe(2);

      await setup("cue-005-latest-value", { type: "follow", delay_millis: 0 }, true);
      await api.request("POST", "/api/v1/cuelists/1/go", {});
      await bench.tick(3_999);
      expect((await runtime(api, 1)).current_cue_number).toBe(1);
      await bench.tick(1);
      expect((await runtime(api, 1)).current_cue_number).toBe(2);
      state.completed = true;
    },
    ui: cue005Ui,
    assert: async (_context, state) => expect(state.completed).toBe(true),
  });

  pairedScenario<{ completed: boolean }>({
    id: "CUE-006",
    title: "explicit playback selection supplies the implicit Cuelist without following execution order",
    arrange: () => ({ completed: false }),
    api: async ({ api, bench }, state) => {
      await loadCanonicalCopy(api, bench, "cue-006-active-playback", "compact-rig");
      await setSequenceMasterFade(api, 0);
      const fixtures = await fixtureIdsByNumber(api);
      const first = await installPlaybackSequence(api, 1, [fixtureCue(1, [[fixtures[1], "intensity", 0.2]])]);
      const second = await installPlaybackSequence(api, 2, [fixtureCue(1, [[fixtures[2], "intensity", 0.3]])]);
      await api.request("POST", "/api/v1/cuelists/2/select", {});
      expect((await playbackState(api)).selected_playback).toBe(2);
      await api.request("POST", "/api/v1/cuelists/1/go", {});
      expect((await playbackState(api)).selected_playback).toBe(2);

      await api.command("programmer.set", { fixture_id: fixtures[3], attribute: "intensity", value: 0.7 });
      await api.command("programmer.execute", { value: "RECORD CUE 7" });
      expect((await object<any>(api, "cue_list", second.id)).body.cues.map((cue: any) => cue.number)).toEqual([1, 7]);
      expect((await object<any>(api, "cue_list", first.id)).body.cues.map((cue: any) => cue.number)).toEqual([1]);

      await api.command("programmer.clear", {});
      await api.command("programmer.set", { fixture_id: fixtures[4], attribute: "intensity", value: 0.6 });
      await api.command("programmer.execute", { value: "RECORD SET 1 CUE 8" });
      expect((await object<any>(api, "cue_list", first.id)).body.cues.map((cue: any) => cue.number)).toEqual([1, 8]);
      expect((await playbackState(api)).selected_playback).toBe(2);
      state.completed = true;
    },
    ui: async ({ api, bench, desk, page }, state) => {
      await loadCanonicalCopy(api, bench, "cue-006-active-playback-ui", "compact-rig");
      await setSequenceMasterFade(api, 0);
      const fixtures = await fixtureIdsByNumber(api);
      const first = await installPlaybackSequence(api, 1, [fixtureCue(1, [[fixtures[1], "intensity", 0.2]])], { name: "Selection One" });
      const second = await installPlaybackSequence(api, 2, [fixtureCue(1, [[fixtures[2], "intensity", 0.3]])], { name: "Selection Two" });
      await desk.open(bench.baseUrl);
      api.session = await page.evaluate(() => JSON.parse(localStorage.getItem("light.primary-session")!));
      await page.locator(".mode-toggle").click();
      await page.keyboard.press("Shift+KeyZ");
      await expect(page.getByLabel("Command line")).toHaveValue("SELECT");
      const firstCard = page.locator(".playback-fader-bank article").filter({ hasText: "Selection One" });
      const secondCard = page.locator(".playback-fader-bank article").filter({ hasText: "Selection Two" });
      await secondCard.getByRole("button", { name: "GO +", exact: true }).click();
      await expect(secondCard).toHaveAttribute("data-selected-playback", "true");
      expect((await playbackState(api)).active).toHaveLength(0);
      await firstCard.getByRole("button", { name: "GO +", exact: true }).click();
      await expect.poll(async () => runtime(api, 1)).toMatchObject({ current_cue_number: 1, enabled: true });
      expect((await playbackState(api)).selected_playback).toBe(2);
      await api.command("programmer.set", { fixture_id: fixtures[3], attribute: "intensity", value: 0.7 });
      await page.locator(".mode-toggle").click();
      await pressCommand(page, "RECORD CUE 7", "RECORD CUE 7");
      await expect.poll(async () => (await object<any>(api, "cue_list", second.id)).body.cues.map((cue: any) => cue.number)).toEqual([1, 7]);
      expect((await object<any>(api, "cue_list", first.id)).body.cues.map((cue: any) => cue.number)).toEqual([1]);
      expect((await playbackState(api)).selected_playback).toBe(2);
      state.completed = true;
    },
    assert: async (_context, state) => expect(state.completed).toBe(true),
  });

  pairedScenario<{ completed: boolean }>({
    id: "CUE-007",
    title: "explicit zeroes block a later inserted on Cue from tracking past Cue 4",
    arrange: () => ({ completed: false }),
    api: async ({ api, bench }, state) => {
      await loadCanonicalCopy(api, bench, "cue-007-explicit-off", "compact-rig");
      await installCompactGroups(api);
      await setSequenceMasterFade(api, 0);
      const installed = await installPlaybackSequence(api, 1, [
        groupCue(1, [["1", "intensity", 1]]),
        groupCue(2, [["1", "intensity", 0]]),
        groupCue(3, [["2", "intensity", 1]]),
        groupCue(3.5, [["1", "intensity", 1]]),
        groupCue(4, [["1", "intensity", 0]]),
        groupCue(5, [["3", "intensity", 1]]),
      ]);
      const stored = await object<any>(api, "cue_list", installed.id);
      expect(groupValues(stored.body.cues.find((cue: any) => cue.number === 2))).toEqual({ "1:intensity": 0 });
      expect(groupValues(stored.body.cues.find((cue: any) => cue.number === 4))).toEqual({ "1:intensity": 0 });
      const expected = [
        [1, 0, 0],
        [0, 0, 0],
        [0, 1, 0],
        [1, 1, 0],
        [0, 1, 0],
        [0, 1, 1],
      ];
      await assertCompactGroupSequence(bench, expected, () => api.request("POST", "/api/v1/cuelists/1/go", {}));
      state.completed = true;
    },
    ui: async ({ api, bench, desk, page }, state) => {
      await loadCanonicalCopy(api, bench, "cue-007-explicit-off-ui", "compact-rig");
      await installCompactGroups(api);
      await setSequenceMasterFade(api, 0);
      await installPlaybackSequence(api, 1, [
        groupCue(1, [["1", "intensity", 1]]),
        groupCue(2, [["1", "intensity", 0]]),
        groupCue(3, [["2", "intensity", 1]]),
        groupCue(3.5, [["1", "intensity", 1]]),
        groupCue(4, [["1", "intensity", 0]]),
        groupCue(5, [["3", "intensity", 1]]),
      ]);
      await desk.open(bench.baseUrl);
      await page.locator(".mode-toggle").click();
      const go = page.locator(".playback-fader-bank article").filter({ hasText: "Playback 1" }).getByRole("button", { name: "GO +", exact: true });
      await assertCompactGroupSequence(
        bench,
        [
          [1, 0, 0],
          [0, 0, 0],
          [0, 1, 0],
          [1, 1, 0],
          [0, 1, 0],
          [0, 1, 1],
        ],
        () => go.click(),
      );
      state.completed = true;
    },
    assert: async (_context, state) => expect(state.completed).toBe(true),
  });

  pairedScenario<{ completed: boolean }>({
    id: "CUE-010",
    title: "tracking and LTP ownership stay per attribute and reveal the underlying programmer",
    arrange: () => ({ completed: false }),
    api: async ({ api, bench }, state) => {
      await loadCanonicalCopy(api, bench, "cue-010-attribute-tracking", "compact-rig");
      await setSequenceMasterFade(api, 0);
      const fixtures = await fixtureIdsByNumber(api);
      const rgb = fixtures[21];
      await installPlaybackSequence(
        api,
        1,
        [
          fixtureCue(1, [[rgb, "intensity", 1]]),
          fixtureCue(2, [[rgb, "intensity", 0.5]]),
          fixtureCue(3, [
            [rgb, "red", 0],
            [rgb, "green", 0],
            [rgb, "blue", 1],
          ]),
          fixtureCue(4, [[fixtures[2], "intensity", 0.4]]),
        ],
        { priority: 100 },
      );
      for (const [attribute, value] of [
        ["red", 0],
        ["green", 1],
        ["blue", 0],
      ] as const)
        await api.command("programmer.set", { fixture_id: rgb, attribute, value });
      await bench.tick(1);

      await api.request("POST", "/api/v1/cuelists/1/go", {});
      await api.request("POST", "/api/v1/cuelists/1/go", {});
      await bench.tick(0);
      expect(await visualizationLevel(api, rgb, "intensity")).toBe(0.5);
      expect(await rgbValues(api, rgb)).toEqual([0, 1, 0]);

      await api.request("POST", "/api/v1/cuelists/1/go", {});
      await bench.tick(0);
      expect(await visualizationLevel(api, rgb, "intensity")).toBe(0.5);
      expect(await rgbValues(api, rgb)).toEqual([0, 0, 1]);
      await api.request("POST", "/api/v1/cuelists/1/go", {});
      await bench.tick(0);
      expect(await visualizationLevel(api, rgb, "intensity")).toBe(0.5);
      expect(await rgbValues(api, rgb)).toEqual([0, 0, 1]);

      await api.request("POST", "/api/v1/cuelists/1/off", {});
      await bench.tick(0);
      expect(await rgbValues(api, rgb)).toEqual([0, 1, 0]);
      state.completed = true;
    },
    ui: async ({ api, bench, desk, page }, state) => {
      await loadCanonicalCopy(api, bench, "cue-010-attribute-tracking-ui", "compact-rig");
      await setSequenceMasterFade(api, 0);
      const fixtures = await fixtureIdsByNumber(api);
      const rgb = fixtures[21];
      await installPlaybackSequence(
        api,
        1,
        [
          fixtureCue(1, [[rgb, "intensity", 1]]),
          fixtureCue(2, [[rgb, "intensity", 0.5]]),
          fixtureCue(3, [
            [rgb, "red", 0],
            [rgb, "green", 0],
            [rgb, "blue", 1],
          ]),
          fixtureCue(4, [[fixtures[2], "intensity", 0.4]]),
        ],
        { priority: 100 },
      );
      const definition = await object<any>(api, "playback", "1");
      await putObject(api, "playback", "1", { ...definition.body, buttons: ["go_minus", "go", "off"] }, definition.revision);
      for (const [attribute, value] of [
        ["red", 0],
        ["green", 1],
        ["blue", 0],
      ] as const)
        await api.command("programmer.set", { fixture_id: rgb, attribute, value });
      await bench.tick(1);
      await desk.open(bench.baseUrl);
      await page.locator(".mode-toggle").click();
      const card = page.locator(".playback-fader-bank article").filter({ hasText: "Playback 1" });
      const go = card.getByRole("button", { name: "GO +", exact: true });
      await go.click();
      await go.click();
      await bench.tick(0);
      expect(await visualizationLevel(api, rgb, "intensity")).toBe(0.5);
      expect(await rgbValues(api, rgb)).toEqual([0, 1, 0]);
      await go.click();
      await bench.tick(0);
      expect(await visualizationLevel(api, rgb, "intensity")).toBe(0.5);
      expect(await rgbValues(api, rgb)).toEqual([0, 0, 1]);
      await go.click();
      await bench.tick(0);
      expect(await rgbValues(api, rgb)).toEqual([0, 0, 1]);
      await card.getByRole("button", { name: "OFF", exact: true }).click();
      await bench.tick(0);
      expect(await rgbValues(api, rgb)).toEqual([0, 1, 0]);
      state.completed = true;
    },
    assert: async (_context, state) => expect(state.completed).toBe(true),
  });

  test("CUE-013 @supplemental-api › inactive deletion is output-neutral and both sole-Cue safeguards are atomic", async ({ api, bench }) => {
    await loadCanonicalCopy(api, bench, "cue-013-inactive-delete", "compact-rig");
    await installCompactGroups(api);
    await setSequenceMasterFade(api, 0);
    const installed = await installPlaybackSequence(api, 1, [groupCue(1, [["1", "intensity", 1]]), groupCue(2, [["2", "intensity", 1]]), groupCue(3, [["3", "intensity", 1]])]);
    await api.request("POST", "/api/v1/cuelists/1/go", {});
    const beforeRuntime = await runtime(api, 1);
    const beforeSlots = logicalSlots(await bench.tick(0), 12);
    const before = await object<any>(api, "cue_list", installed.id);
    await putObject(
      api,
      "cue_list",
      installed.id,
      {
        ...before.body,
        cues: before.body.cues.filter((cue: any) => cue.number !== 3),
      },
      before.revision,
    );
    expect(await runtime(api, 1)).toMatchObject({ current_cue_number: 1, activated_at: beforeRuntime.activated_at });
    expect(logicalSlots(await bench.tick(0), 12)).toEqual(beforeSlots);

    await api.command("programmer.execute", { value: "DELETE SET 1 CUE 1" });
    expect((await object<any>(api, "cue_list", installed.id)).body.cues.map((cue: any) => cue.number)).toEqual([2]);
    expect(await runtime(api, 1)).toMatchObject({
      current_cue_number: 1,
      deleted_cue_hold: { deleted_number: 1, next_number: 2 },
      normal_next_cue_number: 2,
    });
    expect(logicalSlots(await bench.tick(0), 12)).toEqual(beforeSlots);
    await api.request("POST", "/api/v1/cuelists/1/go", {});
    expect(await runtime(api, 1)).toMatchObject({ current_cue_number: 2 });
    expect(logicalSlots(await bench.tick(0), 12)).toEqual([...Array(4).fill(0), ...Array(4).fill(255), ...Array(4).fill(0)]);

    const fixtures = await fixtureIdsByNumber(api);
    const sole = await installPlaybackSequence(api, 2, [fixtureCue(1, [[fixtures[1], "intensity", 0.2]])]);
    const soleBefore = await object<any>(api, "cue_list", sole.id);
    await expect(api.command("programmer.execute", { value: "DELETE SET 2 CUE 1" })).rejects.toThrow();
    expect((await object<any>(api, "cue_list", sole.id)).body).toEqual(soleBefore.body);
    await api.command("programmer.clear", {});
    await expect(api.command("programmer.execute", { value: "RECORD - SET 2 CUE 1" })).rejects.toThrow();
    expect((await object<any>(api, "cue_list", sole.id)).body).toEqual(soleBefore.body);
  });

  test("MERGE-001 @api › two programmer identities arbitrate by priority, HTP magnitude, and stable LTP edit time", async ({ api, bench }) => {
    await loadCanonicalCopy(api, bench, "merge-001-two-programmers", "compact-rig");
    await setSequenceMasterFade(api, 0);
    const fixtures = await fixtureIdsByNumber(api);
    await api.request("POST", "/api/v1/users", { name: "Programmer A", enabled: true });
    await api.request("POST", "/api/v1/users", { name: "Programmer B", enabled: true });
    const first = new ApiDriver(api.baseUrl);
    const second = new ApiDriver(api.baseUrl);
    await first.login("Programmer A");
    await second.login("Programmer B");
    await first.command("programmer.priority", { priority: 0 });
    await second.command("programmer.priority", { priority: 0 });
    await first.command("programmer.set", { fixture_id: fixtures[1], attribute: "intensity", value: 0.4 });
    await bench.tick(1);
    await second.command("programmer.set", { fixture_id: fixtures[1], attribute: "intensity", value: 0.7 });
    expect(slot(await bench.tick(0), 1)).toBe(179);

    await first.command("programmer.priority", { priority: 10 });
    await second.command("programmer.priority", { priority: 20 });
    await first.command("programmer.set", { fixture_id: fixtures[1], attribute: "intensity", value: 0.9 });
    await second.command("programmer.set", { fixture_id: fixtures[1], attribute: "intensity", value: 0.2 });
    expect(slot(await bench.tick(0), 1)).toBe(51);

    const rgb = fixtures[21];
    await first.command("programmer.priority", { priority: 10 });
    await second.command("programmer.priority", { priority: 10 });
    await first.command("programmer.set", { fixture_id: rgb, attribute: "red", value: 0.4 });
    await bench.tick(1);
    await second.command("programmer.set", { fixture_id: rgb, attribute: "red", value: 0.8 });
    expect(await visualizationAfterTick(api, bench, rgb, "red", 0)).toBe(0.8);
    expect(await visualizationAfterTick(api, bench, rgb, "red", 0)).toBe(0.8);

    const diagnostics = await api.request<any>("GET", "/api/v1/diagnostics");
    expect(diagnostics.active_programmers.filter((programmer: any) => programmer.values.some((value: any) => value.fixture_id === rgb && value.attribute === "red"))).toHaveLength(
      2,
    );
    await second.command("programmer.release", { fixture_id: rgb, attribute: "red" });
    expect(await visualizationAfterTick(api, bench, rgb, "red", 0)).toBe(0.4);
  });

  pairedScenario<{ completed: boolean }>({
    id: "MERGE-002",
    title: "independent Sequences coexist and retrigger only their stored addresses",
    arrange: () => ({ completed: false }),
    api: async ({ api, bench }, state) => {
      await loadCanonicalCopy(api, bench, "merge-002-independent", "compact-rig");
      await setSequenceMasterFade(api, 0);
      const fixtures = await fixtureIdsByNumber(api);
      const aFixture = fixtures[21];
      const bFixture = fixtures[22];
      await installPlaybackSequence(
        api,
        1,
        [
          fixtureCue(1, [
            [aFixture, "intensity", 0.6],
            [aFixture, "red", 0],
            [aFixture, "green", 0],
            [aFixture, "blue", 1],
          ]),
        ],
        { name: "Sequence A", priority: 100 },
      );
      await installPlaybackSequence(
        api,
        2,
        [
          fixtureCue(1, [
            [bFixture, "intensity", 0.4],
            [bFixture, "red", 1],
            [bFixture, "green", 0.7],
            [bFixture, "blue", 0.4],
          ]),
        ],
        { name: "Sequence B", priority: 100 },
      );
      await api.request("POST", "/api/v1/cuelists/1/go", {});
      await bench.tick(1);
      await api.request("POST", "/api/v1/cuelists/2/go", {});
      await bench.tick(0);
      expect(await visualizationLevel(api, aFixture, "intensity")).toBe(0.6);
      expect(await visualizationLevel(api, bFixture, "intensity")).toBe(0.4);

      await api.command("programmer.set", { fixture_id: aFixture, attribute: "intensity", value: 0.3 });
      for (const [attribute, value] of [
        ["red", 1],
        ["green", 0],
        ["blue", 0],
      ] as const)
        await api.command("programmer.set", { fixture_id: aFixture, attribute, value });
      await bench.tick(0);
      expect(await visualizationLevel(api, aFixture, "intensity")).toBe(0.6);
      expect(await rgbValues(api, aFixture)).toEqual([1, 0, 0]);
      expect(await rgbValues(api, bFixture)).toEqual([1, 0.7, 0.4]);

      await bench.tick(1);
      await api.request("POST", "/api/v1/cuelists/1/go-to", { cue_number: 1 });
      await bench.tick(0);
      expect(await rgbValues(api, aFixture)).toEqual([0, 0, 1]);
      expect(await rgbValues(api, bFixture)).toEqual([1, 0.7, 0.4]);

      await api.command("programmer.set", { fixture_id: bFixture, attribute: "blue", value: 0.8 });
      await bench.tick(1);
      await api.request("POST", "/api/v1/cuelists/1/go-to", { cue_number: 1 });
      await bench.tick(0);
      expect(await visualizationLevel(api, bFixture, "blue")).toBe(0.8);

      await api.command("programmer.priority", { priority: 110 });
      await api.command("programmer.set", { fixture_id: aFixture, attribute: "red", value: 1 });
      await bench.tick(1);
      await api.request("POST", "/api/v1/cuelists/1/go-to", { cue_number: 1 });
      await bench.tick(0);
      expect(await visualizationLevel(api, aFixture, "red")).toBe(1);
      await api.command("programmer.priority", { priority: 90 });
      await api.request("POST", "/api/v1/cuelists/1/go-to", { cue_number: 1 });
      await bench.tick(0);
      expect(await visualizationLevel(api, aFixture, "red")).toBe(0);

      await api.command("programmer.clear", {});
      await api.request("POST", "/api/v1/cuelists/2/off", {});
      await bench.tick(0);
      expect(await visualizationLevel(api, aFixture, "intensity")).toBe(0.6);
      expect(await visualizationLevel(api, bFixture, "intensity")).toBe(0);
      await api.request("POST", "/api/v1/cuelists/1/off", {});
      await bench.tick(0);
      expect(await visualizationLevel(api, aFixture, "intensity")).toBe(0);
      state.completed = true;
    },
    ui: async ({ api, bench, desk, page }, state) => {
      await loadCanonicalCopy(api, bench, "merge-002-independent-ui", "compact-rig");
      await setSequenceMasterFade(api, 0);
      const fixtures = await fixtureIdsByNumber(api);
      const aFixture = fixtures[21];
      const bFixture = fixtures[22];
      await installPlaybackSequence(
        api,
        1,
        [
          fixtureCue(1, [
            [aFixture, "intensity", 0.6],
            [aFixture, "red", 0],
            [aFixture, "green", 0],
            [aFixture, "blue", 1],
          ]),
        ],
        { name: "Sequence A", priority: 100 },
      );
      await installPlaybackSequence(
        api,
        2,
        [
          fixtureCue(1, [
            [bFixture, "intensity", 0.4],
            [bFixture, "red", 1],
            [bFixture, "green", 0.7],
            [bFixture, "blue", 0.4],
          ]),
        ],
        { name: "Sequence B", priority: 100 },
      );
      await api.request("POST", "/api/v1/cuelists/1/go", {});
      await api.request("POST", "/api/v1/cuelists/2/go", {});
      await bench.tick(1);
      await api.command("programmer.set", { fixture_id: aFixture, attribute: "red", value: 1 });
      await api.command("programmer.set", { fixture_id: bFixture, attribute: "blue", value: 0.8 });
      await bench.tick(1);
      expect(await visualizationLevel(api, bFixture, "blue")).toBe(0.8);
      await desk.open(bench.baseUrl);
      api.session = await page.evaluate(() => JSON.parse(localStorage.getItem("light.primary-session")!));
      expect(await visualizationLevel(api, bFixture, "blue")).toBe(0.8);
      await page.locator(".mode-toggle").click();
      await page.keyboard.press("Shift+KeyZ");
      const first = page.locator(".playback-fader-bank article").filter({ hasText: "Sequence A" });
      await first.getByRole("button", { name: "GO +", exact: true }).click();
      await expect(first).toHaveAttribute("data-selected-playback", "true");
      expect(await visualizationLevel(api, bFixture, "blue")).toBe(0.8);
      await page.locator(".mode-toggle").click();
      await page.getByRole("button", { name: "CUE", exact: true }).click();
      await page.getByRole("button", { name: "1", exact: true }).click();
      await expect(page.getByLabel("Command line")).toHaveValue("CUE 1");
      await page.getByRole("button", { name: "ENT", exact: true }).click();
      await expect(page.getByLabel("Command line")).toHaveValue("FIXTURE");
      await bench.tick(0);
      expect(await rgbValues(api, aFixture)).toEqual([0, 0, 1]);
      expect(await visualizationLevel(api, bFixture, "blue")).toBe(0.8);
      expect(await visualizationLevel(api, aFixture, "intensity")).toBe(0.6);
      expect(await visualizationLevel(api, bFixture, "intensity")).toBe(0.4);
      state.completed = true;
    },
    assert: async (_context, state) => expect(state.completed).toBe(true),
  });

  pairedScenario<{ completed: boolean }>({
    id: "MERGE-003",
    title: "full normal overwrite auto-Offs while partial, disabled, Flash, and Temp restore",
    arrange: () => ({ completed: false }),
    api: async ({ api, bench }, state) => {
      const prepare = async (name: string, includeIntensity: boolean) => {
        await loadCanonicalCopy(api, bench, name, "compact-rig");
        await setSequenceMasterFade(api, 0);
        const fixture = (await fixtureIdsByNumber(api))[21];
        const underlying = await installPlaybackSequence(
          api,
          1,
          [fixtureCue(1, [...(includeIntensity ? [[fixture, "intensity", 1] as FixtureValue] : []), [fixture, "red", 0], [fixture, "green", 0], [fixture, "blue", 1]])],
          { name: "Underlying blue", auto_off: true },
        );
        await installPlaybackSequence(
          api,
          2,
          [
            fixtureCue(1, [
              [fixture, "red", 1],
              [fixture, "green", 0],
              [fixture, "blue", 0],
            ]),
          ],
          { name: "Replacing red", auto_off: false },
        );
        return { fixture, underlying };
      };

      let prepared = await prepare("merge-003-full", false);
      await api.request("POST", "/api/v1/cuelists/1/on", {});
      await bench.tick(1);
      await api.request("POST", "/api/v1/cuelists/2/on", {});
      await bench.tick(0);
      expect(await runtime(api, 1)).toMatchObject({ enabled: false });
      expect(await rgbValues(api, prepared.fixture)).toEqual([1, 0, 0]);

      let definition = await object<any>(api, "playback", "1");
      await putObject(api, "playback", "1", { ...definition.body, auto_off: false }, definition.revision);
      await api.request("POST", "/api/v1/cuelists/2/off", {});
      await api.request("POST", "/api/v1/cuelists/1/on", {});
      await bench.tick(1);
      await api.request("POST", "/api/v1/cuelists/2/on", {});
      await bench.tick(0);
      expect(await runtime(api, 1)).toMatchObject({ enabled: true });
      await api.request("POST", "/api/v1/cuelists/2/off", {});
      await bench.tick(0);
      expect(await rgbValues(api, prepared.fixture)).toEqual([0, 0, 1]);

      definition = await object<any>(api, "playback", "1");
      await putObject(api, "playback", "1", { ...definition.body, auto_off: true }, definition.revision);
      await api.request("POST", "/api/v1/cuelists/1/on", {});
      await api.request("POST", "/api/v1/cuelists/2/flash", { pressed: true });
      await bench.tick(0);
      expect(await runtime(api, 1)).toMatchObject({ enabled: true });
      expect(await rgbValues(api, prepared.fixture)).toEqual([1, 0, 0]);
      await api.request("POST", "/api/v1/cuelists/2/flash", { pressed: false });
      await bench.tick(0);
      expect(await rgbValues(api, prepared.fixture)).toEqual([0, 0, 1]);
      await api.request("POST", "/api/v1/cuelists/2/temp", {});
      await bench.tick(0);
      expect(await runtime(api, 1)).toMatchObject({ enabled: true });
      expect(await rgbValues(api, prepared.fixture)).toEqual([1, 0, 0]);
      await api.request("POST", "/api/v1/cuelists/2/temp", {});
      await bench.tick(0);
      expect(await rgbValues(api, prepared.fixture)).toEqual([0, 0, 1]);

      prepared = await prepare("merge-003-partial", true);
      await api.request("POST", "/api/v1/cuelists/1/on", {});
      await bench.tick(1);
      await api.request("POST", "/api/v1/cuelists/2/on", {});
      await bench.tick(0);
      expect(await runtime(api, 1)).toMatchObject({ enabled: true });
      expect(await visualizationLevel(api, prepared.fixture, "intensity")).toBe(1);
      expect(await rgbValues(api, prepared.fixture)).toEqual([1, 0, 0]);
      state.completed = true;
    },
    ui: async ({ api, bench, desk, page }, state) => {
      await loadCanonicalCopy(api, bench, "merge-003-visible-actions", "compact-rig");
      await setSequenceMasterFade(api, 0);
      const fixture = (await fixtureIdsByNumber(api))[21];
      const underlying = await installPlaybackSequence(
        api,
        1,
        [
          fixtureCue(1, [
            [fixture, "red", 0],
            [fixture, "green", 0],
            [fixture, "blue", 1],
          ]),
        ],
        { name: "Underlying blue", auto_off: true },
      );
      await installPlaybackSequence(
        api,
        2,
        [
          fixtureCue(1, [
            [fixture, "red", 1],
            [fixture, "green", 0],
            [fixture, "blue", 0],
          ]),
        ],
        { name: "Replacing red", auto_off: false },
      );
      for (const [number, buttons] of [
        [1, ["on", "off", "none"]],
        [2, ["on", "flash", "temp"]],
      ] as const) {
        const definition = await object<any>(api, "playback", String(number));
        await putObject(api, "playback", String(number), { ...definition.body, buttons }, definition.revision);
      }
      await desk.open(bench.baseUrl);
      await page.locator(".mode-toggle").click();
      const first = page.locator(".playback-fader-bank article").filter({ hasText: "Underlying blue" });
      const second = page.locator(".playback-fader-bank article").filter({ hasText: "Replacing red" });
      await first.getByRole("button", { name: "ON", exact: true }).click();
      await bench.tick(1);
      await second.getByRole("button", { name: "ON", exact: true }).click();
      await bench.tick(0);
      expect(await runtime(api, 1)).toMatchObject({ enabled: false });
      expect(await rgbValues(api, fixture)).toEqual([1, 0, 0]);

      await api.request("POST", "/api/v1/cuelists/2/off", {});
      await first.getByRole("button", { name: "ON", exact: true }).click();
      const flash = second.getByRole("button", { name: "FLASH", exact: true });
      await flash.hover();
      await page.mouse.down();
      await expect.poll(async () => rgbValues(api, fixture)).toEqual([1, 0, 0]);
      expect(await runtime(api, 1)).toMatchObject({ enabled: true });
      await page.mouse.up();
      await expect.poll(async () => rgbValues(api, fixture)).toEqual([0, 0, 1]);
      const temp = second.getByRole("button", { name: "TEMP", exact: true });
      await temp.click();
      await expect.poll(async () => rgbValues(api, fixture)).toEqual([1, 0, 0]);
      expect(await runtime(api, 1)).toMatchObject({ enabled: true });
      await temp.click();
      await expect.poll(async () => rgbValues(api, fixture)).toEqual([0, 0, 1]);

      const stored = await object<any>(api, "cue_list", underlying.id);
      stored.body.cues[0].changes.push({
        fixture_id: fixture,
        attribute: "intensity",
        value: { kind: "normalized", value: 1 },
        automatic_restore: false,
      });
      await putObject(api, "cue_list", underlying.id, stored.body, stored.revision);
      await api.request("POST", "/api/v1/cuelists/1/off", {});
      await api.request("POST", "/api/v1/cuelists/2/off", {});
      await first.getByRole("button", { name: "ON", exact: true }).click();
      await bench.tick(1);
      await second.getByRole("button", { name: "ON", exact: true }).click();
      await bench.tick(0);
      expect(await runtime(api, 1)).toMatchObject({ enabled: true });
      expect(await visualizationLevel(api, fixture, "intensity")).toBe(1);
      expect(await rgbValues(api, fixture)).toEqual([1, 0, 0]);
      state.completed = true;
    },
    assert: async (_context, state) => expect(state.completed).toBe(true),
  });
});

async function emptyPlaybackPage(api: ApiDriver) {
  const page = (await objects<any>(api, "playback_page")).find((item) => item.body.number === 1);
  await putObject(api, "playback_page", "1", { ...(page?.body ?? { number: 1, name: "Main" }), slots: {} }, page?.revision ?? 0);
}

async function installCompactGroups(api: ApiDriver) {
  const fixtures = await fixtureIdsByNumber(api);
  const existing = await objects<any>(api, "group");
  for (const [id, numbers] of [
    ["1", [1, 2, 3, 4]],
    ["2", [5, 6, 7, 8]],
    ["3", [9, 10, 11, 12]],
  ] as const) {
    const current = existing.find((item) => item.id === id);
    await putObject(
      api,
      "group",
      id,
      {
        ...(current?.body ?? {}),
        id,
        name: `Group ${id}`,
        fixtures: numbers.map((number) => fixtures[number]),
        derived_from: null,
        frozen_from: null,
        programming: current?.body.programming ?? {},
        master: 1,
        playback_fader: current?.body.playback_fader ?? null,
      },
      current?.revision ?? 0,
    );
  }
}

async function expectNewRecordedCuelist(api: ApiDriver, before: Set<string>, cueCount: number) {
  await expect.poll(async () => (await objects<any>(api, "cue_list")).filter((item) => !before.has(item.id)).length).toBe(1);
  const recorded = (await objects<any>(api, "cue_list")).find((item) => !before.has(item.id));
  expect(recorded).toBeDefined();
  await expect.poll(async () => (await object<any>(api, "cue_list", recorded!.id)).body.cues.length).toBe(cueCount);
  return object<any>(api, "cue_list", recorded!.id);
}

async function playbackAtSlot(api: ApiDriver, slot: number): Promise<number> {
  let playbackNumber: number | undefined;
  await expect
    .poll(async () => {
      playbackNumber = (await object<any>(api, "playback_page", "1")).body.slots[String(slot)];
      return playbackNumber;
    })
    .toEqual(expect.any(Number));
  return playbackNumber!;
}

async function playbackState(api: ApiDriver): Promise<any> {
  return api.request("GET", "/api/v1/playbacks");
}

async function runtime(api: ApiDriver, playbackNumber: number): Promise<any> {
  return (await playbackState(api)).active.find((item: any) => item.playback_number === playbackNumber);
}

function logicalSlots(frame: any, count: number): number[] {
  return (frame.universes.find((universe: any) => universe.universe === 1)?.slots ?? []).slice(0, count);
}

function slot(frame: any, fixtureNumber: number): number {
  return frame.universes.find((universe: any) => universe.universe === 1)?.slots[fixtureNumber - 1] ?? 0;
}

async function assertCompactGroupSequence(bench: any, expected: number[][], advance: () => Promise<unknown>) {
  for (const groups of expected) {
    await advance();
    expect(logicalSlots(await bench.tick(0), 12)).toEqual(groups.flatMap((value) => Array(4).fill(value * 255)));
  }
}

type ValueOptions = { automatic_restore?: boolean; fade_millis?: number; delay_millis?: number };
type CueOptions = { fade_millis?: number; delay_millis?: number; trigger?: any };
type FixtureValue = readonly [string, string, number, ValueOptions?];
type GroupValue = readonly [string, string, number];

function fixtureCue(number: number, values: readonly FixtureValue[], options: CueOptions = {}) {
  return {
    id: crypto.randomUUID(),
    number,
    name: `Cue ${number}`,
    changes: values.map(([fixture_id, attribute, value, timing]) => ({
      fixture_id,
      attribute,
      value: { kind: "normalized", value },
      automatic_restore: timing?.automatic_restore ?? false,
      ...(timing?.fade_millis == null ? {} : { fade_millis: timing.fade_millis }),
      ...(timing?.delay_millis == null ? {} : { delay_millis: timing.delay_millis }),
    })),
    group_changes: [],
    fade_millis: options.fade_millis ?? 0,
    delay_millis: options.delay_millis ?? 0,
    trigger: options.trigger ?? { type: "manual" },
    phasers: [],
  };
}

function groupCue(number: number, values: readonly GroupValue[], options: CueOptions = {}) {
  return {
    id: crypto.randomUUID(),
    number,
    name: `Cue ${number}`,
    changes: [],
    group_changes: values.map(([group_id, attribute, value]) => ({
      group_id,
      attribute,
      value: { kind: "normalized", value },
    })),
    fade_millis: options.fade_millis ?? 0,
    delay_millis: options.delay_millis ?? 0,
    trigger: options.trigger ?? { type: "manual" },
    phasers: [],
  };
}

async function installPlaybackSequence(
  api: ApiDriver,
  playbackNumber: number,
  cues: any[],
  options: { name?: string; priority?: number; intensity_priority_mode?: "htp" | "ltp"; auto_off?: boolean } = {},
) {
  const id = crypto.randomUUID();
  await putObject(api, "cue_list", id, {
    id,
    name: options.name ?? `Cuelist ${playbackNumber}`,
    priority: options.priority ?? 0,
    mode: "sequence",
    looped: false,
    chaser_step_millis: 1_000,
    speed_group: null,
    intensity_priority_mode: options.intensity_priority_mode ?? "htp",
    wrap_mode: "off",
    restart_mode: "first_cue",
    force_cue_timing: false,
    disable_cue_timing: false,
    chaser_xfade_millis: 0,
    speed_multiplier: 1,
    cues,
  });
  const existingPlayback = (await objects<any>(api, "playback")).find((item) => item.id === String(playbackNumber));
  await putObject(
    api,
    "playback",
    String(playbackNumber),
    {
      number: playbackNumber,
      name: options.name ?? `Playback ${playbackNumber}`,
      target: { type: "cue_list", cue_list_id: id },
      buttons: ["go_minus", "go", "flash"],
      button_count: 3,
      fader: "master",
      has_fader: true,
      go_activates: true,
      auto_off: options.auto_off ?? false,
      xfade_millis: 0,
      color: "#20c997",
      flash_release: "release_all",
      protect_from_swap: false,
      presentation_icon: null,
      presentation_image: null,
    },
    existingPlayback?.revision ?? 0,
  );
  const page = (await objects<any>(api, "playback_page")).find((item) => item.body.number === 1);
  await putObject(
    api,
    "playback_page",
    page?.id ?? "1",
    {
      ...(page?.body ?? { number: 1, name: "Main" }),
      slots: { ...(page?.body.slots ?? {}), [playbackNumber]: playbackNumber },
    },
    page?.revision ?? 0,
  );
  return { id, playbackNumber };
}

async function setSequenceMasterFade(api: ApiDriver, millis: number) {
  const configuration = await api.request<any>("GET", "/api/v1/configuration");
  await api.request("PUT", "/api/v1/configuration", {
    ...configuration,
    programmer_fade_millis: millis,
    sequence_master_fade_millis: millis,
  });
}

async function currentProgrammer(api: ApiDriver): Promise<any> {
  const programmers = await api.request<any[]>("GET", "/api/v1/programmers");
  return programmers.find((programmer) => programmer.session_id === api.session!.session_id);
}

async function cueListIdForPlayback(api: ApiDriver, playbackNumber: number): Promise<string> {
  const playback = await object<any>(api, "playback", String(playbackNumber));
  expect(playback.body.target.type).toBe("cue_list");
  return playback.body.target.cue_list_id;
}

async function visualizationLevel(api: ApiDriver, fixtureId: string, attribute: string): Promise<number> {
  const visualization = await api.request<any>("GET", "/api/v1/visualization");
  const value = visualization.values.find((item: any) => item.fixture_id === fixtureId && item.attribute === attribute)?.value;
  return rounded(typeof value === "number" ? value : (value?.value ?? 0));
}

async function visualizationAfterTick(api: ApiDriver, bench: any, fixtureId: string, attribute: string, millis: number): Promise<number> {
  await bench.tick(millis);
  return visualizationLevel(api, fixtureId, attribute);
}

async function rgbValues(api: ApiDriver, fixtureId: string): Promise<[number, number, number]> {
  return Promise.all(["red", "green", "blue"].map((attribute) => visualizationLevel(api, fixtureId, attribute))) as Promise<[number, number, number]>;
}

function groupValues(cue: any): Record<string, number> {
  return Object.fromEntries(cue.group_changes.map((change: any) => [`${change.group_id}:${change.attribute}`, rounded(change.value?.value)]));
}

function rounded(value: number): number {
  return Math.round(value * 1_000_000) / 1_000_000;
}

async function openEventStream(api: ApiDriver): Promise<{ socket: WebSocket; events: any[] }> {
  const socket = new WebSocket(api.baseUrl.replace(/^http/, "ws") + "/api/v1/events", ["light.v1", `light.token.${api.session!.token}`]);
  await new Promise<void>((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error("event stream connection timed out")), 5_000);
    socket.addEventListener(
      "open",
      () => {
        clearTimeout(timeout);
        resolve();
      },
      { once: true },
    );
    socket.addEventListener(
      "error",
      () => {
        clearTimeout(timeout);
        reject(new Error("event stream connection failed"));
      },
      { once: true },
    );
  });
  const events: any[] = [];
  socket.addEventListener("message", (message) => {
    const event = JSON.parse(String(message.data));
    if (event.kind) events.push(event);
  });
  return { socket, events };
}

async function showObjectEventAfter(events: any[], mark: number, id: string): Promise<any> {
  await expect
    .poll(() => events.slice(mark).find((event) => event.kind === "show_object_changed" && event.payload?.kind === "cue_list" && event.payload?.id === id) ?? null)
    .not.toBeNull();
  return events.slice(mark).find((event) => event.kind === "show_object_changed" && event.payload?.kind === "cue_list" && event.payload?.id === id);
}

function eventIdentity(event: any) {
  return { kind: event.kind, objectKind: event.payload.kind, id: event.payload.id };
}

async function setCueOnlyFromUi(page: any, checked: boolean) {
  const record = page.getByRole("button", { name: /REC(?: ARMED)?/, exact: true });
  await record.hover();
  await page.mouse.down();
  await page.waitForTimeout(700);
  await page.mouse.up();
  const dialog = page.locator(".store-settings-modal");
  await expect(dialog).toBeVisible();
  const cueOnly = dialog.getByLabel("Cue only");
  if ((await cueOnly.isChecked()) !== checked) {
    await dialog.locator("label").filter({ hasText: "Cue only" }).click();
  }
  if (checked) await expect(cueOnly).toBeChecked();
  else await expect(cueOnly).not.toBeChecked();
  await dialog.getByRole("button", { name: "Done", exact: true }).click();
  await expect(dialog).toBeHidden();
  await page.waitForTimeout(1_000);
}
