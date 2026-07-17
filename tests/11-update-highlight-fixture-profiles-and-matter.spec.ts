import { expect } from "../apps/control-ui/e2e/bench/fixtures";
import type { Page } from "../apps/control-ui/node_modules/@playwright/test/index.js";
import { pairedScenario } from "../apps/control-ui/e2e/bench/pairedScenario";
import { blankFixtureProfile } from "../apps/control-ui/src/components/setup/fixtureProfileModel";
import {
  loadCanonicalCopy,
  object,
  objects,
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

interface HighlightScenarioState {
  fixtures: string[];
}

interface HighlightSurfaceState {
  showId: string;
  fixtures: Array<{ id: string; number: number }>;
  group: { id: string; name: string; fixtures: string[] };
  storedPresetId?: string;
  reconnectRetained?: boolean;
  fixtureCaptureObserved?: boolean;
  stageCaptureObserved?: boolean;
  groupCaptureObserved?: boolean;
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
  title: "Highlight captures an ordered selection and steps without becoming programmer data",
  arrange: async ({ api, bench }, surface) => {
    await loadCanonicalCopy(api, bench, `highlight-001-${surface}`);
    const fixtures = (await objects<any>(api, "patched_fixture")).slice(0, 3).map((entry) => entry.body.fixture_id as string);
    expect(fixtures).toHaveLength(3);
    await api.command("selection.set", { fixtures });
    await api.command("programmer.set", { fixture_id: fixtures[0], attribute: "position.pan", value: 0.63 });
    return { fixtures };
  },
  api: async ({ api }) => {
    await api.request("POST", "/api/v1/highlight/action", { action: "on" });
    await api.request("POST", "/api/v1/highlight/action", { action: "next" });
    const first = (await programmer(api)).selected[0];
    await api.command("programmer.set", { fixture_id: first, attribute: "position.pan", value: 0.41 });
    await new Promise((resolve) => setTimeout(resolve, 160));
    await api.request("POST", "/api/v1/highlight/action", { action: "next" });
    const second = (await programmer(api)).selected[0];
    await api.command("programmer.set", { fixture_id: second, attribute: "position.pan", value: 0.52 });
    await api.request("POST", "/api/v1/highlight/action", { action: "previous" });
  },
  ui: async ({ api, bench, desk, page }, state) => {
    await desk.open(bench.baseUrl);
    await expect.poll(async () => (await programmer(api)).selected.length).toBe(3);
    await page.getByRole("button", { name: "Turn Highlight on" }).click();
    const next = page.getByRole("button", { name: "Next highlighted fixture" });
    const previous = page.getByRole("button", { name: "Previous highlighted fixture" });
    await expect(next).toBeEnabled();
    await next.click();
    await expect(page.locator(".highlight-toggle small")).toContainText("1/3");
    await api.command("programmer.set", { fixture_id: state.fixtures[0], attribute: "position.pan", value: 0.41 });
    await page.waitForTimeout(160);
    await next.click();
    await expect(page.locator(".highlight-toggle small")).toContainText("2/3");
    await api.command("programmer.set", { fixture_id: state.fixtures[1], attribute: "position.pan", value: 0.52 });
    await previous.click();
    await expect(page.locator(".highlight-toggle small")).toContainText("1/3");
  },
  assert: async ({ api }, state) => {
    const highlight = await api.request<any>("GET", "/api/v1/highlight", undefined, true);
    expect(highlight).toMatchObject({ active: true, mode: "step", active_index: 0, can_previous: false, can_next: true });
    expect(highlight.remembered.map((fixture: any) => fixture.fixture_id)).toEqual(state.fixtures);
    expect(highlight.active_fixture.fixture_id).toBe(state.fixtures[0]);
    const programmers = await api.request<any[]>("GET", "/api/v1/programmers", undefined, false);
    expect(programmers.some((entry) => entry.selected.length === 1 && entry.selected[0] === state.fixtures[0])).toBe(true);
    const values = programmers.flatMap((entry) => entry.values ?? []);
    expect(values.some((entry) => entry.fixture_id === state.fixtures[0] && entry.attribute === "position.pan")).toBe(true);
    expect(values.some((entry) => entry.fixture_id === state.fixtures[1] && entry.attribute === "position.pan")).toBe(true);
    expect(values.every((entry) => !String(entry.attribute).toLowerCase().includes("highlight"))).toBe(true);
  },
});

