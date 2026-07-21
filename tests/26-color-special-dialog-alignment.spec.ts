import type { Page } from "../apps/control-ui/node_modules/@playwright/test/index.js";
import { expect } from "../apps/control-ui/e2e/bench/fixtures";
import { pairedScenario } from "../apps/control-ui/e2e/bench/pairedScenario";
import { batchProgrammerValues } from "../apps/control-ui/e2e/bench/programmerValues";
import { replaceProgrammingSelection } from "../apps/control-ui/e2e/bench/programmingSelection";
import {
  colorProgrammerAssignments,
  interpolatePickerRange,
  type PickerColor,
} from "../apps/control-ui/src/components/modals/specialColor";
import { loadCanonicalCopy, programmer } from "./support/catalog";

type Assignment = { fixtureId: string; attribute: string; value: number };
type ColorRangeState = {
  showId: string;
  selected: string[];
  range: Assignment[];
  prior: Assignment[];
};

const start: PickerColor = { hue: 0.1, saturation: 0.8, brightness: 0.85 };
const end: PickerColor = { hue: 0.9, saturation: 0.2, brightness: 0.85 };

pairedScenario<ColorRangeState>({
  id: "COLOR-RANGE-001",
  title: "Shift-drag applies an ordered Color range from software and attached hardware",
  arrange: async ({ api, bench }, surface) => {
    const show = await loadCanonicalCopy(api, bench, `color-range-001-${surface}`, "default-stage");
    const patch = await api.request<any>("GET", "/api/v1/patch", undefined, false);
    const colorTargets = patch.fixtures.flatMap((fixture: any) => {
      const logicalByIndex = new Map(
        fixture.logical_heads.map((head: any) => [head.head_index, head.fixture_id]),
      );
      return fixture.definition.heads.flatMap((head: any) => {
        const fixtureId = head.shared ? fixture.fixture_id : logicalByIndex.get(head.index);
        const attributes = new Set(head.parameters.map((parameter: any) => parameter.attribute));
        const supported = ["color.red", "color.green", "color.blue", "color.cyan", "color.magenta", "color.yellow"]
          .some((attribute) => attributes.has(attribute));
        return fixtureId && supported
          ? [{ fixtureId: fixtureId as string, logical: !head.shared }]
          : [];
      });
    });
    expect(colorTargets.length).toBeGreaterThanOrEqual(3);
    const logical = colorTargets.find((target: any) => target.logical);
    const chosen = [logical ?? colorTargets[2], ...colorTargets.filter((target: any) => target.fixtureId !== logical?.fixtureId).slice(0, 2)];
    const nonColor = patch.fixtures.find((fixture: any) =>
      fixture.definition.heads.every((head: any) =>
        head.parameters.every((parameter: any) => !parameter.attribute.startsWith("color.")),
      ),
    );
    const selected = [
      chosen[0].fixtureId,
      chosen[1].fixtureId,
      ...(nonColor ? [nonColor.fixture_id] : []),
      chosen[2].fixtureId,
    ];
    const range = colorProgrammerAssignments(
      selected,
      patch.fixtures,
      interpolatePickerRange(selected.length, start, end),
    );
    const prior = range.map((assignment) => ({ ...assignment, value: 0.33 }));
    await replaceProgrammingSelection(api, {
      surface: "api",
      showId: show.id,
      fixtures: selected,
    });
    await setMany(api, show.id, prior);
    return { showId: show.id, selected, range, prior };
  },
  api: async ({ api }, state) => {
    await setMany(api, state.showId, state.range);
  },
  ui: async ({ api, bench, desk, page }, state) => {
    await desk.open(api.baseUrl);
    await page.getByRole("button", { name: "Color", exact: true }).click();
    await openColorDialog(page);

    const uniformPoint = { hue: 0.35, saturation: 0.6, brightness: 0.85 };
    const uniform = colorProgrammerAssignments(
      state.selected,
      (await api.request<any>("GET", "/api/v1/patch", undefined, false)).fixtures,
      state.selected.map(() => uniformPoint),
    );
    const beforeUniform = await batchCommandCount(api);
    await clickPicker(page, uniformPoint.hue, 1 - uniformPoint.saturation);
    await expect.poll(() => batchCommandCount(api)).toBe(beforeUniform + 1);
    await expectAssignments(api, uniform);
    await closeAndUndo(page, api, state.prior);

    await openColorDialog(page);
    const beforeSoftwareRange = await batchCommandCount(api);
    await page.keyboard.down("Shift");
    await beginPickerDrag(page, start.hue, 1 - start.saturation, end.hue, 1 - end.saturation);
    await expect(page.locator('.color-range-preview[data-active="true"]')).toBeVisible();
    expect(await batchCommandCount(api)).toBe(beforeSoftwareRange);
    await page.mouse.up();
    await page.keyboard.up("Shift");
    await expect.poll(() => batchCommandCount(api)).toBe(beforeSoftwareRange + 1);
    await expect(page.locator('.color-range-preview[data-active="false"]')).toBeVisible();
    await expectAssignments(api, state.range);
    await closeAndUndo(page, api, state.prior);

    await openColorDialog(page);
    const beforeCancelledRange = await batchCommandCount(api);
    await page.keyboard.down("Shift");
    await beginPickerDrag(page, start.hue, 1 - start.saturation, end.hue, 1 - end.saturation);
    await expect(page.locator('.color-range-preview[data-active="true"]')).toBeVisible();
    await page.locator(".color-sheet").dispatchEvent("pointercancel", {
      pointerId: 1,
      pointerType: "mouse",
    });
    await page.keyboard.up("Shift");
    await page.mouse.up();
    await expect(page.locator(".color-range-preview")).toHaveCount(0);
    expect(await batchCommandCount(api)).toBe(beforeCancelledRange);
    await page.locator(".modal-close").click();

    const hardware = await bench.osc();
    const clientId = `color-range-${crypto.randomUUID()}`;
    try {
      await hardware.subscribe(clientId, api.session!.desk.osc_alias);
      await expect.poll(async () =>
        (await api.request<any>("GET", "/api/v1/bootstrap", undefined, false)).hardware_connected,
      ).toBe(true);
      await openColorDialog(page);
      await hardware.send(`/light/${api.session!.desk.osc_alias}/programmer/shift`, [true]);
      await expect(page.locator('.color-sheet[data-range-shift="armed"]')).toBeVisible();
      const beforeHardwareRange = await batchCommandCount(api);
      await beginPickerDrag(page, start.hue, 1 - start.saturation, end.hue, 1 - end.saturation);
      expect(await batchCommandCount(api)).toBe(beforeHardwareRange);
      await page.mouse.up();
      await expect.poll(() => batchCommandCount(api)).toBe(beforeHardwareRange + 1);
      await expectAssignments(api, state.range);
      await expect(page.locator('.color-sheet[data-range-shift="armed"]')).toBeVisible();
      await hardware.send(`/light/${api.session!.desk.osc_alias}/programmer/shift`, [false]);
      await expect(page.locator('.color-sheet[data-range-shift="idle"]')).toBeVisible();
    } finally {
      await hardware.send(`/light/${api.session!.desk.osc_alias}/programmer/shift`, [false]).catch(() => undefined);
      await hardware.send("/light/unsubscribe", [clientId]).catch(() => undefined);
      await hardware.close();
    }
  },
  assert: async ({ api }, state) => {
    expect((await programmer(api)).selected).toEqual(state.selected);
    await expectAssignments(api, state.range);
  },
});

