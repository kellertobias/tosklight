import type { Page } from "../apps/control-ui/node_modules/@playwright/test/index.js";
import fs from "node:fs/promises";
import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import type { ApiDriver } from "../apps/control-ui/e2e/bench/api";
import { pairedScenario } from "../apps/control-ui/e2e/bench/pairedScenario";

interface VersionedObject<T = Record<string, unknown>> {
  kind: string;
  id: string;
  revision: number;
  body: T;
}

interface ShowEntry { id: string; name: string }

interface Show000State {
  canonical: { compact: ShowEntry; defaultStage: ShowEntry };
  compactCopyName: string;
  defaultCopyName: string;
  compactRevisionName: string;
  defaultRevisionName: string;
  compactCopy?: ShowEntry;
  defaultCopy?: ShowEntry;
  compactFixture: string;
  defaultFixture: string;
  compactInitiallyMatched: boolean;
  defaultInitiallyMatched: boolean;
}

const OBJECT_KINDS = [
  "patch_layer", "patched_fixture", "group", "route", "stage_layout",
  "cue_list", "playback", "playback_page", "user_layout",
] as const;

test.describe("docs/testing/00-generate-show-files.md", () => {
  pairedScenario<Show000State>({
    id: "SHOW-000",
    title: "Save As produces independent reusable show files",
    arrange: async ({ api }, surface) => {
      const suffix = `${surface}-${crypto.randomUUID()}`;
      const canonical = await createCanonicalShows(api);
      const compactFixture = (await objects(api, canonical.compact.id, "patched_fixture")).find((fixture) => fixture.body.fixture_number === 1);
      const defaultFixture = (await objects(api, canonical.defaultStage.id, "patched_fixture")).find((fixture) => fixture.body.fixture_number === 1);
      expect(compactFixture).toBeDefined();
      expect(defaultFixture).toBeDefined();
      return {
        canonical,
        compactCopyName: `show-000-compact-copy-${suffix}`,
        defaultCopyName: `show-000-default-copy-${suffix}`,
        compactRevisionName: `SHOW-000 compact ${surface} mutation`,
        defaultRevisionName: `SHOW-000 default ${surface} mutation`,
        compactFixture: String(compactFixture!.body.fixture_id),
        defaultFixture: String(defaultFixture!.body.fixture_id),
        compactInitiallyMatched: false,
        defaultInitiallyMatched: false,
      };
    },
    api: async ({ api }, state) => {
      state.compactCopy = await copyShow(api, state.canonical.compact, state.compactCopyName);
      state.compactInitiallyMatched = JSON.stringify(await showSnapshot(api, state.compactCopy.id)) === JSON.stringify(await showSnapshot(api, state.canonical.compact.id));
      await updateGroup(api, state.compactCopy.id, "4", {
        name: "Copy Center Spot",
        color: "#1bd6ec",
        icon: "★",
        fixtures: [state.compactFixture],
      });
      const revision = await saveNamedRevision(api, state.compactCopy.id, state.compactRevisionName);
      await updateGroup(api, state.compactCopy.id, "4", { name: "Temporary mutation" });
      await openNamedRevision(api, state.compactCopy.id, revision.revision);

      state.defaultCopy = await copyShow(api, state.canonical.defaultStage, state.defaultCopyName);
      state.defaultInitiallyMatched = JSON.stringify(await showSnapshot(api, state.defaultCopy.id)) === JSON.stringify(await showSnapshot(api, state.canonical.defaultStage.id));
      await createGroup(api, state.defaultCopy.id, "900", "Copy Marker", [state.defaultFixture]);
      await saveNamedRevision(api, state.defaultCopy.id, state.defaultRevisionName);
      await openShow(api, state.canonical.defaultStage.id);
    },
    ui: async ({ api, desk, page }, state) => {
      test.setTimeout(90_000);
      await openShow(api, state.canonical.compact.id);
      await desk.open(api.baseUrl);
      await saveAsThroughUi(page, state.compactCopyName);
      state.compactCopy = await showNamed(api, state.compactCopyName);
      state.compactInitiallyMatched = JSON.stringify(await showSnapshot(api, state.compactCopy.id)) === JSON.stringify(await showSnapshot(api, state.canonical.compact.id));

      await selectFixtureOneThroughUi(page);
      await openGroups(page);
      await page.getByRole("button", { name: "REC", exact: true }).click();
      await groupTile(page, "Center Spot").click();
      await expect.poll(async () => groupBody(api, state.compactCopy!.id, "4")).toMatchObject({ fixtures: [expect.any(String)] });
      expect(await groupBody(api, state.compactCopy.id, "4")).toMatchObject({ fixtures: [state.compactFixture] });

      const commandLine = page.getByLabel("Command line");
      await page.getByRole("button", { name: "ESC", exact: true }).click();
      for (const key of ["SET", "GRP", "4"]) await page.getByRole("button", { name: key, exact: true }).click();
      await expect(commandLine).toHaveValue("SET GROUP 4");
      await page.getByRole("button", { name: "ENT", exact: true }).click();
      const properties = page.getByRole("dialog", { name: "Group properties" });
      await properties.getByLabel("Group name").fill("Copy Center Spot");
      await properties.getByRole("button", { name: /#718596/ }).click();
      await page.getByRole("button", { name: "Use color #1bd6ec" }).click();
      await properties.getByRole("button", { name: /Choose icon/ }).click();
      await page.getByRole("button", { name: "Use ★" }).click();
      await properties.getByRole("button", { name: "Save group" }).click();
      await expect(groupTile(page, "Copy Center Spot")).toBeVisible();
      await saveNamedRevisionThroughUi(page, state.compactRevisionName);
      await loadNamedRevisionThroughUi(page, state.compactCopyName, state.compactRevisionName);

      await loadThroughUi(page, state.canonical.defaultStage.name);
      await saveAsThroughUi(page, state.defaultCopyName);
      state.defaultCopy = await showNamed(api, state.defaultCopyName);
      state.defaultInitiallyMatched = JSON.stringify(await showSnapshot(api, state.defaultCopy.id)) === JSON.stringify(await showSnapshot(api, state.canonical.defaultStage.id));
      await selectFixtureOneThroughUi(page);
      await desk.command("REC GROUP 900", "RECORD GROUP 900");
      await expect.poll(async () => (await objects(api, state.defaultCopy!.id, "group")).some((group) => group.id === "900")).toBe(true);
      await saveNamedRevisionThroughUi(page, state.defaultRevisionName);
      await loadThroughUi(page, state.canonical.defaultStage.name);
    },
    assert: async ({ api }, state) => {
      expect(state.compactCopy).toBeDefined();
      expect(state.defaultCopy).toBeDefined();
      expect(state.compactInitiallyMatched).toBe(true);
      expect(state.defaultInitiallyMatched).toBe(true);
      expect(await groupBody(api, state.compactCopy!.id, "4")).toMatchObject({
        name: "Copy Center Spot",
        color: "#1bd6ec",
        icon: "★",
        fixtures: [state.compactFixture],
      });
      expect(await groupBody(api, state.canonical.compact.id, "4")).toMatchObject({ name: "Center Spot", fixtures: [] });
      expect((await groupBody(api, state.canonical.compact.id, "4")).color).toBeNull();
      expect((await groupBody(api, state.canonical.compact.id, "4")).icon).toBeNull();
      await assertCompactRig(api, state.canonical.compact.id);
      await assertDefaultStage(api, state.defaultCopy!.id);
      expect((await objects(api, state.defaultCopy!.id, "group")).find((group) => group.id === "900")?.body.fixtures).toEqual([state.defaultFixture]);
      expect((await objects(api, state.canonical.defaultStage.id, "group")).some((group) => group.id === "900")).toBe(false);
      await expectActiveShow(api, state.canonical.defaultStage.name);
    },
  });

  test("SHOW-000 @supplemental › direct API Save As and named-revision permutations", async ({ api }) => {
    const canonical = await createCanonicalShows(api);
    const compactCopyName = `show-000-compact-copy-${crypto.randomUUID()}`;
    const defaultCopyName = `show-000-default-copy-${crypto.randomUUID()}`;

    const compactCopy = await copyShow(api, canonical.compact, compactCopyName);
    await expectActiveShow(api, compactCopyName);
    await assertCompactRig(api, compactCopy.id);
    await expect(showSnapshot(api, compactCopy.id)).resolves.toEqual(await showSnapshot(api, canonical.compact.id));
    const compactFixture = String((await objects(api, compactCopy.id, "patched_fixture"))[0].body.fixture_id);
    await updateGroup(api, compactCopy.id, "4", {
      name: "Copy Center Spot",
      color: "#1bd6ec",
      icon: "★",
      fixtures: [compactFixture],
    });
    const compactRevision = await saveNamedRevision(api, compactCopy.id, "SHOW-000 compact API mutation");
    await updateGroup(api, compactCopy.id, "4", { name: "Temporary mutation" });
    await openNamedRevision(api, compactCopy.id, compactRevision.revision);
    expect(await groupBody(api, compactCopy.id, "4")).toMatchObject({ name: "Copy Center Spot", color: "#1bd6ec", icon: "★", fixtures: [compactFixture] });
    await openShow(api, canonical.compact.id);
    expect(await groupBody(api, canonical.compact.id, "4")).toMatchObject({ name: "Center Spot", fixtures: [] });

    const defaultCopy = await copyShow(api, canonical.defaultStage, defaultCopyName);
    await expectActiveShow(api, defaultCopyName);
    await assertDefaultStage(api, defaultCopy.id);
    await expect(showSnapshot(api, defaultCopy.id)).resolves.toEqual(await showSnapshot(api, canonical.defaultStage.id));
    const defaultFixture = String((await objects(api, defaultCopy.id, "patched_fixture"))[0].body.fixture_id);
    await createGroup(api, defaultCopy.id, "900", "Copy Marker", [defaultFixture]);
    await saveNamedRevision(api, defaultCopy.id, "SHOW-000 default API mutation");
    await openShow(api, canonical.defaultStage.id);
    expect((await objects(api, canonical.defaultStage.id, "group")).some((group) => group.id === "900")).toBe(false);
  });

  test("SHOW-000 @supplemental › extended visible Group-properties and alternate-gesture workflow", async ({ api, desk, page }) => {
    test.setTimeout(90_000);
    const canonical = await createCanonicalShows(api);
    const compactCopyName = `show-000-compact-copy-${crypto.randomUUID()}`;
    const defaultCopyName = `show-000-default-copy-${crypto.randomUUID()}`;
    const compactRevisionName = "SHOW-000 compact mutation";
    const defaultRevisionName = "SHOW-000 default mutation";

    await desk.recordStep("ARRANGE", "Open the maintained Compact Rig fixture and make a working copy before changing anything.");
    await openShow(api, canonical.compact.id);
    await desk.open(api.baseUrl);
    await saveAsThroughUi(page, compactCopyName);
    const compactCopy = await showNamed(api, compactCopyName);
    await assertCompactRig(api, compactCopy.id);
    await expect(showSnapshot(api, compactCopy.id)).resolves.toEqual(await showSnapshot(api, canonical.compact.id));

    await desk.recordStep("OPERATOR", "Select fixture 1, record it into stored empty Group 4, then edit that Group through the exact SET GRP 4 ENTER desk shortcut.");
    await selectFixtureOneThroughUi(page);
    await openGroups(page);
    await page.getByRole("button", { name: "REC", exact: true }).click();
    await groupTile(page, "Center Spot").click();
    await expect.poll(async () => groupBody(api, compactCopy.id, "4")).toMatchObject({ fixtures: [expect.any(String)] });
    const commandLine = page.getByLabel("Command line");
    await page.getByRole("button", { name: "ESC", exact: true }).click();
    await page.getByRole("button", { name: "SET", exact: true }).click();
    await expect(commandLine).toHaveValue("SET");
    await page.getByRole("button", { name: "GRP", exact: true }).click();
    await expect(commandLine).toHaveValue(/^SET GROUP\s*$/);
    await page.getByRole("button", { name: "4", exact: true }).click();
    await expect(commandLine).toHaveValue("SET GROUP 4");
    await page.getByRole("button", { name: "ENT", exact: true }).click();
    const properties = page.getByRole("dialog", { name: "Group properties" });
    await expect(properties).toBeVisible();
    await properties.getByLabel("Group name").fill("Copy Center Spot");
    await properties.getByRole("button", { name: /#718596/ }).click();
    await page.getByRole("button", { name: "Use color #1bd6ec" }).click();
    await properties.getByRole("button", { name: /Choose icon/ }).click();
    await page.getByRole("button", { name: "Use ★" }).click();
    await properties.getByRole("button", { name: "Save group" }).click();
    await expect(properties).toBeHidden();
    const editedTile = groupTile(page, "Copy Center Spot");
    await expect(editedTile).toBeVisible();
    await expect(editedTile.getByLabel("Color #1bd6ec")).toBeVisible();
    await expect(editedTile.getByLabel("Icon ★")).toBeVisible();

    await desk.recordStep("VERIFY ALTERNATE GESTURE", "Press SET and tap Group 4; the same modal must reopen with the saved name, color, and icon.");
    await page.getByRole("button", { name: "SET", exact: true }).click();
    await editedTile.click();
    await expect(properties).toBeVisible();
    await expect(properties.getByLabel("Group name")).toHaveValue("Copy Center Spot");
    await expect(properties.getByRole("button", { name: /#1BD6EC/ })).toBeVisible();
    await expect(properties.getByRole("button", { name: /Choose icon/ })).toContainText("★");
    await properties.getByRole("button", { name: "Cancel" }).click();

    await desk.recordStep("PERSIST", "Save a named revision, restore that revision, and verify the Group mutation is still present only in the copy.");
    await saveNamedRevisionThroughUi(page, compactRevisionName);
    await loadNamedRevisionThroughUi(page, compactCopyName, compactRevisionName);
    await openGroups(page);
    await expect(groupTile(page, "Copy Center Spot")).toBeVisible();
    await expect.poll(async () => groupBody(api, compactCopy.id, "4")).toMatchObject({
      name: "Copy Center Spot",
      color: "#1bd6ec",
      icon: "★",
      fixtures: [expect.any(String)],
    });

    await loadThroughUi(page, canonical.compact.name);
    await openGroups(page);
    await expect(groupTile(page, "Center Spot")).toBeVisible();
    expect(await groupBody(api, canonical.compact.id, "4")).toMatchObject({ name: "Center Spot", fixtures: [] });
    expect((await groupBody(api, canonical.compact.id, "4")).color).toBeNull();
    expect((await groupBody(api, canonical.compact.id, "4")).icon).toBeNull();

    await desk.recordStep("SECOND CANONICAL", "Copy the complete Default Stage Show, create hidden Group 900 with the desk keys, and prove the canonical remains unchanged.");
    await loadThroughUi(page, canonical.defaultStage.name);
    await saveAsThroughUi(page, defaultCopyName);
    const defaultCopy = await showNamed(api, defaultCopyName);
    await assertDefaultStage(api, defaultCopy.id);
    await expect(showSnapshot(api, defaultCopy.id)).resolves.toEqual(await showSnapshot(api, canonical.defaultStage.id));
    await selectFixtureOneThroughUi(page);
    await desk.command("REC GROUP 900", "RECORD GROUP 900");
    await expect.poll(async () => (await objects(api, defaultCopy.id, "group")).some((group) => group.id === "900")).toBe(true);
    await saveNamedRevisionThroughUi(page, defaultRevisionName);
    await loadThroughUi(page, canonical.defaultStage.name);
    expect((await objects(api, canonical.defaultStage.id, "group")).some((group) => group.id === "900")).toBe(false);
    await desk.recordStep("PASSED", "Both maintained show fixtures are unchanged; all deliberate mutations and revisions belong only to their Save As copies.");
  });
});

async function createCanonicalShows(api: ApiDriver) {
  const suffix = crypto.randomUUID();
  const [compactBytes, defaultStageBytes] = await Promise.all([
    fs.readFile(new URL("./fixtures/compact-rig.show", import.meta.url)),
    fs.readFile(new URL("./fixtures/default-stage.show", import.meta.url)),
  ]);
  const compact = await api.request<ShowEntry>("POST", "/api/v1/shows", {
    name: `compact-rig-${suffix}`, data_base64: compactBytes.toString("base64"), overwrite: false,
  });
  const defaultStage = await api.request<ShowEntry>("POST", "/api/v1/shows", {
    name: `Default Stage Show ${suffix}`, data_base64: defaultStageBytes.toString("base64"), overwrite: false,
  });
  return { compact, defaultStage };
}

async function copyShow(api: ApiDriver, source: ShowEntry, name: string): Promise<ShowEntry> {
  const data_base64 = (await downloadShow(api, source.id)).toString("base64");
  const copy = await api.request<ShowEntry>("POST", "/api/v1/shows", { name, data_base64, overwrite: false });
  await openShow(api, copy.id);
  return copy;
}

async function downloadShow(api: ApiDriver, id: string): Promise<Buffer> {
  const response = await fetch(`${api.baseUrl}/api/v1/shows/${id}/download`, {
    headers: { authorization: `Bearer ${api.session?.token}` },
  });
  expect(response.ok).toBe(true);
  return Buffer.from(await response.arrayBuffer());
}

async function saveAsThroughUi(page: Page, name: string): Promise<void> {
  await openShowMenu(page);
  await page.getByRole("button", { name: "Save As", exact: true }).click();
  await page.getByRole("textbox", { name: "Show name" }).fill(name);
  await page.getByRole("button", { name: "Save show", exact: true }).click();
  await expect(page.locator(".dock-identity b")).toContainText(name);
  await closeShowMenu(page);
}

async function loadThroughUi(page: Page, name: string): Promise<void> {
  await openShowMenu(page);
  await page.getByRole("button", { name: "Load", exact: true }).click();
  const entry = page.locator(".show-library article").filter({ has: page.getByText(name, { exact: true }) });
  await entry.getByRole("button", { name: "Load Latest Autosave" }).click();
  await expect(page.locator(".dock-identity b")).toContainText(name);
  await closeShowMenu(page);
}

async function openShowMenu(page: Page): Promise<void> {
  if (!await page.locator(".show-modal").isVisible()) {
    await page.getByRole("button", { name: /Open show menu/ }).click();
  }
}

async function closeShowMenu(page: Page): Promise<void> {
  if (await page.locator(".show-modal").isVisible()) {
    await page.getByRole("button", { name: "Close Show", exact: true }).click();
    await expect(page.locator(".show-modal")).toBeHidden();
  }
}

async function saveNamedRevisionThroughUi(page: Page, name: string): Promise<void> {
  await openShowMenu(page);
  await page.getByRole("button", { name: "Save Named Revision", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Save named revision" });
  await dialog.getByLabel("Revision name").fill(name);
  await dialog.getByRole("button", { name: /^Save Revision/ }).click();
  await expect(dialog).toBeHidden();
  await expect(page.locator(".show-details")).toContainText(name);
  await closeShowMenu(page);
}

async function loadNamedRevisionThroughUi(page: Page, showName: string, revisionName: string): Promise<void> {
  await openShowMenu(page);
  await page.getByRole("button", { name: "Load", exact: true }).click();
  const entry = page.locator(".revision-show-library article").filter({ has: page.getByText(showName, { exact: true }) });
  const revision = entry.locator(".named-revision-list button").filter({ hasText: revisionName });
  await expect(revision).toBeVisible();
  await revision.click();
  await expect(page.locator(".dock-identity b")).toContainText(showName);
  await closeShowMenu(page);
}

async function openBuiltIn(page: Page, name: string): Promise<void> {
  const entry = page.locator(".dock-entry").filter({ hasText: name });
  if (!await entry.isVisible()) await page.getByRole("button", { name: "BUILT-INS", exact: true }).click();
  await expect(entry).toBeVisible();
  await entry.click();
}

async function openGroups(page: Page): Promise<void> {
  if (!await page.locator(".group-pool-window").isVisible()) {
    await page.getByRole("button", { name: "SHIFT", exact: true }).click();
    await page.getByRole("button", { name: "1", exact: true }).click();
  }
  await expect(page.locator(".group-pool-window")).toBeVisible();
}

async function selectFixtureOneThroughUi(page: Page): Promise<void> {
  await openBuiltIn(page, "Fixtures");
  const row = page.locator(".ui-data-table").getByRole("row").nth(1);
  await expect(row).toBeVisible();
  await row.click();
  await expect(row).toHaveClass(/selected/);
}

function groupTile(page: Page, name: string) {
  return page.locator(".group-card").filter({ has: page.getByText(name, { exact: true }) }).first();
}

async function assertCompactRig(api: ApiDriver, showId: string): Promise<void> {
  const fixtures = await objects(api, showId, "patched_fixture");
  expect(fixtures).toHaveLength(16);
  const layers = (await objects(api, showId, "patch_layer")).map((layer) => layer.body.name);
  expect(layers).toEqual(expect.arrayContaining(["Dimmers", "LEDs"]));
  const byNumber = new Map(fixtures.map((fixture) => [fixture.body.fixture_number, fixture.body]));
  for (let number = 1; number <= 12; number += 1) {
    expect(byNumber.get(number)).toMatchObject({ universe: 1, address: number, layer_id: "dimmers" });
  }
  for (const [index, number] of [21, 22, 23, 24].entries()) {
    const fixture = byNumber.get(number);
    expect(fixture).toMatchObject({ name: `RGB LED ${index + 1}`, universe: 1, address: 13 + index * 3, layer_id: "leds" });
    const parameters = fixture.definition.heads[0].parameters;
    expect(parameters.find((parameter: Record<string, unknown>) => parameter.attribute === "intensity")).toMatchObject({ virtual_dimmer: true, components: [] });
  }
  const groups = await objects(api, showId, "group");
  expect(groups.map((item) => [item.id, item.body.name, item.body.fixtures.length])).toEqual([
    ["1", "All Dimmers", 12], ["2", "Odd Dimmers", 6], ["3", "Front Dimmers", 4], ["4", "Center Spot", 0],
  ]);
  await assertRoutes(api, showId);
}

async function assertDefaultStage(api: ApiDriver, showId: string): Promise<void> {
  const fixtures = await objects(api, showId, "patched_fixture");
  expect(fixtures).toHaveLength(49);
  const byNumber = new Map(fixtures.map((fixture) => [fixture.body.fixture_number, fixture.body]));
  for (const [number, universe, address] of [
    [1, 1, 1], [6, 1, 6], [28, 1, 11], [29, 1, 12], [99, 1, 13],
    [101, 2, 1], [201, 2, 49], [301, 2, 79],
    [401, 3, 1], [501, 3, 61], [601, 3, 241], [999, 4, 1],
  ]) {
    expect(byNumber.get(number)).toMatchObject({ universe, address });
  }
  expect(new Set(fixtures.map((fixture) => fixture.body.universe))).toEqual(new Set([1, 2, 3, 4]));
  expect(fixtures.filter((fixture) => String(fixture.body.name).startsWith("Back RGB Sunstrip "))).toHaveLength(6);
  const stage = await objects(api, showId, "stage_layout");
  expect(stage).toHaveLength(1);
  expect(Object.keys(stage[0].body.positions3d as object).length).toBeGreaterThan(49);
  await assertRoutes(api, showId);
}

async function assertRoutes(api: ApiDriver, showId: string): Promise<void> {
  const routes = await objects(api, showId, "route");
  expect(routes.map((route) => [route.body.protocol, route.body.logical_universe, route.body.destination_universe, route.body.enabled])).toEqual([
    ["art_net", 1, 1, true], ["sacn", 1, 101, true],
  ]);
}

async function showSnapshot(api: ApiDriver, showId: string) {
  const entries = await Promise.all(OBJECT_KINDS.map(async (kind) => [kind, await objects(api, showId, kind)] as const));
  return Object.fromEntries(entries.map(([kind, values]) => [kind, values.map(({ id, body }) => ({
    id,
    body: kind === "patched_fixture"
      ? {
          move_in_black_enabled: true,
          move_in_black_delay_millis: 0,
          ...body,
        }
      : body,
  }))]));
}

async function objects(api: ApiDriver, showId: string, kind: string): Promise<VersionedObject[]> {
  const result = await api.request<VersionedObject[]>("GET", `/api/v1/shows/${showId}/objects/${kind}`, undefined, false);
  return result.sort((left, right) => left.id.localeCompare(right.id, undefined, { numeric: true }));
}

async function groupBody(api: ApiDriver, showId: string, id: string): Promise<Record<string, unknown>> {
  const group = (await objects(api, showId, "group")).find((item) => item.id === id);
  expect(group, `Group ${id} should exist in show ${showId}`).toBeDefined();
  return group!.body;
}

async function put(api: ApiDriver, showId: string, kind: string, id: string, body: unknown, revision = 0): Promise<void> {
  await api.request("PUT", `/api/v1/shows/${showId}/objects/${kind}/${id}`, body, true, revision);
}

async function updateGroup(api: ApiDriver, showId: string, id: string, update: Record<string, unknown>): Promise<void> {
  const stored = (await objects(api, showId, "group")).find((group) => group.id === id);
  expect(stored).toBeDefined();
  await put(api, showId, "group", id, { ...stored!.body, ...update }, stored!.revision);
}

async function createGroup(api: ApiDriver, showId: string, id: string, name: string, fixtures: string[]): Promise<void> {
  await put(api, showId, "group", id, group(name, fixtures, null));
}

async function saveNamedRevision(api: ApiDriver, showId: string, name: string): Promise<{ revision: number }> {
  return api.request("POST", `/api/v1/shows/${showId}/revisions`, { name });
}

async function openNamedRevision(api: ApiDriver, showId: string, revision: number): Promise<void> {
  await api.request("POST", `/api/v1/shows/${showId}/revisions/${revision}/open`, { transition: "hold_current" });
}

async function openShow(api: ApiDriver, id: string): Promise<void> {
  await api.request("POST", `/api/v1/shows/${id}/open`, { transition: "hold_current" });
}

async function showNamed(api: ApiDriver, name: string): Promise<ShowEntry> {
  const show = (await api.request<ShowEntry[]>("GET", "/api/v1/shows", undefined, false)).find((entry) => entry.name === name);
  expect(show).toBeDefined();
  return show!;
}

async function expectActiveShow(api: ApiDriver, name: string): Promise<void> {
  const bootstrap = await api.request<{ active_show: ShowEntry | null }>("GET", "/api/v1/bootstrap", undefined, false);
  expect(bootstrap.active_show?.name).toBe(name);
}

function group(name: string, fixtures: string[], playback_fader: number | null) {
  return { name, fixtures, derived_from: null, frozen_from: null, programming: {}, master: 1, playback_fader };
}
