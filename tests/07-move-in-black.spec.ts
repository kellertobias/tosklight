import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import { pairedScenario } from "../apps/control-ui/e2e/bench/pairedScenario";
import { activeShowId, loadCanonicalCopy, object, objects, putObject } from "./support/catalog";

interface MibState {
  enabledFixture: string;
  disabledFixture: string;
  cueListId: string;
}

test.describe("MIB-001 · docs/testing/02-cues-tracking-and-arbitration.md", () => {
  pairedScenario<MibState>({
    id: "MIB-001",
    title: "a dark fixture prepositions for its next lit Cue",
    arrange: async ({ api, bench }, surface) => {
      await loadCanonicalCopy(api, bench, `mib-001-${surface}`, "default-stage");
      const patch = await api.request<any>("GET", "/api/v1/patch", undefined, false);
      for (const fixture of patch.fixtures) {
        expect(fixture.move_in_black_enabled).toBe(true);
        expect(fixture.move_in_black_delay_millis).toBe(0);
      }
      const patched = await objects<any>(api, "patched_fixture");
      const enabled = patched.find((entry) => entry.body.fixture_number === 101)!;
      const disabled = patched.find((entry) => entry.body.fixture_number === 102)!;
      await putObject(api, "patched_fixture", enabled.id, {
        ...enabled.body,
        move_in_black_enabled: true,
        move_in_black_delay_millis: 1_000,
      }, enabled.revision);
      await putObject(api, "patched_fixture", disabled.id, {
        ...disabled.body,
        move_in_black_enabled: false,
        move_in_black_delay_millis: 1_000,
      }, disabled.revision);
      const cueListId = await installMibCuelist(api, enabled.id, disabled.id);

      const showId = await activeShowId(api);
      await api.request("POST", `/api/v1/shows/${showId}/open`, { transition: "hold_current" });
      const restoredEnabled = await object<any>(api, "patched_fixture", enabled.id);
      const restoredDisabled = await object<any>(api, "patched_fixture", disabled.id);
      expect([restoredEnabled.body.move_in_black_enabled, restoredEnabled.body.move_in_black_delay_millis]).toEqual([true, 1_000]);
      expect([restoredDisabled.body.move_in_black_enabled, restoredDisabled.body.move_in_black_delay_millis]).toEqual([false, 1_000]);
      return { enabledFixture: enabled.id, disabledFixture: disabled.id, cueListId };
    },
    api: async ({ api, bench }, state) => runExactTiming(api, bench, state),
    ui: async ({ api, bench, desk, page }, state) => {
      await desk.open(bench.baseUrl);
      await openPatch(page);
      const enabled = page.getByLabel("Move in Black 101");
      const disabled = page.getByLabel("Move in Black 102");
      await expect(enabled).toBeChecked();
      await expect(disabled).not.toBeChecked();
      await expect(page.getByLabel("MIB Delay 101")).toHaveValue("1");
      await expect(page.getByLabel("MIB Delay 102")).toHaveValue("1");

      await enabled.click();
      await expect.poll(async () => (await object<any>(api, "patched_fixture", state.enabledFixture)).body.move_in_black_enabled).toBe(false);
      await enabled.click();
      await expect.poll(async () => (await object<any>(api, "patched_fixture", state.enabledFixture)).body.move_in_black_enabled).toBe(true);
      await page.reload();
      await expect(page.locator(".connection-cover")).toBeHidden({ timeout: 10_000 });
      await openPatch(page);
      await expect(page.getByLabel("Move in Black 101")).toBeChecked();
      await expect(page.getByLabel("MIB Delay 101")).toHaveValue("1");
      await runExactTiming(api, bench, state);
    },
    assert: async ({ api, bench }, state) => {
      const diagnostics = await mibDiagnostics(api);
      const enabled = mibFor(diagnostics, state.enabledFixture, state.cueListId);
      const disabled = mibFor(diagnostics, state.disabledFixture, state.cueListId);
      expect(enabled.state).toBe("completed");
      expect(enabled.target_cue_number).toBe(3);
      expect(disabled.state).toBe("disabled");
      const frame = await bench.tick(0);
      expect(frame.universes.find((entry: any) => entry.universe === 2)!.slots[1]).toBe(204);
    },
  });

  test("MIB-001 @wire › resolved light blocks MIB and every return to dark restarts the full safety delay", async ({ api, bench }) => {
    await loadCanonicalCopy(api, bench, "mib-001-resolved-dark", "default-stage");
    const patched = await objects<any>(api, "patched_fixture");
    const enabled = patched.find((entry) => entry.body.fixture_number === 101)!;
    const comparison = patched.find((entry) => entry.body.fixture_number === 102)!;
    await putObject(api, "patched_fixture", enabled.id, {
      ...enabled.body,
      move_in_black_enabled: true,
      move_in_black_delay_millis: 1_000,
    }, enabled.revision);
    const mibCueListId = await installMibCuelist(api, enabled.id, comparison.id);
    const blockerCueListId = await installIntensityBlocker(api, enabled.id);

    await api.request("POST", "/api/v1/test/clock/reset", undefined, false);
    await api.request("POST", `/api/v1/playbacks/${mibCueListId}/release`, {});
    await api.request("POST", `/api/v1/playbacks/${blockerCueListId}/release`, {});
    await api.request("POST", "/api/v1/cuelists/1/off", {});
    await api.request("POST", "/api/v1/cuelists/2/off", {});
    await api.request("POST", "/api/v1/cuelists/2/go", {});
    await api.request("POST", "/api/v1/cuelists/1/go", {});
    await api.request("POST", "/api/v1/cuelists/1/go", {});
    await bench.tick(5_000);

    let enabledRuntime = mibFor(await mibDiagnostics(api), enabled.id, mibCueListId);
    expect(enabledRuntime).toMatchObject({ state: "blocked", dark_since: null, delay_deadline: null });
    expect(enabledRuntime.positions[0].current.value).toBeCloseTo(0.2, 4);

    await api.request("POST", "/api/v1/cuelists/2/off", {});
    await bench.tick(0);
    enabledRuntime = mibFor(await mibDiagnostics(api), enabled.id, mibCueListId);
    expect(enabledRuntime.state).toBe("delaying");
    const firstDarkSince = Date.parse(enabledRuntime.dark_since);
    expect(Date.parse(enabledRuntime.delay_deadline) - firstDarkSince).toBe(1_000);

    await bench.tick(500);
    await api.request("POST", "/api/v1/cuelists/2/go", {});
    await bench.tick(1);
    enabledRuntime = mibFor(await mibDiagnostics(api), enabled.id, mibCueListId);
    expect(enabledRuntime).toMatchObject({ state: "blocked", dark_since: null });

    await bench.tick(499);
    await api.request("POST", "/api/v1/cuelists/2/off", {});
    await bench.tick(0);
    enabledRuntime = mibFor(await mibDiagnostics(api), enabled.id, mibCueListId);
    expect(enabledRuntime.state).toBe("delaying");
    const restartedDarkSince = Date.parse(enabledRuntime.dark_since);
    expect(restartedDarkSince).toBeGreaterThan(firstDarkSince);
    expect(Date.parse(enabledRuntime.delay_deadline) - restartedDarkSince).toBe(1_000);

    let frame = await bench.tick(999);
    expect(frame.universes.find((entry: any) => entry.universe === 2)!.slots[1]).toBe(51);
    frame = await bench.tick(1);
    enabledRuntime = mibFor(await mibDiagnostics(api), enabled.id, mibCueListId);
    expect(enabledRuntime.state).toBe("moving");
    expect(frame.universes.find((entry: any) => entry.universe === 2)!.slots[1]).toBe(51);
  });

  test("MIB-001 @wire › dark-Cue look-ahead retargets live edits and cancels when the future lit Cue disappears", async ({ api, bench }) => {
    await loadCanonicalCopy(api, bench, "mib-001-invalidation", "default-stage");
    const patched = await objects<any>(api, "patched_fixture");
    const enabled = patched.find((entry) => entry.body.fixture_number === 101)!;
    const comparison = patched.find((entry) => entry.body.fixture_number === 102)!;
    await putObject(api, "patched_fixture", enabled.id, {
      ...enabled.body,
      move_in_black_enabled: true,
      move_in_black_delay_millis: 1_000,
    }, enabled.revision);
    const cueListId = await installMibCuelist(api, enabled.id, comparison.id);

    const initial = await object<any>(api, "cue_list", cueListId);
    const extraDarkCue = {
      id: crypto.randomUUID(),
      number: 2.5,
      name: "Another dark Cue",
      changes: [],
      group_changes: [],
      fade_millis: 0,
      delay_millis: 0,
      trigger: { type: "manual" },
      phasers: [],
    };
    const withDarkChain = { ...initial.body, cues: [...initial.body.cues.slice(0, 2), extraDarkCue, initial.body.cues[2]] };
    await putObject(api, "cue_list", cueListId, withDarkChain, initial.revision);

    await api.request("POST", "/api/v1/test/clock/reset", undefined, false);
    await api.request("POST", "/api/v1/cuelists/1/go", {});
    await api.request("POST", "/api/v1/cuelists/1/go", {});
    await bench.tick(2_000);
    let enabledRuntime = mibFor(await mibDiagnostics(api), enabled.id, cueListId);
    expect(enabledRuntime).toMatchObject({ state: "delaying", current_cue_number: 2, target_cue_number: 3 });
    expect(enabledRuntime.positions[0].target.value).toBeCloseTo(0.8, 4);

    await bench.tick(500);
    const beforeRetarget = await object<any>(api, "cue_list", cueListId);
    const retargeted = structuredClone(beforeRetarget.body);
    const futurePan = retargeted.cues
      .find((cue: any) => cue.number === 3)
      .changes.find((change: any) => change.fixture_id === enabled.id && change.attribute === "pan");
    futurePan.value.value = 0.6;
    await putObject(api, "cue_list", cueListId, retargeted, beforeRetarget.revision);
    await bench.tick(0);
    enabledRuntime = mibFor(await mibDiagnostics(api), enabled.id, cueListId);
    expect(enabledRuntime).toMatchObject({ state: "delaying", target_cue_number: 3, cancellation_reason: "future_target_recalculated" });
    expect(enabledRuntime.positions[0].target.value).toBeCloseTo(0.6, 4);

    const afterRetarget = await object<any>(api, "cue_list", cueListId);
    for (const darkCue of afterRetarget.body.cues.filter((cue: any) => cue.number === 2 || cue.number === 2.5)) {
      expect(darkCue.changes.some((change: any) => change.fixture_id === enabled.id && change.attribute === "pan")).toBe(false);
    }

    await bench.tick(1_000);
    enabledRuntime = mibFor(await mibDiagnostics(api), enabled.id, cueListId);
    expect(enabledRuntime.state).toBe("moving");
    expect(enabledRuntime.positions[0].current.value).toBeGreaterThan(0.2);

    const beforeDelete = await object<any>(api, "cue_list", cueListId);
    await putObject(api, "cue_list", cueListId, {
      ...beforeDelete.body,
      cues: beforeDelete.body.cues.filter((cue: any) => cue.number !== 3),
    }, beforeDelete.revision);
    const revealed = await bench.tick(0);
    enabledRuntime = mibFor(await mibDiagnostics(api), enabled.id, cueListId);
    expect(enabledRuntime).toMatchObject({ state: "cancelled", cancellation_reason: "future_target_invalidated" });
    expect(revealed.universes.find((entry: any) => entry.universe === 2)!.slots[1]).toBe(51);
  });
});