async function openColorDialog(page: Page): Promise<void> {
  await page.getByRole("button", { name: "Special Dialog", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Color · Special Dialog" })).toBeVisible();
}

async function clickPicker(page: Page, x: number, y: number): Promise<void> {
  const box = await page.locator(".color-sheet").boundingBox();
  expect(box).toBeTruthy();
  await page.mouse.click(box!.x + box!.width * x, box!.y + box!.height * y);
}

async function beginPickerDrag(
  page: Page,
  startX: number,
  startY: number,
  endX: number,
  endY: number,
): Promise<void> {
  const box = await page.locator(".color-sheet").boundingBox();
  expect(box).toBeTruthy();
  await page.mouse.move(box!.x + box!.width * startX, box!.y + box!.height * startY);
  await page.mouse.down();
  await page.mouse.move(box!.x + box!.width * endX, box!.y + box!.height * endY, { steps: 5 });
}

async function closeAndUndo(page: Page, api: any, expected: Assignment[]): Promise<void> {
  await page.locator(".modal-close").click();
  await page.getByRole("button", { name: "UND", exact: true }).click();
  await expectAssignments(api, expected);
}

async function setMany(api: any, showId: string, assignments: Assignment[]): Promise<void> {
  await batchProgrammerValues(api, {
    surface: "api",
    showId,
    mutations: assignments.map(({ fixtureId, attribute, value }) => ({
      action: "set_fixture",
      fixtureId,
      attribute,
      value: { kind: "normalized", value },
      timing: { fade: true, fadeMillis: 3_000, delayMillis: null },
    })),
  });
}

async function expectAssignments(api: any, expected: Assignment[]): Promise<void> {
  await expect.poll(async () => {
    const values = (await programmer(api)).values;
    return expected.every((assignment) => {
      const actual = values.find((value) =>
        value.fixture_id === assignment.fixtureId && value.attribute === assignment.attribute,
      );
      const value = actual?.value?.value ?? actual?.value;
      return typeof value === "number" && Math.abs(value - assignment.value) < 0.00001;
    });
  }).toBe(true);
}

async function batchCommandCount(api: any): Promise<number> {
  const audit = await api.request<any[]>("GET", "/api/v1/audit?after=0");
  return audit.filter((event) => event.kind === "programmer_changed").length;
}