pairedScenario<HighlightSurfaceState>({
  id: "HIGHLIGHT-002",
  title: "fixture, stage, and Group captures remain transient across store, reconnect, and show reload",
  arrange: async ({ api, bench }, surface) => {
    const show = await loadCanonicalCopy(api, bench, `highlight-002-${surface}`);
    const patched = (await objects<any>(api, "patched_fixture"))
      .map((entry) => ({ id: entry.body.fixture_id as string, number: Number(entry.body.fixture_number) }))
      .filter((entry) => Number.isFinite(entry.number))
      .sort((left, right) => left.number - right.number);
    const group = (await objects<any>(api, "group")).find((entry) => entry.body.fixtures.length >= 2);
    expect(patched.length).toBeGreaterThanOrEqual(3);
    expect(group).toBeDefined();
    return {
      showId: show.id,
      fixtures: patched.slice(0, 3),
      group: {
        id: group!.id,
        name: group!.body.name || `Group ${group!.id}`,
        fixtures: [...group!.body.fixtures],
      },
    };
  },
  api: async ({ api }, state) => {
    await api.command("selection.set", { fixtures: state.fixtures.slice(0, 2).map((fixture) => fixture.id) });
    await api.request("POST", "/api/v1/highlight/action", { action: "on" });
    state.fixtureCaptureObserved = (await api.request<any>("GET", "/api/v1/highlight", undefined, true)).remembered.length === 2;
    await api.request("POST", "/api/v1/highlight/action", { action: "off" });

    await api.command("selection.set", { fixtures: [state.fixtures[2].id] });
    await api.request("POST", "/api/v1/highlight/action", { action: "capture" });
    await api.request("POST", "/api/v1/highlight/action", { action: "on" });
    state.stageCaptureObserved = (await api.request<any>("GET", "/api/v1/highlight", undefined, true)).remembered[0]?.fixture_id === state.fixtures[2].id;

    await api.command("selection.set", { fixtures: state.group.fixtures });
    await api.request("POST", "/api/v1/highlight/action", { action: "capture" });
    state.groupCaptureObserved = JSON.stringify((await api.request<any>("GET", "/api/v1/highlight", undefined, true)).remembered.map((fixture: any) => fixture.fixture_id)) === JSON.stringify(state.group.fixtures);
    await api.command("programmer.set", { fixture_id: state.group.fixtures[0], attribute: "position.pan", value: 0.61 });
    state.storedPresetId = "199";
    await storeCurrentProgrammerPreset(api, state.showId, state.storedPresetId);

    const deskId = api.session!.desk.id;
    await api.login("Operator", deskId);
    state.reconnectRetained = (await api.request<any>("GET", "/api/v1/highlight", undefined, true)).active;
    await api.request("POST", `/api/v1/shows/${state.showId}/open`, { transition: "hold_current" });
  },
  ui: async ({ api, bench, desk, page }, state) => {
    await desk.open(bench.baseUrl);
    await openBuiltIn(page, "Fixtures");
    for (const fixture of state.fixtures.slice(0, 2)) await fixtureSheetRow(page, fixture.number).click();
    await page.getByRole("button", { name: "Turn Highlight on" }).click();
    await expect.poll(async () => (await api.request<any>("GET", "/api/v1/highlight", undefined, true)).remembered.map((fixture: any) => fixture.fixture_id)).toEqual(state.fixtures.slice(0, 2).map((fixture) => fixture.id));
    state.fixtureCaptureObserved = true;
    await page.getByRole("button", { name: "Turn Highlight off" }).click();
    await expect.poll(async () => (await api.request<any>("GET", "/api/v1/highlight", undefined, true)).active).toBe(false);

    await page.getByRole("button", { name: "CLR", exact: true }).click();
    await openBuiltIn(page, "Stage");
    await page.locator(`.stage-fixture[data-fixture-id="${state.fixtures[2].id}"]`).click();
    await page.getByRole("button", { name: "Capture current selection for Highlight" }).click();
    await page.getByRole("button", { name: "Turn Highlight on" }).click();
    await expect.poll(async () => (await api.request<any>("GET", "/api/v1/highlight", undefined, true)).remembered.map((fixture: any) => fixture.fixture_id)).toEqual([state.fixtures[2].id]);
    state.stageCaptureObserved = true;

    await page.getByRole("button", { name: "CLR", exact: true }).click();
    await openGroups(page);
    await page.locator(".group-pool-window .group-card").filter({ hasText: state.group.name }).first().click();
    await page.getByRole("button", { name: "Capture current selection for Highlight" }).click();
    await expect.poll(async () => (await api.request<any>("GET", "/api/v1/highlight", undefined, true)).remembered.map((fixture: any) => fixture.fixture_id)).toEqual(state.group.fixtures);
    state.groupCaptureObserved = true;
    await api.command("programmer.set", { fixture_id: state.group.fixtures[0], attribute: "position.pan", value: 0.61 });

    await openBuiltIn(page, "Presets");
    await page.locator(".global-store-button").click();
    const emptyPreset = page.locator(".preset-pool-window .preset-card.empty").first();
    state.storedPresetId = (await emptyPreset.locator(".number").textContent())!.trim();
    await emptyPreset.click();
    await expect.poll(async () => (await objects<any>(api, "preset")).some((preset) => preset.id === state.storedPresetId)).toBe(true);

    await page.reload();
    await expect(page.locator(".connection-cover")).toBeHidden({ timeout: 10_000 });
    state.reconnectRetained = (await api.request<any>("GET", "/api/v1/highlight", undefined, true)).active;
    await api.request("POST", `/api/v1/shows/${state.showId}/open`, { transition: "hold_current" });
  },
  assert: async ({ api }, state) => {
    expect(state.fixtureCaptureObserved).toBe(true);
    expect(state.stageCaptureObserved).toBe(true);
    expect(state.groupCaptureObserved).toBe(true);
    expect(state.reconnectRetained).toBe(true);
    const preset = await object<any>(api, "preset", state.storedPresetId!);
    const storedAttributes = Object.values(preset.body.values ?? {}).flatMap((attributes: any) => Object.keys(attributes));
    expect(storedAttributes).toContain("position.pan");
    expect(storedAttributes.every((attribute) => !attribute.toLowerCase().includes("highlight"))).toBe(true);
    const highlight = await api.request<any>("GET", "/api/v1/highlight", undefined, true);
    expect(highlight).toMatchObject({ active: false, output_enabled: false, remembered: [] });
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
    await page.locator(".setup-window nav").getByRole("button", { name: "Fixture library", exact: true }).click();
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
    await page.locator(".setup-window nav").getByRole("button", { name: "Screens & playback", exact: true }).click();
    const settings = page.locator('article[aria-label="Matter playback bridge"]');
    const toggle = settings.getByRole("checkbox", { name: "Enable this desk as a Matter bridge" });
    await expect(settings.getByText("Desk installation · shared across shows and Desktops")).toBeVisible();
    await expect(toggle).not.toBeChecked();
    await toggle.click();
    await expect.poll(async () => (await api.request<any>("GET", "/api/v1/configuration", undefined, false)).configuration.matter_enabled).toBe(true);
    state.observed = await api.request<any>("GET", "/api/v1/matter/status");
    await toggle.click();
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