async function installMibCuelist(api: any, enabledFixture: string, disabledFixture: string): Promise<string> {
  const cueListId = crypto.randomUUID();
  const cue = (number: number, changes: any[], fade_millis = 0) => ({
    id: crypto.randomUUID(), number, name: `Cue ${number}`, changes, group_changes: [],
    fade_millis, delay_millis: 0, trigger: { type: "manual" }, phasers: [],
  });
  const set = (fixture_id: string, attribute: string, value: number, fade_millis?: number) => ({
    fixture_id, attribute, value: { kind: "normalized", value }, automatic_restore: false,
    ...(fade_millis == null ? {} : { fade_millis }),
  });
  const fixtures = [enabledFixture, disabledFixture];
  await putObject(api, "cue_list", cueListId, {
    id: cueListId, name: "Move in Black", priority: 10, mode: "sequence", looped: false,
    chaser_step_millis: 1_000, speed_group: null, intensity_priority_mode: "htp",
    wrap_mode: "off", restart_mode: "first_cue", force_cue_timing: false,
    disable_cue_timing: false, chaser_xfade_millis: 0, speed_multiplier: 1,
    cues: [
      cue(1, fixtures.flatMap((fixture) => [set(fixture, "intensity", 1), set(fixture, "pan", 0.2)])),
      cue(2, fixtures.map((fixture) => set(fixture, "intensity", 0)), 2_000),
      cue(3, fixtures.flatMap((fixture) => [set(fixture, "intensity", 1), set(fixture, "pan", 0.8, 3_000)])),
    ],
  });
  await putObject(api, "playback", "1", {
    number: 1, name: "MIB", target: { type: "cue_list", cue_list_id: cueListId },
    buttons: ["go_minus", "go", "flash"], fader: "master", go_activates: true,
    auto_off: true, xfade_millis: 0, color: "#20c997", flash_release: "release_all",
    protect_from_swap: false,
  });
  return cueListId;
}

