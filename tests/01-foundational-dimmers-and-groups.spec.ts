import type { Page } from "../apps/control-ui/node_modules/@playwright/test/index.js";
import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import type { ApiDriver } from "../apps/control-ui/e2e/bench/api";

interface VersionedObject<T = Record<string, any>> {
  kind: string;
  id: string;
  revision: number;
  body: T;
}

interface ShowEntry { id: string; name: string }

interface ProgrammerState {
  selected: string[];
  selection_expression: any;
  values: Array<{ fixture_id: string; attribute: string; value: { value?: number } | number }>;
  group_values: Record<string, Record<string, { value: { value?: number } | number }>>;
  command_line: string;
}

const INTENSITY = "intensity";

test.describe("docs/testing/01-foundational-dimmers-and-groups.md", () => {
  test("DIM-001 @api › ordered live group edits keep programmer references and output order", async ({ api, bench }) => {
    await loadCompactRig(api, bench, "dim-001-api");
    const fixtures = await fixtureIdsByNumber(api);

    await command(api, "GROUP 3 AT 50");
    await expectProgrammer(api, (programmer) => {
      expect(programmer.group_values["3"]?.[INTENSITY]).toBeDefined();
      expect(programmer.values).toHaveLength(0);
      expect(programmer.selection_expression).toMatchObject({ type: "live_group", group_id: "3" });
    });
    await expectSlotsAfterTick(bench, 3_000, [128, 128, 128, 128, 0, 0]);

    await select(api, [fixtures[5], fixtures[6]]);
    await command(api, "RECORD + GROUP 3");
    await expectGroupNumbers(api, "3", [1, 2, 3, 4, 5, 6]);

    await command(api, "GROUP 3 - 2");
    await command(api, "RECORD GROUP 3");
    await expectGroupNumbers(api, "3", [1, 3, 4, 5, 6]);
    await expectSlotsAfterTick(bench, 0, [128, 0, 128, 128, 128, 128]);

    await command(api, "GROUP 3 + 2");
    await command(api, "RECORD GROUP 3");
    await expectGroupNumbers(api, "3", [1, 3, 4, 5, 6, 2]);
    await expectSlotsAfterTick(bench, 0, [128, 128, 128, 128, 128, 128]);

    await select(api, []);
    await command(api, "RECORD - GROUP 3");
    await expectGroupMissing(api, "3");
  });

  test("DIM-002 @api › command reaches exact fade boundary and both UDP protocols", async ({ api, bench }) => {
    await loadCompactRig(api, bench, "dim-002-api");
    await command(api, "GROUP 1 AT 50");
    await expectProgrammer(api, (programmer) => expect(normalized(programmer.group_values["1"][INTENSITY].value)).toBe(0.5));
    await expectSlotsAfterTick(bench, 2_999, Array(12).fill(127));
    await expectSlotsAfterTick(bench, 1, Array(12).fill(128));
    await expectSlotsAfterTick(bench, 1, Array(12).fill(128));
  });

  test("DIM-002 @ui › visible Lightning Desk keypad reaches exact fade boundary", async ({ api, bench, desk, page }) => {
    await loadCompactRig(api, bench, "dim-002-ui");
    await desk.open(api.baseUrl);
    await pressCommand(page, "GROUP 1 AT 50", "G1 AT 50");
    await expectProgrammer(api, (programmer) => expect(normalized(programmer.group_values["1"][INTENSITY].value)).toBe(0.5));
    await expectSlotsAfterTick(bench, 2_999, Array(12).fill(127));
    await expectSlotsAfterTick(bench, 1, Array(12).fill(128));
    await expectSlotsAfterTick(bench, 1, Array(12).fill(128));
  });

  test("CMD-001 @ui › persistent fixture and group prefixes survive Enter, Clear, and Escape", async ({ api, bench, desk, page }) => {
    await loadCompactRig(api, bench, "cmd-001-ui");
    await desk.open(api.baseUrl);
    const commandLine = page.getByLabel("Command line");
    const press = async (key: string) => page.getByRole("button", { name: key, exact: true }).click();
    const controlSection = await page.locator(".control-section").boundingBox();
    const programmerRight = await page.locator(".control-right-pane").boundingBox();
    expect(programmerRight?.width).toBeCloseTo(384, 0);
    expect((controlSection!.x + controlSection!.width) - (programmerRight!.x + programmerRight!.width)).toBeLessThanOrEqual(6);

    await page.getByRole("button", { name: /Prog\. Fade/ }).click();
    const fadeDialog = page.getByRole("dialog", { name: "Prog. Fade value" });
    await expect(fadeDialog.getByRole("slider", { name: "Prog. Fade" })).toBeVisible();
    await expect(fadeDialog.getByLabel("Number input keypad")).toBeVisible();
    await fadeDialog.getByRole("button", { name: "Close attribute value" }).click();
    await expect(page.getByRole("slider", { name: "Prog. Fade" })).toHaveCount(0);

    await page.locator(".mode-toggle").click();
    await expect(page.getByRole("slider", { name: "Prog. Fade" })).toBeVisible();
    await expect(page.getByRole("slider", { name: "Cue Fade" })).toBeVisible();
    const playbackRight = await page.locator(".control-right-pane").boundingBox();
    expect(playbackRight?.width).toBeCloseTo(384, 0);
    expect(playbackRight?.x).toBeCloseTo(programmerRight!.x, 0);
    await page.locator(".mode-toggle").click();
    await expect(page.getByRole("slider", { name: "Prog. Fade" })).toHaveCount(0);

    await expect(commandLine).toHaveValue("FIXTURE");
    await press("GRP");
    await expect(commandLine).toHaveValue("GROUP");
    await press("ENT");
    await expect(commandLine).toHaveValue("GROUP");
    await press("CLR");
    await expect(commandLine).toHaveValue("GROUP");
    await page.getByRole("button", { name: "ESC", exact: true }).click();
    await expect(commandLine).toHaveValue("GROUP");

    for (const key of ["7", "+", "8"]) await press(key);
    await expect(commandLine).toHaveValue("G7 + G8");
    await page.getByRole("button", { name: "ESC", exact: true }).click();
    for (const key of ["7", "+", "GRP", "8"]) await press(key);
    await expect(commandLine).toHaveValue("G7 + F8");
    await page.getByRole("button", { name: "ESC", exact: true }).click();

    await press("GRP");
    await press("ENT");
    await expect(commandLine).toHaveValue("FIXTURE");
    for (const key of ["7", "+", "8"]) await press(key);
    await expect(commandLine).toHaveValue("F7 + F8");
    await page.getByRole("button", { name: "ESC", exact: true }).click();
    for (const key of ["GRP", "7", "+", "8"]) await press(key);
    await expect(commandLine).toHaveValue("G7 + F8");
    await page.getByRole("button", { name: "ESC", exact: true }).click();
    for (const key of ["GRP", "7", "+", "GRP", "8"]) await press(key);
    await expect(commandLine).toHaveValue("G7 + G8");
    await page.getByRole("button", { name: "ESC", exact: true }).click();
    for (const key of ["7", "+", "GRP", "8"]) await press(key);
    await expect(commandLine).toHaveValue("F7 + G8");

    await page.getByRole("button", { name: "ESC", exact: true }).click();
    for (const key of ["GRP", "3", "+", "5"]) await press(key);
    await expect(commandLine).toHaveValue("G3 + F5");
    await press("ENT");
    await expect(commandLine).toHaveValue("FIXTURE");
    await expectSelectedNumbers(api, [1, 2, 3, 4, 5]);

    for (const key of ["5", "+", "GRP", "3"]) await press(key);
    await expect(commandLine).toHaveValue("F5 + G3");
    await press("ENT");
    await expectSelectedNumbers(api, [5, 1, 2, 3, 4]);
  });

  test("GROUP-003 @api › derived group follows source order edits", async ({ api, bench }) => {
    await loadCompactRig(api, bench, "group-003-api");
    await command(api, "GROUP 1 DIV 2");
    await command(api, "RECORD GROUP 5");
    await expectGroup(api, "5", (group) => {
      expect(group.body.derived_from).toMatchObject({ source_group_id: "1", rule: { type: "every_nth", n: 2, offset: 0 } });
    });
    await command(api, "GROUP 5");
    await expectSelectedNumbers(api, [1, 3, 5, 7, 9, 11]);

    await overwriteGroupByNumbers(api, "1", [12, 1, 2, 4, 5, 6, 7, 8, 9, 10, 11]);
    await command(api, "GROUP 5");
    await expectSelectedNumbers(api, [12, 2, 5, 7, 9, 11]);

    await overwriteGroupByNumbers(api, "1", [12, 1, 2, 8, 4, 5, 6, 7, 9, 10, 11]);
    await command(api, "GROUP 5");
    await expectSelectedNumbers(api, [12, 2, 4, 6, 9, 11]);
    await expectGroupNumbers(api, "4", []);
  });

  test("GROUP-004 @api › frozen group survives source edits and unpatched fixtures stay programmable", async ({ api, bench }) => {
    await loadCompactRig(api, bench, "group-004-api");
    const fixtures = await fixtureIdsByNumber(api);

    await command(api, "GROUP GROUP 1");
    await command(api, "RECORD GROUP 5");
    await expectGroup(api, "5", (group) => {
      expect(group.body.frozen_from).toMatchObject({ source_group_id: "1" });
      expect(group.body.derived_from).toBeNull();
    });
    await expectGroupNumbers(api, "5", [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);

    await overwriteGroupByNumbers(api, "1", [12, 1, 2, 4, 5, 6, 7, 8, 9, 10, 11]);
    await unpatchFixture(api, fixtures[3]);
    await expectGroupNumbers(api, "5", [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);

    await command(api, "GROUP 5 AT 50");
    await expectProgrammer(api, (programmer) => {
      expect(programmer.group_values["5"]?.[INTENSITY]).toBeDefined();
      expect(programmer.selected).toContain(fixtures[3]);
    });
    await expectSlotsAfterTick(bench, 3_000, [128, 128, 0, 128, 128, 128, 128, 128, 128, 128, 128, 128]);
    await command(api, "RECORD 1.1");
    const preset = await object(api, "preset", "1.1");
    expect(preset.body.group_values["5"]?.[INTENSITY]).toBeDefined();
  });

  test("GROUP-005 @api › stored empty groups differ from missing groups", async ({ api, bench }) => {
    await loadCompactRig(api, bench, "group-005-api");
    const fixtures = await fixtureIdsByNumber(api);

    await command(api, "DELETE GROUP 4");
    await expectGroupMissing(api, "4");
    await command(api, "GROUP 1 THRU 5");
    await expectSelectedNumbers(api, [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);

    await select(api, []);
    await command(api, "RECORD GROUP 4");
    await expectGroupNumbers(api, "4", []);
    await command(api, "GROUP 4 AT 50");
    await expectProgrammer(api, (programmer) => expect(programmer.group_values["4"]?.[INTENSITY]).toBeDefined());
    await expectSlotsAfterTick(bench, 3_000, Array(12).fill(0));

    await select(api, [fixtures[1]]);
    await command(api, "RECORD + GROUP 4");
    await expectGroupNumbers(api, "4", [1]);
    await expectSlotsAfterTick(bench, 3_000, [128, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

    await command(api, "DELETE GROUP 4");
    await expect(commandError(api, "GROUP 4")).resolves.toContain("group 4 does not exist");
    await expect(commandError(api, "RECORD + GROUP 4")).resolves.toContain("group 4 does not exist");
  });

  test("PROG-003 @api › programmer intensity arbitration is LTP before playback merge", async ({ api, bench }) => {
    await loadCompactRig(api, bench, "prog-003-api");
    const fixtures = await fixtureIdsByNumber(api);

    await command(api, "GROUP 1 AT 50");
    await command(api, "1 AT 75");
    await expectSlotsAfterTick(bench, 3_000, [191, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128]);

    await loadCompactRig(api, bench, "prog-003-lower-api");
    await command(api, "GROUP 1 AT 50");
    await command(api, "1 AT 25");
    await expectSlotsAfterTick(bench, 3_000, [64, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128]);

    await command(api, "GROUP 1 AT 50");
    await expectSlotsAfterTick(bench, 3_000, [128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128]);
    await expectProgrammer(api, (programmer) => {
      expect(programmer.group_values["1"]?.[INTENSITY]).toBeDefined();
      expect(programmer.values.some((value) => value.fixture_id === fixtures[1])).toBe(true);
    });
  });

  test("PROG-004 @ui › Clear is selection first, then programmer values", async ({ api, bench, desk, page }) => {
    await loadCompactRig(api, bench, "prog-004-ui");
    await desk.open(api.baseUrl);
    const clear = page.getByRole("button", { name: "CLR", exact: true });

    await pressCommand(page, "GROUP 1", "G1");
    await expectProgrammer(api, (programmer) => expect(programmer.selected).toHaveLength(12));
    await clear.click();
    await expectProgrammer(api, (programmer) => {
      expect(programmer.selected).toHaveLength(0);
      expect(programmer.values).toHaveLength(0);
      expect(Object.keys(programmer.group_values)).toHaveLength(0);
    });

    await pressCommand(page, "1 + 2 AT 75", "F1 + F2 AT 75");
    await expectSelectedNumbers(api, [1, 2]);
    await pressCommand(page, "AT 50");
    await expectProgrammer(api, (programmer) => {
      expect(programmer.values.map((value) => normalized(value.value))).toEqual([0.5, 0.5]);
    });
    await expectSlotsAfterTick(bench, 3_000, [128, 128, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

    await clear.click();
    await expectProgrammer(api, (programmer) => {
      expect(programmer.selected).toHaveLength(0);
      expect(programmer.values).toHaveLength(2);
    });
    await expectSlotsAfterTick(bench, 0, [128, 128, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

    await clear.click();
    await expectProgrammer(api, (programmer) => {
      expect(programmer.selected).toHaveLength(0);
      expect(programmer.values).toHaveLength(0);
      expect(Object.keys(programmer.group_values)).toHaveLength(0);
    });
    await expectSlotsAfterTick(bench, 0, Array(12).fill(0));
  });
});

async function loadCompactRig(api: ApiDriver, bench: any, _name: string): Promise<void> {
  await api.request("POST", "/api/v1/test/clock/reset", undefined, false);
  bench.artnet.reset();
  bench.sacn.reset();
  await api.login();
  await api.command("selection.set", { fixtures: [] });
  await api.command("programmer.clear", {});
  const group4 = (await objects(api, "group")).find((group) => group.id === "4");
  await putObject(api, "group", "4", {
    id: "4",
    name: "Center Spot",
    fixtures: [],
    derived_from: null,
    frozen_from: null,
    programming: {},
    master: 1,
    playback_fader: null,
  }, group4?.revision ?? 0);
}

async function command(api: ApiDriver, value: string): Promise<void> {
  await api.command("programmer.execute", { value });
}

async function commandError(api: ApiDriver, value: string): Promise<string> {
  try {
    await command(api, value);
  } catch (error) {
    return error instanceof Error ? error.message : String(error);
  }
  throw new Error(`Expected command to fail: ${value}`);
}

async function pressCommand(page: Page, value: string, visibleValue = value): Promise<void> {
  const commandLine = page.getByLabel("Command line");
  await page.getByRole("button", { name: "ESC", exact: true }).click();
  for (const key of commandKeys(value)) {
    await page.getByRole("button", { name: key, exact: true }).click();
  }
  await expect(commandLine).toHaveValue(visibleValue);
  await page.getByRole("button", { name: "ENT", exact: true }).click();
}

function commandKeys(value: string): string[] {
  return value.trim().split(/\s+/).flatMap((token) => {
    if (token === "GROUP") return ["GRP"];
    if (token === "DEGRP") return ["GRP", "GRP"];
    if (token === "THRU") return ["TRU"];
    if (/^\d+$/.test(token)) return [...token];
    return [token];
  });
}

async function programmer(api: ApiDriver): Promise<ProgrammerState> {
  const programmers = await api.request<ProgrammerState[]>("GET", "/api/v1/programmers", undefined, false);
  const current = programmers.find((item: any) => item.session_id === api.session?.session_id) ?? programmers[0];
  expect(current).toBeDefined();
  return current;
}

async function expectProgrammer(api: ApiDriver, assertion: (programmer: ProgrammerState) => void | Promise<void>): Promise<void> {
  await expect.poll(async () => {
    const programmers = await api.request<ProgrammerState[]>("GET", "/api/v1/programmers", undefined, false);
    let lastError: unknown = null;
    for (const snapshot of programmers) {
      try {
        await assertion(snapshot);
        return true;
      } catch (error) {
        lastError = error;
      }
    }
    if (lastError) throw lastError;
    throw new Error("No programmer matched assertion");
  }, { timeout: 2_000 }).toBe(true);
}

async function select(api: ApiDriver, fixtures: string[]): Promise<void> {
  await api.command("selection.set", { fixtures });
}

async function objects<T = Record<string, any>>(api: ApiDriver, kind: string): Promise<Array<VersionedObject<T>>> {
  const bootstrap = await api.request<{ active_show: ShowEntry | null }>("GET", "/api/v1/bootstrap", undefined, false);
  expect(bootstrap.active_show).toBeTruthy();
  const result = await api.request<Array<VersionedObject<T>>>("GET", `/api/v1/shows/${bootstrap.active_show!.id}/objects/${kind}`, undefined, false);
  return result.sort((left, right) => left.id.localeCompare(right.id, undefined, { numeric: true }));
}

async function object<T = Record<string, any>>(api: ApiDriver, kind: string, id: string): Promise<VersionedObject<T>> {
  const found = (await objects<T>(api, kind)).find((item) => item.id === id);
  expect(found).toBeDefined();
  return found!;
}

async function putObject(api: ApiDriver, kind: string, id: string, body: unknown, revision = 0): Promise<void> {
  const bootstrap = await api.request<{ active_show: ShowEntry | null }>("GET", "/api/v1/bootstrap", undefined, false);
  expect(bootstrap.active_show).toBeTruthy();
  await api.request("PUT", `/api/v1/shows/${bootstrap.active_show!.id}/objects/${kind}/${id}`, body, true, revision);
}

async function fixtureIdsByNumber(api: ApiDriver): Promise<Record<number, string>> {
  const fixtures = await objects(api, "patched_fixture");
  return Object.fromEntries(fixtures.map((fixture) => [fixture.body.fixture_number, fixture.body.fixture_id]));
}

async function fixtureNumberById(api: ApiDriver): Promise<Record<string, number>> {
  const fixtures = await objects(api, "patched_fixture");
  return Object.fromEntries(fixtures.map((fixture) => [fixture.body.fixture_id, fixture.body.fixture_number]));
}

async function expectSelectedNumbers(api: ApiDriver, expected: number[]): Promise<void> {
  const byId = await fixtureNumberById(api);
  await expectProgrammer(api, (snapshot) => {
    expect(snapshot.selected.map((id) => byId[id])).toEqual(expected);
  });
}

async function expectGroup(api: ApiDriver, id: string, assertion: (group: VersionedObject) => void): Promise<void> {
  await expect.poll(async () => {
    const group = (await objects(api, "group")).find((item) => item.id === id);
    expect(group).toBeDefined();
    assertion(group!);
    return true;
  }, { timeout: 2_000 }).toBe(true);
}

async function expectGroupMissing(api: ApiDriver, id: string): Promise<void> {
  await expect.poll(async () => (await objects(api, "group")).some((item) => item.id === id), { timeout: 2_000 }).toBe(false);
}

async function expectGroupNumbers(api: ApiDriver, id: string, expected: number[]): Promise<void> {
  const byId = await fixtureNumberById(api);
  await expectGroup(api, id, (group) => expect(group.body.fixtures.map((fixture: string) => byId[fixture])).toEqual(expected));
}

async function overwriteGroupByNumbers(api: ApiDriver, id: string, numbers: number[]): Promise<void> {
  const byNumber = await fixtureIdsByNumber(api);
  const existing = await object(api, "group", id);
  await putObject(api, "group", id, {
    ...existing.body,
    fixtures: numbers.map((number) => byNumber[number]),
    derived_from: null,
    frozen_from: null,
  }, existing.revision);
}

async function unpatchFixture(api: ApiDriver, fixtureId: string): Promise<void> {
  const fixture = (await objects(api, "patched_fixture")).find((item) => item.body.fixture_id === fixtureId);
  expect(fixture).toBeDefined();
  await putObject(api, "patched_fixture", fixture!.id, { ...fixture!.body, universe: null, address: null }, fixture!.revision);
}

async function expectSlotsAfterTick(bench: any, millis: number, expected: number[]): Promise<void> {
  const artnetMark = bench.artnet.mark();
  const sacnMark = bench.sacn.mark();
  const tick = await bench.tick(millis);
  const slots = tick.universes.find((universe: any) => universe.universe === 1)?.slots ?? [];
  expect(slots.slice(0, expected.length)).toEqual(expected);
  const artnet = await bench.artnet.nextAfter(artnetMark, "artnet", 1);
  const sacn = await bench.sacn.nextAfter(sacnMark, "sacn", 101);
  expect(Array.from(artnet.slots.slice(0, expected.length))).toEqual(expected);
  expect(Array.from(sacn.slots.slice(0, expected.length))).toEqual(expected);
}

function normalized(value: { value?: number } | number | undefined): number | undefined {
  return typeof value === "number" ? value : value?.value;
}
