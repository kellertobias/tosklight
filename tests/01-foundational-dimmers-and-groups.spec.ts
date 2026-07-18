import type { Page } from "../apps/control-ui/node_modules/@playwright/test/index.js";
import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import {
  commandLineRequiresLegacyCompatibility,
  type ApiDriver,
} from "../apps/control-ui/e2e/bench/api";
import { pairedScenario } from "../apps/control-ui/e2e/bench/pairedScenario";
import { loadCanonicalCopy } from "./support/catalog";

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
  pairedScenario({
    id: "DIM-001",
    title: "ordered Group edits retain their live value and append re-added fixtures",
    arrange: async ({ api, bench }, surface) => {
      await loadCompactRig(api, bench, `dim-001-paired-${surface}`);
      return {};
    },
    api: async ({ api }) => {
      for (const value of [
        "GROUP 3 AT 50",
        "GROUP 3 + 5 + 6",
        "RECORD GROUP 3",
        "GROUP 3 - 2 + 2",
        "RECORD GROUP 3",
      ]) await command(api, value);
    },
    ui: async ({ api, desk, page }) => {
      await desk.open(api.baseUrl);
      for (const [value, visible] of [
        ["GROUP 3 AT 50", "G3 AT 50"],
        ["GROUP 3 + 5 + 6", "G3 + F5 + F6"],
        ["RECORD GROUP 3", "RECORD GROUP 3"],
        ["GROUP 3 - 2 + 2", "G3 - F2 + F2"],
        ["RECORD GROUP 3", "RECORD GROUP 3"],
      ] as const) await pressCommandAndWait(page, value, visible);
    },
    assert: async ({ api, bench }) => {
      await expectGroupNumbers(api, "3", [1, 3, 4, 5, 6, 2]);
      await expectProgrammer(api, (state) => {
        expect(state.group_values["3"]?.[INTENSITY]).toBeDefined();
        expect(state.values).toHaveLength(0);
      });
      await expectSlotsAfterTick(bench, 3_000, [128, 128, 128, 128, 128, 128]);
    },
  });

  pairedScenario({
    id: "DIM-002",
    title: "Lightning Desk command reaches the exact rendered output boundary",
    arrange: async ({ api, bench }, surface) => {
      await loadCompactRig(api, bench, `dim-002-paired-${surface}`);
      return {};
    },
    api: async ({ api }) => command(api, "GROUP 1 AT 50"),
    ui: async ({ api, desk, page }) => {
      await desk.open(api.baseUrl);
      await pressCommandAndWait(page, "GROUP 1 AT 50", "G1 AT 50");
    },
    assert: async ({ api, bench }) => {
      await expectProgrammer(api, (state) => expect(normalized(state.group_values["1"][INTENSITY].value)).toBe(0.5));
      await expectSlotsAfterTick(bench, 2_999, Array(12).fill(127));
      await expectSlotsAfterTick(bench, 1, Array(12).fill(128));
      await expectSlotsAfterTick(bench, 1, Array(12).fill(128));
    },
  });

  pairedScenario({
    id: "CMD-001",
    title: "Fixture and Group default modes toggle while explicit prefixes stay scoped",
    arrange: async ({ api, bench }, surface) => {
      await loadCompactRig(api, bench, `cmd-001-paired-${surface}`);
      return {};
    },
    api: async ({ api }) => {
      await api.command("programmer.command_target", { value: "GROUP" });
      await api.command("programmer.command_target", { value: "FIXTURE" });
      await command(api, "G1 + F2");
    },
    ui: async ({ api, desk, page }) => {
      await desk.open(api.baseUrl);
      const commandLine = page.getByLabel("Command line");
      await page.getByRole("button", { name: "GRP", exact: true }).click();
      await expect(commandLine).toHaveValue("GROUP");
      await page.getByRole("button", { name: "ENT", exact: true }).click();
      await expect(commandLine).toHaveValue("GROUP");
      await page.getByRole("button", { name: "GRP", exact: true }).click();
      await expect(commandLine).toHaveValue("FIXTURE");
      await page.getByRole("button", { name: "ENT", exact: true }).click();
      await expect(commandLine).toHaveValue("FIXTURE");
      await pressCommandAndWait(page, "GROUP 1 + 2", "G1 + F2");
    },
    assert: async ({ api }) => {
      const fixtures = await fixtureIdsByNumber(api);
      await expectSelectedNumbers(api, [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
      await expectProgrammer(api, (state) => {
        expect(state.selection_expression).toMatchObject({
          type: "sources",
          items: [
            { type: "live_group", group_id: "1" },
            { type: "fixture", fixture_id: fixtures[2] },
          ],
        });
      });
    },
  });

  pairedScenario({
    id: "GROUP-003",
    title: "derived Group follows source ordering",
    arrange: async ({ api, bench }, surface) => {
      await loadCompactRig(api, bench, `group-003-paired-${surface}`);
      return { sourceOrder: [12, 1, 2, 8, 4, 5, 6, 7, 9, 10, 11] };
    },
    api: async ({ api }, state) => {
      await command(api, "GROUP 1 DIV 2");
      await command(api, "RECORD GROUP 5");
      await overwriteGroupByNumbers(api, "1", state.sourceOrder);
    },
    ui: async ({ api, desk, page }, state) => {
      await desk.open(api.baseUrl);
      await pressCommandAndWait(page, "GROUP 1 DIV 2", "G1 DIV 2");
      await pressCommandAndWait(page, "RECORD GROUP 5", "RECORD GROUP 5");
      await selectFixtureRows(api, page, state.sourceOrder);
      await recordExistingGroup(page, 1, "Overwrite");
    },
    assert: async ({ api }, state) => {
      await expectGroupNumbers(api, "1", state.sourceOrder);
      await expectGroupNumbers(api, "5", [12, 2, 4, 6, 9, 11]);
      await expectGroup(api, "5", (group) => {
        expect(group.body.derived_from).toMatchObject({
          source_group_id: "1",
          rule: { type: "every_nth", n: 2, offset: 0 },
        });
        expect(group.body.frozen_from).toBeNull();
      });
      await expectGroupNumbers(api, "4", []);
    },
  });

  pairedScenario({
    id: "GROUP-004",
    title: "frozen Group survives source edits and keeps unpatched fixtures programmable",
    arrange: async ({ api, bench }, surface) => {
      await loadCompactRig(api, bench, `group-004-paired-${surface}`);
      return {
        fixtures: await fixtureIdsByNumber(api),
        sourceOrder: [12, 1, 2, 4, 5, 6, 7, 8, 9, 10, 11],
      };
    },
    api: async ({ api }, state) => {
      await command(api, "GROUP GROUP 1");
      await command(api, "RECORD GROUP 5");
      await overwriteGroupByNumbers(api, "1", state.sourceOrder);
      await unpatchFixture(api, state.fixtures[3]);
      await command(api, "GROUP 5 AT 50");
    },
    ui: async ({ api, desk, page }, state) => {
      await desk.open(api.baseUrl);
      await openGroups(page);
      await groupCard(page, 1).dblclick();
      await pressCommandAndWait(page, "RECORD GROUP 5", "RECORD GROUP 5");
      await selectFixtureRows(api, page, state.sourceOrder);
      await recordExistingGroup(page, 1, "Overwrite");

      await openPatch(page);
      const row = patchFixtureRow(page, 3);
      const address = row.locator(".patch-address");
      await page.getByRole("button", { name: "SET", exact: true }).click();
      await address.click();
      const editor = page.locator(".patch-edit-modal");
      await editor.getByLabel("Fixture address").fill("");
      await editor.getByRole("button", { name: "Set", exact: true }).click();
      await expect(address).toHaveText("Unpatched");

      await openGroups(page);
      await groupCard(page, 5).click();
      await setDimmerByTouch(page, 50);
    },
    assert: async ({ api, bench }, state) => {
      await expectGroup(api, "5", (group) => {
        expect(group.body.frozen_from).toMatchObject({ source_group_id: "1" });
        expect(group.body.derived_from).toBeNull();
      });
      await expectGroupNumbers(api, "5", [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
      const fixture3 = await object<any>(api, "patched_fixture", state.fixtures[3]);
      expect(fixture3.body.universe).toBeNull();
      expect(fixture3.body.address).toBeNull();
      await expectProgrammer(api, (programmerState) => expect(programmerState.group_values["5"]?.[INTENSITY]).toBeDefined());
      await expectSlotsAfterTick(bench, 3_000, [128, 128, 0, 128, 128, 128, 128, 128, 128, 128, 128, 128]);
    },
  });

  pairedScenario<{ fixtures: Record<number, string>; rangedGroups: string[]; emptySlots: number[] }>({
    id: "GROUP-005",
    title: "stored empty Groups remain distinct from missing references",
    arrange: async ({ api, bench }, surface) => {
      await loadCompactRig(api, bench, `group-005-paired-${surface}`);
      await command(api, "DELETE GROUP 4");
      return { fixtures: await fixtureIdsByNumber(api), rangedGroups: [], emptySlots: [] };
    },
    api: async ({ api, bench }, state) => {
      await command(api, "GROUP 1 THRU 5");
      state.rangedGroups = (await programmer(api)).selection_expression?.items.map((item: any) => item.group_id) ?? [];
      await select(api, []);
      await command(api, "RECORD GROUP 4");
      await command(api, "GROUP 4 AT 50");
      state.emptySlots = slotsFromFrame(await bench.tick(3_000), 12);
      await select(api, [state.fixtures[1]]);
      await command(api, "RECORD + GROUP 4");
    },
    ui: async ({ api, bench, desk, page }, state) => {
      await desk.open(api.baseUrl);
      await pressCommandAndWait(page, "GROUP 1 THRU 5", "G1 THRU 5");
      await expectProgrammer(api, (programmerState) => {
        state.rangedGroups = programmerState.selection_expression?.items.map((item: any) => item.group_id) ?? [];
        expect(state.rangedGroups).toEqual(["1", "2", "3"]);
      });
      await page.getByRole("button", { name: "CLR", exact: true }).click();
      await openGroups(page);
      await page.locator(".global-store-button").click();
      await groupCard(page, 4).click();
      await expect(page.locator(".record-mode-dialog")).toHaveCount(0);
      await expectGroupNumbers(api, "4", []);
      await expect(page.locator(".global-store-button")).toHaveText("REC");
      await expect(groupCard(page, 4)).toContainText("Group 4");
      await groupCard(page, 4).click();
      await expectProgrammer(api, (programmerState) => {
        expect(programmerState.selection_expression).toMatchObject({
          type: "sources",
          items: [{ type: "live_group", group_id: "4" }],
        });
      });
      await expect(page.locator(".vertical-touch-fader-stack").filter({ hasText: "Enc 1 · Dimmer" })).toBeVisible();
      await setDimmerByTouch(page, 50);
      state.emptySlots = slotsFromFrame(await bench.tick(3_000), 12);
      await openFixtures(page);
      await fixtureRow(page, 1).click();
      await openGroups(page);
      await page.locator(".global-store-button").click();
      await groupCard(page, 4).click();
    },
    assert: async ({ api, bench }, state) => {
      expect(state.rangedGroups).toEqual(["1", "2", "3"]);
      expect(state.emptySlots).toEqual(Array(12).fill(0));
      await expectGroupNumbers(api, "4", [1]);
      await expectProgrammer(api, (programmerState) => expect(programmerState.group_values["4"]?.[INTENSITY]).toBeDefined());
      await expectSlotsAfterTick(bench, 0, [128, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    },
  });

  pairedScenario({
    id: "PROG-001",
    title: "values retain selection until replacement while leading Plus continues it",
    arrange: async ({ api, bench }, surface) => {
      await loadCompactRig(api, bench, `prog-001-paired-${surface}`);
      return {};
    },
    api: async ({ api }) => {
      for (const value of ["1 + 2 AT 50", "AT 25", "3 AT 75", "+ 4 AT 100"]) await command(api, value);
    },
    ui: async ({ api, desk, page }) => {
      await desk.open(api.baseUrl);
      for (const [value, visible] of [
        ["1 + 2 AT 50", "F1 + F2 AT 50"],
        ["AT 25", "AT 25"],
        ["3 AT 75", "F3 AT 75"],
        ["+ 4 AT 100", "+F4 AT 100"],
      ] as const) await enterCommandWithoutEscape(page, value, visible);
    },
    assert: async ({ api, bench }) => {
      await expectSelectedNumbers(api, [3, 4]);
      await expectProgrammer(api, (state) => {
        const intensity = state.values.filter((value) => value.attribute === INTENSITY);
        expect(intensity).toHaveLength(4);
      });
      await expectSlotsAfterTick(bench, 3_000, [64, 64, 255, 255, 0, 0, 0, 0, 0, 0, 0, 0]);
    },
  });

  pairedScenario<{ initialSlots: number[]; order: number[] }>({
    id: "PROG-002",
    title: "relative values spread across the live ordered Group",
    arrange: async ({ api, bench }, surface) => {
      await loadCompactRig(api, bench, `prog-002-paired-${surface}`);
      await overwriteGroupByNumbers(api, "1", [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
      return { initialSlots: [], order: [12, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10] };
    },
    api: async ({ api, bench }, state) => {
      await command(api, "GROUP 1 AT 0 THRU 100");
      state.initialSlots = slotsFromFrame(await bench.tick(3_000), 12);
      await overwriteGroupByNumbers(api, "1", state.order);
    },
    ui: async ({ api, bench, desk, page }, state) => {
      await desk.open(api.baseUrl);
      await pressCommandAndWait(page, "GROUP 1 AT 0 THRU 100", "G1 AT 0 THRU 100");
      state.initialSlots = slotsFromFrame(await bench.tick(3_000), 12);
      await selectFixtureRows(api, page, state.order);
      await recordExistingGroup(page, 1, "Overwrite");
    },
    assert: async ({ api, bench }, state) => {
      expect(state.initialSlots).toEqual([0, 28, 57, 85, 113, 142, 170, 198, 227, 255, 0, 0]);
      await expectGroupNumbers(api, "1", state.order);
      await expectProgrammer(api, (programmerState) => {
        expect(programmerState.group_values["1"]?.[INTENSITY]?.value).toMatchObject({ kind: "spread", value: [0, 1] });
        expect(programmerState.values).toHaveLength(0);
      });
      await expectSlotsAfterTick(bench, 0, [26, 51, 77, 102, 128, 153, 179, 204, 230, 255, 0, 0]);
    },
  });

  test("PROG-002 @ui › fixture ranges and retained selections spread through the desk command line", async ({ api, bench, desk, page }) => {
    await loadCompactRig(api, bench, "prog-002-fixture-command-ui");
    await desk.open(api.baseUrl);

    await pressCommandAndWait(page, "1 THRU 5 AT 20 THRU 50", "F1 THRU 5 AT 20 THRU 50");
    await expectSlotsAfterTick(bench, 3_000, [51, 70, 89, 108, 128, 0, 0, 0, 0, 0, 0, 0]);

    await pressCommandAndWait(page, "1 THRU 5", "F1 THRU 5");
    await pressCommandAndWait(page, "AT 0 THRU 50", "AT 0 THRU 50");
    await expectSlotsAfterTick(bench, 3_000, [0, 32, 64, 96, 128, 0, 0, 0, 0, 0, 0, 0]);
  });

  pairedScenario<{ overrideSlots: number[]; fixture: string }>({
    id: "PROG-003",
    title: "newer fixture intensity wins LTP and releases back to its Group value",
    arrange: async ({ api, bench }, surface) => {
      await loadCompactRig(api, bench, `prog-003-paired-${surface}`);
      return { overrideSlots: [], fixture: (await fixtureIdsByNumber(api))[1] };
    },
    api: async ({ api, bench }, state) => {
      await command(api, "GROUP 1 AT 50");
      await command(api, "1 AT 25");
      state.overrideSlots = slotsFromFrame(await bench.tick(3_000), 12);
      await api.command("programmer.release", { fixture_id: state.fixture, attribute: INTENSITY });
    },
    ui: async ({ api, bench, desk, page }, state) => {
      await desk.open(api.baseUrl);
      await openGroups(page);
      await groupCard(page, 1).click();
      await setDimmerByTouch(page, 50);
      await openFixtures(page);
      await fixtureRow(page, 1).click();
      await setDimmerByTouch(page, 25);
      state.overrideSlots = slotsFromFrame(await bench.tick(3_000), 12);
      await page.getByRole("button", { name: "Release Dimmer" }).click();
    },
    assert: async ({ api, bench }, state) => {
      expect(state.overrideSlots).toEqual([64, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128]);
      await expectProgrammer(api, (programmerState) => {
        expect(programmerState.group_values["1"]?.[INTENSITY]).toBeDefined();
        expect(programmerState.values.some((value) => value.fixture_id === state.fixture && value.attribute === INTENSITY)).toBe(false);
      });
      await expectSlotsAfterTick(bench, 0, Array(12).fill(128));
    },
  });

  pairedScenario<{ afterFirstClear: { selected: number; values: number; slots: number[] } }>({
    id: "PROG-004",
    title: "Clear removes selection first and programmer values second",
    arrange: async ({ api, bench }, surface) => {
      await loadCompactRig(api, bench, `prog-004-paired-${surface}`);
      return { afterFirstClear: { selected: -1, values: -1, slots: [] } };
    },
    api: async ({ api, bench }, state) => {
      await command(api, "1 + 2 AT 50");
      await bench.tick(3_000);
      await select(api, []);
      const first = await programmer(api);
      state.afterFirstClear = {
        selected: first.selected.length,
        values: first.values.length,
        slots: slotsFromFrame(await bench.tick(0), 12),
      };
      await api.command("programmer.clear", {});
    },
    ui: async ({ api, bench, desk, page }, state) => {
      await desk.open(api.baseUrl);
      await pressCommandAndWait(page, "1 + 2 AT 50", "F1 + F2 AT 50");
      await bench.tick(3_000);
      const clear = page.getByRole("button", { name: "CLR", exact: true });
      await clear.click();
      await expect(clear).toHaveClass(/clear-warning/);
      await expectProgrammer(api, (programmerState) => {
        if (programmerState.selected.length !== 0 || programmerState.values.length !== 2) throw new Error("first Clear stage has not settled");
        state.afterFirstClear = {
          selected: programmerState.selected.length,
          values: programmerState.values.length,
          slots: [],
        };
      });
      state.afterFirstClear.slots = slotsFromFrame(await bench.tick(0), 12);
      await clear.click();
      await expect(clear).toHaveClass(/clear-idle/);
    },
    assert: async ({ api, bench }, state) => {
      expect(state.afterFirstClear).toEqual({
        selected: 0,
        values: 2,
        slots: [128, 128, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
      });
      await expectProgrammer(api, (programmerState) => {
        expect(programmerState.selected).toHaveLength(0);
        expect(programmerState.values).toHaveLength(0);
        expect(Object.keys(programmerState.group_values)).toHaveLength(0);
      });
      await expectSlotsAfterTick(bench, 0, Array(12).fill(0));
    },
  });

  test("DIM-001 @supplemental › exhaustive API add, subtract, deletion, and dependency boundaries", async ({ api, bench }) => {
    const prepare = async (name: string) => {
      await loadCompactRig(api, bench, name);
      const fixtures = await fixtureIdsByNumber(api);
      await command(api, "GROUP 3 AT 50");
      await expectProgrammer(api, (programmer) => {
        expect(programmer.group_values["3"]?.[INTENSITY]).toBeDefined();
        expect(programmer.values).toHaveLength(0);
        expect(programmer.selection_expression).toMatchObject({ type: "live_group", group_id: "3" });
      });
      await expectSlotsAfterTick(bench, 3_000, [128, 128, 128, 128, 0, 0]);
      return fixtures;
    };

    // The three documented add-to-end workflows start from independent show copies.
    for (const workflow of ["merge", "live-overwrite", "command-merge"] as const) {
      const fixtures = await prepare(`dim-001-${workflow}-api`);
      if (workflow === "live-overwrite") {
        await gestureGroup(api, "3");
        await gestureFixture(api, fixtures[5]);
        await gestureFixture(api, fixtures[6]);
        await expectProgrammer(api, (programmer) => {
          expect(programmer.selection_expression).toMatchObject({
            type: "sources",
            items: [
              { type: "live_group", group_id: "3" },
              { type: "fixture", fixture_id: fixtures[5] },
              { type: "fixture", fixture_id: fixtures[6] },
            ],
          });
        });
        await command(api, "RECORD GROUP 3");
      } else {
        await gestureFixture(api, fixtures[5]);
        await gestureFixture(api, fixtures[6]);
        await expectSelectedNumbers(api, [5, 6]);
        await command(api, "RECORD + GROUP 3");
      }
      await expectGroupNumbers(api, "3", [1, 2, 3, 4, 5, 6]);
      await expectProgrammer(api, (programmer) => expect(programmer.group_values["3"]?.[INTENSITY]).toBeDefined());
    }

    // Removal retains relative order; a later add and a remove-then-add in one expression append.
    await prepare("dim-001-primary-api");
    await command(api, "G3 + F5 + F6");
    await command(api, "RECORD GROUP 3");
    await command(api, "G3 - F2");
    await command(api, "RECORD GROUP 3");
    await expectGroupNumbers(api, "3", [1, 3, 4, 5, 6]);
    await expectSlotsAfterTick(bench, 0, [128, 0, 128, 128, 128, 128]);
    await command(api, "G3 + F2");
    await command(api, "RECORD GROUP 3");
    await expectGroupNumbers(api, "3", [1, 3, 4, 5, 6, 2]);
    await expectSlotsAfterTick(bench, 0, [128, 128, 128, 128, 128, 128]);

    await prepare("dim-001-left-to-right-api");
    await command(api, "G3 + F5 + F6");
    await command(api, "RECORD GROUP 3");
    await command(api, "G3 - F2 + F2");
    await expectSelectedNumbers(api, [1, 3, 4, 5, 6, 2]);
    await command(api, "RECORD GROUP 3");
    await expectGroupNumbers(api, "3", [1, 3, 4, 5, 6, 2]);

    // Subtract accepts one fixture or an ordered multi-fixture gesture without rebuilding Group 3.
    const subtractFixtures = await prepare("dim-001-subtract-api");
    await command(api, "G3 + F5 + F6");
    await command(api, "RECORD GROUP 3");
    await gestureFixture(api, subtractFixtures[2]);
    await command(api, "RECORD - GROUP 3");
    await expectGroupNumbers(api, "3", [1, 3, 4, 5, 6]);
    await gestureFixture(api, subtractFixtures[5]);
    await gestureFixture(api, subtractFixtures[6]);
    await command(api, "RECORD - GROUP 3");
    await expectGroupNumbers(api, "3", [1, 3, 4]);

    // Empty-selection subtract and DELETE are equivalent both when accepted and dependency-blocked.
    for (const operation of ["RECORD - GROUP 3", "DELETE GROUP 3"] as const) {
      await prepare(`dim-001-delete-${operation.startsWith("RECORD") ? "subtract" : "delete"}-api`);
      await select(api, []);
      await command(api, operation);
      await expectGroupMissing(api, "3");
    }
    let rejection = "";
    for (const operation of ["RECORD - GROUP 3", "DELETE GROUP 3"] as const) {
      await prepare(`dim-001-dependent-${operation.startsWith("RECORD") ? "subtract" : "delete"}-api`);
      const group3 = await object(api, "group", "3");
      await putObject(api, "group", "6", {
        id: "6",
        name: "Depends on 3",
        fixtures: [],
        derived_from: { source_group_id: "3", rule: { type: "all" } },
        frozen_from: null,
        programming: {},
        master: 1,
        playback_fader: null,
      });
      const before = JSON.stringify(await objects(api, "group"));
      await select(api, []);
      const error = await commandError(api, operation);
      expect(error).toContain("derived group 6 depends on it");
      if (rejection) expect(error).toContain(rejection);
      else rejection = "cannot delete group 3";
      expect(JSON.stringify(await objects(api, "group"))).toBe(before);
      expect((await object(api, "group", "3")).revision).toBe(group3.revision);
    }
  });

  test("DIM-002 @supplemental › repeated API fade endpoint and UDP stability", async ({ api, bench }) => {
    await loadCompactRig(api, bench, "dim-002-api");
    await command(api, "GROUP 1 AT 50");
    await expectProgrammer(api, (programmer) => expect(normalized(programmer.group_values["1"][INTENSITY].value)).toBe(0.5));
    await expectSlotsAfterTick(bench, 2_999, Array(12).fill(127));
    await expectSlotsAfterTick(bench, 1, Array(12).fill(128));
    await expectSlotsAfterTick(bench, 1, Array(12).fill(128));
  });

  test("DIM-002 @supplemental › repeated visible keypad fade endpoint", async ({ api, bench, desk, page }) => {
    await loadCompactRig(api, bench, "dim-002-ui");
    await desk.open(api.baseUrl);
    await pressCommand(page, "GROUP 1 AT 50", "G1 AT 50");
    await expectProgrammer(api, (programmer) => expect(normalized(programmer.group_values["1"][INTENSITY].value)).toBe(0.5));
    await expectSlotsAfterTick(bench, 2_999, Array(12).fill(127));
    await expectSlotsAfterTick(bench, 1, Array(12).fill(128));
    await expectSlotsAfterTick(bench, 1, Array(12).fill(128));
  });

  test("CMD-001 @supplemental › exhaustive visible prefix, geometry, range, Clear, and Escape cases", async ({ api, bench, desk, page }) => {
    await loadCompactRig(api, bench, "cmd-001-ui");
    await setGroupByNumbers(api, "4", "Middle Pair", [9, 10]);
    await setGroupByNumbers(api, "5", "Back Dimmers", [5, 6, 7, 8]);
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

    const enter = async (keys: string[], visible: string, selected: number[], target: "FIXTURE" | "GROUP") => {
      for (const key of keys) await press(key);
      await expect(commandLine).toHaveValue(visible);
      await press("ENT");
      await expect(commandLine).toHaveValue(target);
      await expectSelectedNumbers(api, selected);
    };
    const clear = async (target: "FIXTURE" | "GROUP") => {
      await press("CLR");
      await expect(commandLine).toHaveValue(target);
      await expectSelectedNumbers(api, []);
    };

    await expect(commandLine).toHaveValue("FIXTURE");
    await press("GRP");
    await expect(commandLine).toHaveValue("GROUP");
    await press("ENT");
    await expect(commandLine).toHaveValue("GROUP");
    await expectSelectedNumbers(api, []);

    await enter(["1", "+", "2"], "G1 + G2", [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12], "GROUP");
    await clear("GROUP");
    await press("GRP");
    await expect(commandLine).toHaveValue("FIXTURE");
    await enter(["1", "+", "2"], "F1 + G2", [1, 3, 5, 7, 9, 11], "GROUP");
    await clear("GROUP");
    await enter(["GRP", "1", "+", "GRP", "2"], "F1 + F2", [1, 2], "GROUP");
    await clear("GROUP");
    await enter(["3", "TRU", "5"], "G3 THRU 5", [1, 2, 3, 4, 9, 10, 5, 6, 7, 8], "GROUP");
    await clear("GROUP");
    await enter(["3", "TRU", "5", "+", "GRP", "6"], "G3 THRU 5 + F6", [1, 2, 3, 4, 9, 10, 5, 6, 7, 8], "GROUP");
    await clear("GROUP");

    await press("GRP");
    await expect(commandLine).toHaveValue("FIXTURE");
    await press("ENT");
    await expect(commandLine).toHaveValue("FIXTURE");
    await expectSelectedNumbers(api, []);
    await enter(["1", "+", "2"], "F1 + F2", [1, 2], "FIXTURE");
    await clear("FIXTURE");
    await enter(["GRP", "1", "+", "2"], "G1 + F2", [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12], "FIXTURE");
    await clear("FIXTURE");
    await enter(["GRP", "1", "+", "GRP", "2"], "G1 + G2", [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12], "FIXTURE");
    await clear("FIXTURE");
    await press("GRP");
    await press("GRP");
    await expect(commandLine).toHaveValue("DEGRP");
    await enter(["3", "+", "GRP", "5"], "DEGRP 3 + G5", [1, 2, 3, 4, 5, 6, 7, 8], "FIXTURE");
    await clear("FIXTURE");
    await enter(["GRP", "3", "TRU", "5"], "G3 THRU 5", [1, 2, 3, 4, 9, 10, 5, 6, 7, 8], "FIXTURE");
    await clear("FIXTURE");
    await enter(["GRP", "3", "TRU", "5", "+", "6"], "G3 THRU 5 + F6", [1, 2, 3, 4, 9, 10, 5, 6, 7, 8], "FIXTURE");
    await clear("FIXTURE");
    await page.getByRole("button", { name: "ESC", exact: true }).click();
    await expect(commandLine).toHaveValue("FIXTURE");
  });

  test("GROUP-003 @supplemental › second API source reorder remains live", async ({ api, bench }) => {
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

  test("GROUP-004 @supplemental › API frozen Preset storage and unpatched output boundary", async ({ api, bench }) => {
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

  test("PROG-001 @supplemental › Preset numbers are local to each family pool", async ({ api, bench }) => {
    await loadCompactRig(api, bench, "prog-001-family-local-preset-numbers");
    const fixtures = await fixtureIdsByNumber(api);
    const fixture = fixtures[1];

    await putObject(api, "preset", "2.1", {
      name: "Color one",
      family: "Color",
      number: 1,
      values: { [fixture]: { "color.red": { kind: "normalized", value: 1 } } },
      group_values: {},
    });
    await putObject(api, "preset", "3.1", {
      name: "Position one",
      family: "Position",
      number: 1,
      values: { [fixture]: { pan: { kind: "normalized", value: 0.25 } } },
      group_values: {},
    });

    const colorOne = await object(api, "preset", "2.1");
    const positionOne = await object(api, "preset", "3.1");
    expect(colorOne.body).toMatchObject({ family: "Color", number: 1 });
    expect(positionOne.body).toMatchObject({ family: "Position", number: 1 });

    await select(api, [fixture]);
    await api.command("preset.apply", { family: "Color", number: 1 });
    await api.command("preset.apply", { family: "Position", number: 1 });
    await expectProgrammer(api, (programmer) => {
      expect(programmer.values).toEqual(expect.arrayContaining([
        expect.objectContaining({ fixture_id: fixture, attribute: "color.red" }),
        expect.objectContaining({ fixture_id: fixture, attribute: "pan" }),
      ]));
    });
  });

  test("GROUP-005 @supplemental › API deletion and missing-reference errors remain atomic", async ({ api, bench }) => {
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

  test("PROG-003 @supplemental › API higher/lower LTP and scoped release permutations", async ({ api, bench }) => {
    await loadCompactRig(api, bench, "prog-003-api");
    const fixtures = await fixtureIdsByNumber(api);

    await command(api, "GROUP 1 AT 50");
    await command(api, "1 AT 75");
    await api.command("programmer.set", { fixture_id: fixtures[1], attribute: "pan", value: 0.33 });
    await expectSlotsAfterTick(bench, 3_000, [191, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128]);

    await api.command("programmer.release", { fixture_id: fixtures[1], attribute: INTENSITY });
    await expectProgrammer(api, (programmer) => {
      expect(programmer.values.some((value) => value.fixture_id === fixtures[1] && value.attribute === INTENSITY)).toBe(false);
      expect(programmer.values.some((value) => value.fixture_id === fixtures[1] && value.attribute === "pan")).toBe(true);
      expect(programmer.group_values["1"]?.[INTENSITY]).toBeDefined();
    });
    await expectSlotsAfterTick(bench, 0, Array(12).fill(128));

    await loadCompactRig(api, bench, "prog-003-lower-api");
    const lowerFixtures = await fixtureIdsByNumber(api);
    await command(api, "GROUP 1 AT 50");
    await command(api, "1 AT 25");
    await expectSlotsAfterTick(bench, 3_000, [64, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128]);

    await command(api, "GROUP 1 AT 50");
    await expectSlotsAfterTick(bench, 3_000, [128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128]);
    await api.command("programmer.group.set", { group_id: "1", attribute: "pan", value: 0.4 });
    await api.command("programmer.group.release", { group_id: "1", attribute: INTENSITY });
    await expectProgrammer(api, (programmer) => {
      expect(programmer.group_values["1"]?.[INTENSITY]).toBeUndefined();
      expect(programmer.group_values["1"]?.pan).toBeDefined();
      expect(programmer.values.some((value) => value.fixture_id === lowerFixtures[1] && value.attribute === INTENSITY)).toBe(true);
    });
    await expectSlotsAfterTick(bench, 0, [64, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
  });

  test("PROG-004 @supplemental › visible Clear styling, replacement, and continuation boundaries", async ({ api, bench, desk, page }) => {
    await loadCompactRig(api, bench, "prog-004-ui");
    await desk.open(api.baseUrl);
    const clear = page.getByRole("button", { name: "CLR", exact: true });
    await expect(clear).toHaveClass(/clear-idle/);

    await pressCommand(page, "GROUP 1", "G1");
    await expectProgrammer(api, (programmer) => expect(programmer.selected).toHaveLength(12));
    await expect(clear).toHaveClass(/clear-active/);
    await clear.click();
    await expectProgrammer(api, (programmer) => {
      expect(programmer.selected).toHaveLength(0);
      expect(programmer.values).toHaveLength(0);
      expect(Object.keys(programmer.group_values)).toHaveLength(0);
    });
    await expect(clear).toHaveClass(/clear-idle/);

    await pressCommand(page, "1 + 2 AT 75", "F1 + F2 AT 75");
    await expectSelectedNumbers(api, [1, 2]);
    await expect(clear).toHaveClass(/clear-active/);
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
    await expect(clear).toHaveClass(/clear-warning/);
    await expectSlotsAfterTick(bench, 0, [128, 128, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

    await clear.click();
    await expectProgrammer(api, (programmer) => {
      expect(programmer.selected).toHaveLength(0);
      expect(programmer.values).toHaveLength(0);
      expect(Object.keys(programmer.group_values)).toHaveLength(0);
    });
    await expect(clear).toHaveClass(/clear-idle/);
    await expectSlotsAfterTick(bench, 0, Array(12).fill(0));

    await loadCompactRig(api, bench, "prog-004-ui-replacement");
    await desk.open(api.baseUrl);
    await pressCommand(page, "1 + 2 AT 75", "F1 + F2 AT 75");
    await pressCommand(page, "3 AT 80", "F3 AT 80");
    await expectSelectedNumbers(api, [3]);
    await expectSlotsAfterTick(bench, 3_000, [191, 191, 204, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

    await loadCompactRig(api, bench, "prog-004-ui-continuation");
    await desk.open(api.baseUrl);
    await pressCommand(page, "1 + 2 AT 75", "F1 + F2 AT 75");
    await pressCommand(page, "+ 3 AT 50", "+F3 AT 50");
    await expectSelectedNumbers(api, [1, 2, 3]);
    await expectSlotsAfterTick(bench, 3_000, [128, 128, 128, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
  });

  test("DIM-001 @supplemental › visible Merge and Overwrite dialogs retain live ordering", async ({ api, bench, desk, page }) => {
    await loadCompactRig(api, bench, "dim-001-ui");
    await desk.open(api.baseUrl);

    await openGroups(page);
    await groupCard(page, 3).click();
    await setDimmerByTouch(page, 50);
    await expectProgrammer(api, (state) => {
      expect(state.group_values["3"]?.[INTENSITY]).toBeDefined();
      expect(state.values).toHaveLength(0);
    });

    await openFixtures(page);
    await fixtureRow(page, 5).click();
    await fixtureRow(page, 6).click();
    await expectSelectedNumbers(api, [5, 6]);
    await openGroups(page);
    await recordExistingGroup(page, 3, "Merge");
    await expectGroupNumbers(api, "3", [1, 2, 3, 4, 5, 6]);
    await expectSlotsAfterTick(bench, 3_000, [128, 128, 128, 128, 128, 128]);

    await pressCommand(page, "GROUP 3 - 2", "G3 - F2");
    await recordExistingGroup(page, 3, "Overwrite");
    await expectGroupNumbers(api, "3", [1, 3, 4, 5, 6]);
    await expectVisibleGroupOrder(page, 3, [1, 3, 4, 5, 6]);
    await expectSlotsAfterTick(bench, 0, [128, 0, 128, 128, 128, 128]);

    await pressCommand(page, "GROUP 3 + 2", "G3 + F2");
    await recordExistingGroup(page, 3, "Overwrite");
    await expectGroupNumbers(api, "3", [1, 3, 4, 5, 6, 2]);
    await expectVisibleGroupOrder(page, 3, [1, 3, 4, 5, 6, 2]);
    await expectSlotsAfterTick(bench, 0, [128, 128, 128, 128, 128, 128]);
  });

  test("CMD-001 @supplemental › exhaustive API default-mode, range, and dereference cases", async ({ api, bench }) => {
    await loadCompactRig(api, bench, "cmd-001-api");
    await setGroupByNumbers(api, "4", "Middle Pair", [9, 10]);
    await setGroupByNumbers(api, "5", "Back Dimmers", [5, 6, 7, 8]);

    type ExpectedSource = ["fixture" | "live_group", number | string];
    const enter = async (
      value: string,
      expectedNumbers: number[],
      expectedSources: ExpectedSource[],
    ) => {
      await api.command("programmer.command_line", { value });
      expect((await programmer(api)).command_line).toBe(value);
      await command(api, value);
      await expectSelectedNumbers(api, expectedNumbers);
      const state = await programmer(api);
      expect(state.selection_expression?.type).toBe("sources");
      const sources = state.selection_expression.items.map((source: any) =>
        source.type === "fixture"
          ? ["fixture", source.fixture_id]
          : ["live_group", source.group_id],
      );
      const fixtures = await fixtureIdsByNumber(api);
      expect(sources).toEqual(expectedSources.map(([type, id]) => [type, type === "fixture" ? fixtures[id as number] : String(id)]));
      await select(api, []);
    };

    // Cases 1–8: Group is the persistent default. Bare terms are live Groups while explicit
    // Fixture terms remain scoped to only their own address term.
    await api.command("programmer.command_target", { value: "GROUP" });
    await select(api, []);
    await enter("G1 + G2", [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12], [["live_group", "1"], ["live_group", "2"]]);
    await enter("F1 + G2", [1, 3, 5, 7, 9, 11], [["fixture", 1], ["live_group", "2"]]);
    await enter("F1 + F2", [1, 2], [["fixture", 1], ["fixture", 2]]);
    await enter("G3 THRU 5", [1, 2, 3, 4, 9, 10, 5, 6, 7, 8], [["live_group", "3"], ["live_group", "4"], ["live_group", "5"]]);
    await enter("G3 THRU 5 + F6", [1, 2, 3, 4, 9, 10, 5, 6, 7, 8], [["live_group", "3"], ["live_group", "4"], ["live_group", "5"], ["fixture", 6]]);

    // Cases 9–16: Fixture is the persistent default. A single explicit Group prefix remains
    // live; DEGRP expands only its own term to fixture references.
    await api.command("programmer.command_target", { value: "FIXTURE" });
    await enter("F1 + F2", [1, 2], [["fixture", 1], ["fixture", 2]]);
    await enter("G1 + F2", [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12], [["live_group", "1"], ["fixture", 2]]);
    await enter("G1 + G2", [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12], [["live_group", "1"], ["live_group", "2"]]);
    await enter("DEGRP 3 + G5", [1, 2, 3, 4, 5, 6, 7, 8], [["fixture", 1], ["fixture", 2], ["fixture", 3], ["fixture", 4], ["live_group", "5"]]);
    await enter("G3 THRU 5", [1, 2, 3, 4, 9, 10, 5, 6, 7, 8], [["live_group", "3"], ["live_group", "4"], ["live_group", "5"]]);
    await enter("G3 THRU 5 + F6", [1, 2, 3, 4, 9, 10, 5, 6, 7, 8], [["live_group", "3"], ["live_group", "4"], ["live_group", "5"], ["fixture", 6]]);

    expect((await programmer(api)).selected).toHaveLength(0);
  });

  test("GROUP-003 @supplemental › visible derived Group follows a second source reorder", async ({ api, bench, desk, page }) => {
    await loadCompactRig(api, bench, "group-003-ui");
    await desk.open(api.baseUrl);
    await pressCommand(page, "GROUP 1 DIV 2", "G1 DIV 2");
    await pressCommand(page, "RECORD GROUP 5", "RECORD GROUP 5");

    const firstOrder = [12, 1, 2, 4, 5, 6, 7, 8, 9, 10, 11];
    await selectFixtureRows(api, page, firstOrder);
    await expectSelectedNumbers(api, firstOrder);
    await recordExistingGroup(page, 1, "Overwrite");
    await expectGroupNumbers(api, "1", firstOrder);
    await expectGroupNumbers(api, "5", [12, 2, 5, 7, 9, 11]);
    await expect((await object<any>(api, "group", "5")).body.derived_from).toMatchObject({
      source_group_id: "1",
      rule: { type: "every_nth", n: 2, offset: 0 },
    });

    const secondOrder = [12, 1, 2, 8, 4, 5, 6, 7, 9, 10, 11];
    await selectFixtureRows(api, page, secondOrder);
    await expectSelectedNumbers(api, secondOrder);
    await recordExistingGroup(page, 1, "Overwrite");
    await expectGroupNumbers(api, "1", secondOrder);
    await expectGroupNumbers(api, "5", [12, 2, 4, 6, 9, 11]);
    await expectGroupNumbers(api, "4", []);

    await openGroups(page);
    await groupCard(page, 5).click();
    await expectSelectedNumbers(api, [12, 2, 4, 6, 9, 11]);
  });

  test("GROUP-004 @supplemental › frozen Group remains visible across Stage and Fixture panes", async ({ api, bench, desk, page }) => {
    await loadCompactRig(api, bench, "group-004-ui");
    await desk.open(api.baseUrl);
    const fixtures = await fixtureIdsByNumber(api);

    await openGroups(page);
    await groupCard(page, 1).dblclick();
    await expectProgrammer(api, (state) => {
      expect(state.selected).toHaveLength(12);
      expect(state.selection_expression?.type).toBe("frozen_group");
    });
    await pressCommand(page, "RECORD GROUP 5", "RECORD GROUP 5");
    await expect((await object<any>(api, "group", "5")).body.frozen_from).toMatchObject({ source_group_id: "1" });

    const reordered = [12, 1, 2, 4, 5, 6, 7, 8, 9, 10, 11];
    await selectFixtureRows(api, page, reordered);
    await recordExistingGroup(page, 1, "Overwrite");
    await expectGroupNumbers(api, "1", reordered);

    await openPatch(page);
    const fixture3Row = patchFixtureRow(page, 3);
    const address = fixture3Row.locator(".patch-address");
    await page.getByRole("button", { name: "SET", exact: true }).click();
    await address.click();
    const editor = page.locator(".patch-edit-modal");
    await expect(editor.getByRole("heading", { name: "Set fixture address" })).toBeVisible();
    await editor.getByLabel("Fixture address").fill("");
    await editor.getByRole("button", { name: "Set", exact: true }).click();
    await expect(address).toHaveText("Unpatched");
    await expect.poll(async () => (await object<any>(api, "patched_fixture", fixtures[3])).body.universe).toBeNull();

    await openGroups(page);
    await expect(groupCard(page, 5)).toHaveClass(/frozen/);
    await expect(groupCard(page, 5)).not.toContainText("missing");
    await groupCard(page, 5).click();
    await setDimmerByTouch(page, 50);
    await expectGroupNumbers(api, "5", [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
    await expectProgrammer(api, (state) => expect(state.group_values["5"]?.[INTENSITY]).toBeDefined());
    await expectSlotsAfterTick(bench, 3_000, [128, 128, 0, 128, 128, 128, 128, 128, 128, 128, 128, 128]);

    await openFixtures(page);
    await expect(fixtureRow(page, 3)).toBeVisible();
    await openBuiltIn(page, "Stage");
    await expect(stageFixture(page, fixtures[3])).toBeVisible();
  });

  test("GROUP-005 @supplemental › visible deletion, missing errors, and repeated range skipping", async ({ api, bench, desk, page }) => {
    test.setTimeout(60_000);
    await loadCompactRig(api, bench, "group-005-ui");
    await command(api, "DELETE GROUP 4");
    await expectGroupMissing(api, "4");
    await desk.open(api.baseUrl);

    await pressCommand(page, "GROUP 1 THRU 5", "G1 THRU 5");
    await expectProgrammer(api, (state) => {
      expect(state.selection_expression?.items.map((item: any) => item.group_id)).toEqual(["1", "2", "3"]);
    });
    await expectGroupMissing(api, "4");
    await page.getByRole("button", { name: "CLR", exact: true }).click();
    await expectSelectedNumbers(api, []);

    await openGroups(page);
    await page.locator(".global-store-button").click();
    await groupCard(page, 4).click();
    await expect(page.locator(".record-mode-dialog")).toHaveCount(0);
    await expectGroupNumbers(api, "4", []);
    await expect(page.locator(".global-store-button")).toHaveText("REC");
    await expect(groupCard(page, 4)).toContainText("Group 4");

    await groupCard(page, 4).click();
    await expectProgrammer(api, (state) => {
      expect(state.selection_expression).toMatchObject({
        type: "sources",
        items: [{ type: "live_group", group_id: "4" }],
      });
    });
    await expect(page.locator(".vertical-touch-fader-stack").filter({ hasText: "Enc 1 · Dimmer" })).toBeVisible();
    await setDimmerByTouch(page, 50);
    await expectProgrammer(api, (state) => expect(state.group_values["4"]?.[INTENSITY]).toBeDefined());
    await expectSlotsAfterTick(bench, 3_000, Array(12).fill(0));

    await openFixtures(page);
    await fixtureRow(page, 1).click();
    await expectSelectedNumbers(api, [1]);
    await openGroups(page);
    await page.locator(".global-store-button").click();
    await groupCard(page, 4).click();
    await expect(page.locator(".record-mode-dialog")).toHaveCount(0);
    await expectGroupNumbers(api, "4", [1]);
    await expectSlotsAfterTick(bench, 0, [128, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

    await pressCommand(page, "DELETE GROUP 4", "DELETE GROUP 4");
    await expectGroupMissing(api, "4");
    await pressCommand(page, "GROUP 4", "G4");
    await expect(page.getByLabel("Command line")).toHaveClass(/error/);

    await pressCommand(page, "GROUP 1 THRU 5", "G1 THRU 5");
    await expectProgrammer(api, (state) => {
      expect(state.selection_expression?.items.map((item: any) => item.group_id)).toEqual(["1", "2", "3"]);
    });
    await expectGroupMissing(api, "4");
  });

  for (const surface of ["api", "ui"] as const) {
    test(`PROG-001 @supplemental-${surface} › drag, Preset, and mixed-source selection boundaries`, async ({ api, bench, desk, page }) => {
      await loadCompactRig(api, bench, `prog-001-${surface}`);
      if (surface === "ui") {
        test.setTimeout(90_000);
        const fixtures = await fixtureIdsByNumber(api);
        await putObject(api, "preset", "1.199", {
          name: "Selection Intensity",
          family: "Intensity",
          number: 199,
          values: {
            [fixtures[21]]: { intensity: { kind: "normalized", value: 0.6 } },
            [fixtures[22]]: { intensity: { kind: "normalized", value: 0.6 } },
          },
          group_values: {},
        });
        await desk.open(api.baseUrl);
        await openBuiltIn(page, "Stage");
        await stageFixture(page, fixtures[1]).click();
        await stageFixture(page, fixtures[2]).click();
        await expectSelectedNumbers(api, [1, 2]);

        const fixture3 = await stageFixture(page, fixtures[3]).boundingBox();
        const fixture4 = await stageFixture(page, fixtures[4]).boundingBox();
        expect(fixture3).toBeTruthy();
        expect(fixture4).toBeTruthy();
        await page.mouse.move(Math.min(fixture3!.x, fixture4!.x) - 3, Math.min(fixture3!.y, fixture4!.y) - 3);
        await page.mouse.down();
        await page.mouse.move(Math.max(fixture3!.x + fixture3!.width, fixture4!.x + fixture4!.width) + 3, Math.max(fixture3!.y + fixture3!.height, fixture4!.y + fixture4!.height) + 3, { steps: 5 });
        await page.mouse.up();
        await expectSelectedNumbers(api, [1, 2, 3, 4]);

        await openFixtures(page);
        await fixtureRow(page, 5).click();
        await openGroups(page);
        await groupCard(page, 2).click();
        await groupCard(page, 3).click();
        await groupCard(page, 1).click();
        await expectSelectedNumbers(api, [1, 2, 3, 4, 5, 7, 9, 11, 6, 8, 10, 12]);
        await expectProgrammer(api, (state) => {
          expect(state.selection_expression?.type).toBe("sources");
          expect(state.selection_expression?.items.map((item: any) => item.type)).toEqual([
            "fixture", "fixture", "fixture", "fixture", "fixture", "live_group", "live_group", "live_group",
          ]);
        });
        await setDimmerByTouch(page, 50);
        await expectSelectedNumbers(api, [1, 2, 3, 4, 5, 7, 9, 11, 6, 8, 10, 12]);
        await setDimmerByTouch(page, 25);

        await openBuiltIn(page, "Stage");
        await stageFixture(page, fixtures[21]).click();
        await stageFixture(page, fixtures[22]).click();
        await expectSelectedNumbers(api, [21, 22]);
        await openBuiltIn(page, "Presets");
        await page.locator(".preset-card").filter({ hasText: "Selection Intensity" }).click();
        await expectSelectedNumbers(api, [21, 22]);
        await pressCommand(page, "+ 23", "+F23");
        await expectSelectedNumbers(api, [21, 22, 23]);
        await setDimmerByTouch(page, 40);

        await openGroups(page);
        await groupCard(page, 3).click();
        await setDimmerByTouch(page, 25);
        await expectSelectedNumbers(api, [1, 2, 3, 4]);

        await openBuiltIn(page, "Stage");
        await stageFixture(page, fixtures[6]).click();
        await expectSelectedNumbers(api, [6]);
        await page.getByRole("button", { name: "CLR", exact: true }).click();
        await expectSelectedNumbers(api, []);
        await stageFixture(page, fixtures[7]).click();
        await openGroups(page);
        await groupCard(page, 3).click();
        await expectSelectedNumbers(api, [7, 1, 2, 3, 4]);
        await expectSlotsAfterTick(bench, 3_000, Array(12).fill(64));
      } else {
        await command(api, "1 + 2 AT 50");
        await command(api, "AT 25");
        await command(api, "3 AT 75");
        await command(api, "+ 4 AT 100");
        await expectSelectedNumbers(api, [3, 4]);
        await expectSlotsAfterTick(bench, 3_000, [64, 64, 255, 255, 0, 0, 0, 0, 0, 0, 0, 0]);
      }
    });

    test(`PROG-002 @supplemental-${surface} › repeated ascending live-order spread`, async ({ api, bench, desk, page }) => {
      await loadCompactRig(api, bench, `prog-002-${surface}`);
      await overwriteGroupByNumbers(api, "1", [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
      if (surface === "ui") {
        await desk.open(api.baseUrl);
        await pressCommand(page, "GROUP 1 AT 0 THRU 100", "G1 AT 0 THRU 100");
      } else {
        await command(api, "GROUP 1 AT 0 THRU 100");
      }
      await expectProgrammer(api, (state) => {
        expect(state.group_values["1"]?.[INTENSITY]?.value).toMatchObject({ kind: "spread", value: [0, 1] });
        expect(state.values).toHaveLength(0);
      });
      await expectSlotsAfterTick(bench, 3_000, [0, 28, 57, 85, 113, 142, 170, 198, 227, 255, 0, 0]);
      await overwriteGroupByNumbers(api, "1", [12, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
      await expectSlotsAfterTick(bench, 0, [26, 51, 77, 102, 128, 153, 179, 204, 230, 255, 0, 0]);
    });
  }

  test("PROG-001 @supplemental › API Preset recall preserves and closes gesture boundaries", async ({ api, bench }) => {
    await loadCompactRig(api, bench, "prog-001-preset-api");
    const fixtures = await fixtureIdsByNumber(api);
    await gestureFixture(api, fixtures[21]);
    await gestureFixture(api, fixtures[22]);
    await putObject(api, "preset", "1.200", {
      name: "LED intensity",
      family: "Intensity",
      number: 200,
      values: {
        [fixtures[21]]: { intensity: { kind: "normalized", value: 0.4 } },
        [fixtures[22]]: { intensity: { kind: "normalized", value: 0.4 } },
      },
      group_values: {},
    });
    const before = await programmer(api);
    expect(before.selected).toEqual([fixtures[21], fixtures[22]]);
    expect(before.selection_expression).toMatchObject({
      type: "sources",
      items: [
        { type: "fixture", fixture_id: fixtures[21] },
        { type: "fixture", fixture_id: fixtures[22] },
      ],
    });

    await api.command("preset.apply", { family: "Intensity", number: 200 });
    const recalled = await programmer(api);
    expect(recalled.selected).toEqual(before.selected);
    expect(recalled.selection_expression).toEqual(before.selection_expression);
    expect(recalled.values.filter((value) => value.attribute === INTENSITY)).toHaveLength(2);

    await gestureFixture(api, fixtures[23]);
    const replacement = await programmer(api);
    expect(replacement.selected).toEqual([fixtures[23]]);
    expect(replacement.values.filter((value) => value.attribute === INTENSITY)).toHaveLength(2);
  });

  test("PROG-002 @supplemental › uniform, descending, multi-point, storage, and recall permutations", async ({ api, bench }) => {
    const fresh = async (name: string) => {
      await loadCompactRig(api, bench, name);
      await overwriteGroupByNumbers(api, "1", [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    };
    const createCuePlayback = async (number: number) => {
      const cueListId = crypto.randomUUID();
      await putObject(api, "cue_list", cueListId, {
        id: cueListId,
        name: `Spread ${number}`,
        priority: 0,
        mode: "sequence",
        looped: false,
        chaser_step_millis: 1000,
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
          name: "",
          changes: [],
          fade_millis: 0,
          delay_millis: 0,
          trigger: { type: "manual" },
          phasers: [],
          group_changes: [],
        }],
      });
      await putObject(api, "playback", String(number), {
        number,
        name: `Spread ${number}`,
        target: { type: "cue_list", cue_list_id: cueListId },
      });
      return cueListId;
    };

    await fresh("prog-002-uniform-api");
    await command(api, "GROUP 1 AT 0");
    await expectProgrammer(api, (state) => {
      expect(normalized(state.group_values["1"]?.[INTENSITY]?.value)).toBe(0);
      expect(state.values).toHaveLength(0);
    });
    await expectSlotsAfterTick(bench, 3_000, Array(12).fill(0));

    await fresh("prog-002-descending-api");
    await command(api, "GROUP 1 AT 100 THRU 0");
    await expectProgrammer(api, (state) => {
      expect(state.group_values["1"]?.[INTENSITY]?.value).toMatchObject({ kind: "spread", value: [1, 0] });
      expect(state.values).toHaveLength(0);
    });
    await expectSlotsAfterTick(bench, 3_000, [255, 227, 198, 170, 142, 113, 85, 57, 28, 0, 0, 0]);

    await fresh("prog-002-multi-point-api");
    await command(api, "GROUP 1 AT 100 THRU 0 THRU 100");
    const multi = await bench.tick(3_000);
    const multiSlots = multi.universes.find((universe: any) => universe.universe === 1)!.slots.slice(0, 10);
    expect(multiSlots).toEqual([...multiSlots].reverse());
    expect(multiSlots[0]).toBe(255);
    expect(multiSlots[9]).toBe(255);
    expect(multiSlots[4]).toBe(multiSlots[5]);
    expect(multiSlots.slice(0, 5)).toEqual([...multiSlots.slice(0, 5)].sort((left: number, right: number) => right - left));
    expect(multiSlots.slice(5)).toEqual([...multiSlots.slice(5)].sort((left: number, right: number) => left - right));

    // Live Group storage remains one relative spread in the programmer, Preset, and Cue. Editing
    // membership recalculates the current value and both recalls over eleven ordered members.
    await fresh("prog-002-live-storage-api");
    const liveCueListId = await createCuePlayback(1);
    await command(api, "GROUP 1 AT 0 THRU 100");
    await command(api, "RECORD 1.1");
    await command(api, "RECORD SET 1 CUE 1");
    let preset = await object(api, "preset", "1.1");
    expect(preset.body.group_values["1"]?.[INTENSITY]).toMatchObject({ kind: "spread", value: [0, 1] });
    expect(Object.keys(preset.body.values)).toHaveLength(0);
    let cueList = await object(api, "cue_list", liveCueListId);
    expect(cueList.body.cues[0].group_changes).toEqual(expect.arrayContaining([
      expect.objectContaining({ group_id: "1", attribute: INTENSITY, value: expect.objectContaining({ kind: "spread", value: [0, 1] }) }),
    ]));
    expect(cueList.body.cues[0].changes).toHaveLength(0);
    await overwriteGroupByNumbers(api, "1", [12, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    const liveEleven = [26, 51, 77, 102, 128, 153, 179, 204, 230, 255, 0, 0];
    await expectSlotsAfterTick(bench, 3_000, liveEleven);
    await api.command("programmer.clear", {});
    await command(api, "GROUP 1 AT 1.1");
    await expectProgrammer(api, (state) => {
      expect(state.group_values["1"]?.[INTENSITY]?.value).toMatchObject({ kind: "spread", value: [0, 1] });
      expect(state.values).toHaveLength(0);
    });
    await expectSlotsAfterTick(bench, 3_000, liveEleven);
    await api.command("programmer.clear", {});
    await api.command("playback.go", { cue_list_id: liveCueListId });
    await expectSlotsAfterTick(bench, 3_000, liveEleven);

    // Dereferencing calculates ten fixture-scoped values once. Group edits and recalls do not
    // attach the stored look to the newly inserted fixture 12.
    await fresh("prog-002-dereferenced-storage-api");
    const frozenCueListId = await createCuePlayback(2);
    await command(api, "DEGRP 1 AT 0 THRU 100");
    await expectProgrammer(api, (state) => {
      expect(Object.keys(state.group_values)).toHaveLength(0);
      expect(state.values).toHaveLength(10);
    });
    await command(api, "RECORD 1.2");
    await command(api, "RECORD SET 2 CUE 1");
    preset = await object(api, "preset", "1.2");
    expect(Object.keys(preset.body.group_values)).toHaveLength(0);
    expect(Object.keys(preset.body.values)).toHaveLength(10);
    cueList = await object(api, "cue_list", frozenCueListId);
    expect(cueList.body.cues[0].group_changes).toHaveLength(0);
    expect(cueList.body.cues[0].changes).toHaveLength(10);
    await overwriteGroupByNumbers(api, "1", [12, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    const frozenTen = [0, 28, 57, 85, 113, 142, 170, 198, 227, 255, 0, 0];
    await expectSlotsAfterTick(bench, 3_000, frozenTen);
    await api.command("programmer.clear", {});
    await command(api, "DEGRP 1 AT 1.2");
    await expectProgrammer(api, (state) => {
      expect(Object.keys(state.group_values)).toHaveLength(0);
      expect(state.values).toHaveLength(10);
    });
    await expectSlotsAfterTick(bench, 3_000, frozenTen);
    await api.command("programmer.clear", {});
    await api.command("playback.go", { cue_list_id: frozenCueListId });
    await expectSlotsAfterTick(bench, 3_000, frozenTen);
  });

  test("PROG-003 @supplemental › visible higher/lower LTP and scoped release permutations", async ({ api, bench, desk, page }) => {
    await loadCompactRig(api, bench, "prog-003-ui");
    await desk.open(api.baseUrl);
    await openGroups(page);
    await groupCard(page, 1).click();
    await setDimmerByTouch(page, 50);
    await openFixtures(page);
    await fixtureRow(page, 1).click();
    await setDimmerByTouch(page, 75);
    await expectSlotsAfterTick(bench, 3_000, [191, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128]);
    await page.getByRole("button", { name: "Release Dimmer" }).click();
    await expectSlotsAfterTick(bench, 0, Array(12).fill(128));

    await loadCompactRig(api, bench, "prog-003-ui-lower");
    await desk.open(api.baseUrl);
    await openGroups(page);
    await groupCard(page, 1).click();
    await setDimmerByTouch(page, 50);
    await openFixtures(page);
    await fixtureRow(page, 1).click();
    await setDimmerByTouch(page, 25);
    await expectSlotsAfterTick(bench, 3_000, [64, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128]);
    await pressCommand(page, "GROUP 1 AT 50", "G1 AT 50");
    await expectSlotsAfterTick(bench, 3_000, Array(12).fill(128));
    await page.getByRole("button", { name: "Release Dimmer" }).click();
    await expectSlotsAfterTick(bench, 0, [64, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    await openFixtures(page);
    await fixtureRow(page, 1).click();
    await page.getByRole("button", { name: "Release Dimmer" }).click();
    await expectSlotsAfterTick(bench, 0, Array(12).fill(0));
  });

  test("PROG-004 @supplemental › direct API clear-stage boundary", async ({ api, bench }) => {
    await loadCompactRig(api, bench, "prog-004-api");
    await command(api, "1 + 2 AT 50");
    await select(api, []);
    await expectProgrammer(api, (state) => {
      expect(state.selected).toHaveLength(0);
      expect(state.values).toHaveLength(2);
    });
    await expectSlotsAfterTick(bench, 3_000, [128, 128, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    await api.command("programmer.clear", {});
    await expectProgrammer(api, (state) => expect(state.values).toHaveLength(0));
    await expectSlotsAfterTick(bench, 0, Array(12).fill(0));
  });
});

async function loadCompactRig(api: ApiDriver, bench: any, name: string): Promise<void> {
  await loadCanonicalCopy(api, bench, name);
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
  if (commandLineRequiresLegacyCompatibility(value)) {
    await api.executeLegacyCommandLine(value);
  } else {
    await api.executeCommandLine(value);
  }
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

async function pressCommandAndWait(page: Page, value: string, visibleValue = value): Promise<void> {
  await pressCommand(page, value, visibleValue);
  await expect(page.getByLabel("Command line")).toHaveValue(/^(FIXTURE|GROUP)$/);
}

async function enterCommandWithoutEscape(page: Page, value: string, visibleValue = value): Promise<void> {
  const commandLine = page.getByLabel("Command line");
  for (const key of commandKeys(value)) await page.getByRole("button", { name: key, exact: true }).click();
  await expect(commandLine).toHaveValue(visibleValue);
  await page.getByRole("button", { name: "ENT", exact: true }).click();
  await expect(commandLine).toHaveValue("FIXTURE");
}

function commandKeys(value: string): string[] {
  return value.trim().split(/\s+/).flatMap((token) => {
    if (token === "GROUP") return ["GRP"];
    if (token === "DEGRP") return ["GRP", "GRP"];
    if (token === "THRU") return ["TRU"];
    if (token === "RECORD") return ["REC"];
    if (token === "DELETE") return ["DEL"];
    if (token === "DIV") return ["DIV"];
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

async function gestureFixture(api: ApiDriver, fixtureId: string, remove = false): Promise<void> {
  await api.command("selection.gesture", {
    source: { type: "fixture", fixture_id: fixtureId },
    remove,
  });
}

async function gestureGroup(api: ApiDriver, groupId: string, remove = false): Promise<void> {
  await api.command("selection.gesture", {
    source: { type: "live_group", group_id: groupId },
    remove,
  });
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

async function setGroupByNumbers(api: ApiDriver, id: string, name: string, numbers: number[]): Promise<void> {
  const byNumber = await fixtureIdsByNumber(api);
  const existing = (await objects(api, "group")).find((group) => group.id === id);
  await putObject(api, "group", id, {
    ...(existing?.body ?? {}),
    id,
    name,
    fixtures: numbers.map((number) => byNumber[number]),
    derived_from: null,
    frozen_from: null,
    programming: existing?.body.programming ?? {},
    master: existing?.body.master ?? 1,
    playback_fader: existing?.body.playback_fader ?? null,
  }, existing?.revision ?? 0);
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

function slotsFromFrame(frame: { universes: Array<{ universe: number; slots: number[] }> }, count: number): number[] {
  return (frame.universes.find((universe) => universe.universe === 1)?.slots ?? []).slice(0, count);
}

function normalized(value: { value?: number } | number | undefined): number | undefined {
  return typeof value === "number" ? value : value?.value;
}

async function openBuiltIn(page: Page, name: string): Promise<void> {
  const entry = page.locator(".dock-entry").filter({ hasText: name }).first();
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

async function openFixtures(page: Page): Promise<void> {
  await openBuiltIn(page, "Fixtures");
  await expect(page.locator(".fixture-window")).toBeVisible();
}

async function openPatch(page: Page): Promise<void> {
  if (await page.locator(".patch-table").isVisible()) return;
  await page.getByRole("button", { name: /Open show menu/ }).click();
  await page.getByRole("button", { name: "Show Patch", exact: true }).click();
  await expect(page.locator(".patch-table")).toBeVisible();
}

function patchFixtureRow(page: Page, number: number) {
  return page.locator(".patch-table tbody tr").filter({
    has: page.locator("td:nth-child(2)").filter({ hasText: new RegExp(`^${number}$`) }),
  }).first();
}

function groupCard(page: Page, number: number) {
  return page.locator(".group-pool-window .group-card").nth(number - 1);
}

async function recordExistingGroup(page: Page, number: number, mode: "Merge" | "Overwrite"): Promise<void> {
  await openGroups(page);
  await page.locator(".global-store-button").click();
  await expect(page.locator(".global-store-button")).toHaveText("REC ARMED");
  await groupCard(page, number).click();
  const dialog = page.locator(".record-mode-dialog");
  await expect(dialog).toBeVisible();
  await dialog.getByRole("button", { name: mode, exact: true }).click();
  await expect(dialog).toBeHidden();
}

async function expectVisibleGroupOrder(page: Page, number: number, fixtures: number[]): Promise<void> {
  await openGroups(page);
  const card = groupCard(page, number);
  const box = await card.boundingBox();
  expect(box).toBeTruthy();
  await page.mouse.move(box!.x + box!.width / 2, box!.y + box!.height / 2);
  await page.mouse.down();
  await page.waitForTimeout(700);
  await page.mouse.up();
  const order = page.locator(".group-context-menu .group-order");
  await expect(order).toBeVisible();
  for (const [index, fixture] of fixtures.entries())
    await expect(order).toContainText(`${index + 1}. Fixture ${fixture}`);
  await page.locator(".group-context-menu").getByRole("button", { name: "Cancel", exact: true }).click();
}

function fixtureRow(page: Page, number: number) {
  return page
    .locator(".fixture-window .ui-data-table-row:not(.header)")
    .filter({ has: page.getByRole("cell", { name: String(number), exact: true }) })
    .first();
}

async function selectFixtureRows(api: ApiDriver, page: Page, fixtures: number[]): Promise<void> {
  await openFixtures(page);
  for (const [index, fixture] of fixtures.entries()) {
    await fixtureRow(page, fixture).click();
    await expectSelectedNumbers(api, fixtures.slice(0, index + 1));
  }
}

function stageFixture(page: Page, fixtureId: string) {
  return page.locator(`.stage-fixture[data-fixture-id="${fixtureId}"]`);
}

async function setDimmerByTouch(page: Page, value: number): Promise<void> {
  const encoder = page.locator(".vertical-touch-fader-stack").filter({ hasText: "Enc 1 · Dimmer" });
  await encoder.getByRole("button", { name: "Set value" }).click();
  const dialog = page.getByRole("dialog", { name: "Enc 1 · Dimmer value" });
  await expect(dialog).toBeVisible();
  await page.keyboard.type(String(value));
  await page.keyboard.press("Enter");
  await expect(dialog).toBeHidden();
}
