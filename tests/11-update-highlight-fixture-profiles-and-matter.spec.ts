import { expect } from "../apps/control-ui/e2e/bench/fixtures";
import type { Page } from "../apps/control-ui/node_modules/@playwright/test/index.js";
import { pairedScenario } from "../apps/control-ui/e2e/bench/pairedScenario";
import { blankFixtureProfile } from "../apps/control-ui/src/components/setup/fixtureProfileModel";
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
    await api.command("programmer.execute", { value: `RECORD GROUP ${state.singletonGroupId}` });
    await highlightAction(api, "all");
    await api.command("programmer.execute", { value: `RECORD GROUP ${state.completeGroupId}` });

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
    preset: { name: "Highlight isolation", family: "All", values, group_values: {} },
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