async function installIntensityBlocker(api: any, fixtureId: string): Promise<string> {
  const cueListId = crypto.randomUUID();
  await putObject(api, "cue_list", cueListId, {
    id: cueListId,
    name: "MIB resolved-light blocker",
    priority: 20,
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
      id: crypto.randomUUID(),
      number: 1,
      name: "Keep fixture visibly lit",
      changes: [{
        fixture_id: fixtureId,
        attribute: "intensity",
        value: { kind: "normalized", value: 0.2 },
        automatic_restore: false,
      }],
      group_changes: [],
      fade_millis: 0,
      delay_millis: 0,
      trigger: { type: "manual" },
      phasers: [],
    }],
  });
  await putObject(api, "playback", "2", {
    number: 2,
    name: "MIB blocker",
    target: { type: "cue_list", cue_list_id: cueListId },
    buttons: ["go_minus", "go", "flash"],
    fader: "master",
    go_activates: true,
    auto_off: true,
    xfade_millis: 0,
    color: "#f59e0b",
    flash_release: "release_all",
    protect_from_swap: false,
  });
  return cueListId;
}

async function runExactTiming(api: any, bench: any, state: MibState) {
  await api.request("POST", "/api/v1/test/clock/reset", undefined, false);
  await api.request("POST", `/api/v1/playbacks/${state.cueListId}/release`, {});
  await api.request("POST", "/api/v1/cuelists/1/off", {});
  await api.request("POST", "/api/v1/cuelists/1/go", {});
  await api.request("POST", "/api/v1/cuelists/1/go", {});

  let frame = await bench.tick(1_999);
  let diagnostics = await mibDiagnostics(api);
  expect(frame.universes.find((entry: any) => entry.universe === 2)!.slots[1]).toBe(51);
  let enabled = mibFor(diagnostics, state.enabledFixture, state.cueListId);
  expect(enabled.state).toBe("blocked");
  expect(enabled.dark_since).toBeNull();

  const darkFrame = await bench.tick(1);
  diagnostics = await mibDiagnostics(api);
  enabled = mibFor(diagnostics, state.enabledFixture, state.cueListId);
  const disabled = mibFor(diagnostics, state.disabledFixture, state.cueListId);
  expect(darkFrame.universes.find((entry: any) => entry.universe === 2)!.slots[0]).toBe(0);
  expect(enabled.state).toBe("delaying");
  expect(Date.parse(enabled.delay_deadline) - Date.parse(enabled.dark_since)).toBe(1_000);
  expect(disabled.state).toBe("disabled");

  frame = await bench.tick(999);
  expect(frame.universes.find((entry: any) => entry.universe === 2)!.slots[1]).toBe(51);
  frame = await bench.tick(1);
  diagnostics = await mibDiagnostics(api);
  enabled = mibFor(diagnostics, state.enabledFixture, state.cueListId);
  expect(enabled.state).toBe("moving");
  expect(frame.universes.find((entry: any) => entry.universe === 2)!.slots[1]).toBe(51);

  frame = await bench.tick(1_500);
  let universe = frame.universes.find((entry: any) => entry.universe === 2)!.slots;
  expect(universe[1]).toBe(128);
  expect(universe[7]).toBe(51);
  frame = await bench.tick(1_500);
  universe = frame.universes.find((entry: any) => entry.universe === 2)!.slots;
  expect(universe[1]).toBe(204);
  expect(universe[7]).toBe(51);
  diagnostics = await mibDiagnostics(api);
  expect(mibFor(diagnostics, state.enabledFixture, state.cueListId).state).toBe("completed");

  await api.request("POST", "/api/v1/cuelists/1/go", {});
  frame = await bench.tick(0);
  universe = frame.universes.find((entry: any) => entry.universe === 2)!.slots;
  expect(universe[1]).toBe(204);
  expect(universe[7]).toBe(51);
  frame = await bench.tick(1_500);
  universe = frame.universes.find((entry: any) => entry.universe === 2)!.slots;
  expect(universe[1]).toBe(204);
  expect(universe[7]).toBe(128);
}

async function mibDiagnostics(api: any): Promise<any[]> {
  return (await api.request<any>("GET", "/api/v1/diagnostics")).move_in_black;
}

function mibFor(diagnostics: any[], fixtureId: string, cueListId: string): any {
  const runtime = diagnostics.find((entry) => entry.fixture_id === fixtureId && entry.cue_list_id === cueListId);
  expect(runtime, `MIB diagnostics for fixture ${fixtureId} and Cuelist ${cueListId}`).toBeDefined();
  return runtime;
}

async function openPatch(page: any): Promise<void> {
  if (await page.getByLabel("Move in Black 101").count()) return;
  await page.getByRole("button", { name: /Open show menu/ }).click();
  await page.getByRole("button", { name: "Show Patch", exact: true }).click();
  await expect(page.getByLabel("Move in Black 101")).toBeVisible();
}
