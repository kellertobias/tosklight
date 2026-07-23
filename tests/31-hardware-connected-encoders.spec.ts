import type { Page } from "../apps/control-ui/node_modules/@playwright/test/index.js";
import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import { replaceProgrammingSelection } from "../apps/control-ui/e2e/bench/programmingSelection";
import { expectProgrammer, fixtureIdsByNumber, loadCanonicalCopy, normalized, pressCommand } from "./support/catalog";

test("ENCODER-DISPLAY-001 @supplemental-ui › six stable slots mirror physical encoder targets", async ({ api, bench, desk, page }) => {
  const show = await loadCanonicalCopy(api, bench, "encoder-display-001", "default-stage");
  const fixtures = await fixtureIdsByNumber(api);
  await replaceProgrammingSelection(api, {
    surface: "api",
    showId: show.id,
    fixtures: [fixtures[101]],
  });
  await desk.open(api.baseUrl);
  const hardware = await bench.osc();
  await hardware.subscribe(`encoder-display-${crypto.randomUUID()}`, api.session!.desk.osc_alias);
  try {
    await expect.poll(async () => (await api.request<any>("GET", "/api/v1/bootstrap", undefined, false)).hardware_connected).toBe(true);
    await page.getByRole("button", { name: "Position" }).click();
    const cards = page.locator(".hardware-encoder-display");
    await expect(cards).toHaveCount(6);
    await expect(cards.nth(0)).toContainText("Enc 1");
    await expect(cards.nth(0)).toContainText("Pan");
    await expect(cards.nth(1)).toContainText("Enc 2");
    await expect(cards.nth(1)).toContainText("Tilt");
    for (let index = 2; index < 6; index += 1) {
      await expect(cards.nth(index)).toContainText(`Enc ${index + 1}`);
      await expect(cards.nth(index)).toContainText("Unassigned");
      await expect(cards.nth(index)).not.toContainText("Not mapped");
    }
    await expect(page.locator(".parameter-surfaces").getByRole("slider")).toHaveCount(0);

    const headerPositions = await cards.nth(0).locator("header").evaluate((header) => {
      const headerBox = header.getBoundingClientRect();
      const labelBox = header.querySelector("b")!.getBoundingClientRect();
      const numberBox = header.querySelector("small")!.getBoundingClientRect();
      return {
        labelLeft: labelBox.left - headerBox.left,
        numberRight: headerBox.right - numberBox.right,
        verticalOffset: Math.abs(labelBox.top - numberBox.top),
      };
    });
    expect(headerPositions.labelLeft).toBeLessThan(2);
    expect(headerPositions.numberRight).toBeLessThan(2);
    expect(headerPositions.verticalOffset).toBeLessThan(3);
    const valuePosition = await cards.nth(0).locator(".hardware-encoder-target > strong").evaluate((value) => {
      const cardBox = value.closest(".hardware-encoder-display")!.getBoundingClientRect();
      const valueBox = value.getBoundingClientRect();
      return {
        horizontalOffset: Math.abs((valueBox.left + valueBox.width / 2) - (cardBox.left + cardBox.width / 2)),
        verticalOffset: Math.abs((valueBox.top + valueBox.height / 2) - (cardBox.top + cardBox.height / 2)),
      };
    });
    expect(valuePosition.horizontalOffset).toBeLessThan(2);
    expect(valuePosition.verticalOffset).toBeLessThan(2);
    expect(Number(await cards.nth(2).evaluate((element) => getComputedStyle(element).opacity))).toBeLessThan(.5);

    const boxes = await cards.evaluateAll((elements) => elements.map((element) => {
      const box = element.getBoundingClientRect();
      return { left: box.left, right: box.right, width: box.width, height: box.height };
    }));
    expect(boxes.every((box) => box.width >= 70 && box.height >= 80)).toBe(true);
    expect(boxes.every((box, index) => index === 0 || box.left >= boxes[index - 1].right)).toBe(true);

    const beforeText = await cards.nth(0).locator("strong").first().textContent();
    await hardware.send(`/light/${api.session!.desk.osc_alias}/encode/1`, ["up"]);
    await expect.poll(async () => cards.nth(0).locator("strong").first().textContent()).not.toBe(beforeText);

    await page.getByRole("button", { name: "Direct values and actions" }).click();
    await expect(cards).toHaveCount(6);
    for (let index = 0; index < 6; index += 1) await expect(cards.nth(index)).toContainText("Unassigned");
  } finally {
    await hardware.close();
  }
});

test("PROG-002 @ui › hardware encoder modal spreads a typed value over the ordered selection", async ({ api, bench, desk, page }) => {
  await loadCanonicalCopy(api, bench, "encoder-spread-ui", "compact-rig");
  await desk.open(api.baseUrl);
  await pressCommand(page, "1 THRU 5", "F1 THRU 5");
  const hardware = await bench.osc();
  await hardware.subscribe(`encoder-spread-${crypto.randomUUID()}`, api.session!.desk.osc_alias);
  try {
    await expect.poll(async () => (await api.request<any>("GET", "/api/v1/bootstrap", undefined, false)).hardware_connected).toBe(true);

    const dimmer = page.getByRole("button", { name: "Encoder 1: Dimmer, 0%", exact: true });
    await dimmer.click();
    const dialog = page.getByRole("dialog", { name: "Encoder 1 value", exact: true });
    for (const key of ["0", "THRU", "5", "0", "ENTER"]) {
      await dialog.getByRole("button", { name: key, exact: true }).click();
    }

    const frame = await bench.tick(3_000);
    const universe = frame.universes.find((candidate: any) => candidate.universe === 1);
    expect(universe.slots.slice(0, 12)).toEqual([0, 32, 64, 96, 128, 0, 0, 0, 0, 0, 0, 0]);
    await expect(page.locator(".hardware-encoder-display").filter({ hasText: "Dimmer" })).toContainText("0%...50%");
  } finally {
    await hardware.close();
  }
});

// Deterministic multi-point anchor rule (docs/plans/Next/50): repeated THRU through the
// production hardware-connected encoder value modal must land the normative five-item
// resolution exactly once as per-fixture programmer values.
async function typeEncoderModalExpression(page: Page, expression: string[]): Promise<void> {
  const dialog = page.getByRole("dialog", { name: "Encoder 1 value", exact: true });
  for (const key of expression) {
    await dialog.getByRole("button", { name: key, exact: true }).click();
  }
  await expect(dialog.locator("strong")).toHaveText("100 THRU 0 THRU 100");
  await dialog.getByRole("button", { name: "ENTER", exact: true }).click();
  await expect(dialog).toBeHidden();
}

const MULTI_POINT_KEYS = ["1", "0", "0", "THRU", "0", "THRU", "1", "0", "0"];

test("PROG-002 @ui › hardware encoder modal lands a multi-point intensity spread once over the ordered selection", async ({ api, bench, desk, page }) => {
  await loadCanonicalCopy(api, bench, "encoder-multi-point-intensity-ui", "compact-rig");
  await desk.open(api.baseUrl);
  await pressCommand(page, "1 THRU 5", "F1 THRU 5");
  const fixtures = await fixtureIdsByNumber(api);
  const hardware = await bench.osc();
  await hardware.subscribe(`encoder-multi-point-${crypto.randomUUID()}`, api.session!.desk.osc_alias);
  try {
    await expect.poll(async () => (await api.request<any>("GET", "/api/v1/bootstrap", undefined, false)).hardware_connected).toBe(true);

    await page.getByRole("button", { name: "Encoder 1: Dimmer, 0%", exact: true }).click();
    await typeEncoderModalExpression(page, MULTI_POINT_KEYS);

    // One atomic mutation: exactly one resolved value per selected fixture, nothing else.
    await expectProgrammer(api, (state) => {
      expect(state.values).toHaveLength(5);
      expect(Object.keys(state.group_values)).toHaveLength(0);
      const byFixture = [1, 2, 3, 4, 5].map((number) =>
        state.values.filter((value) => value.fixture_id === fixtures[number] && value.attribute === "intensity"));
      expect(byFixture.map((entries) => entries.length)).toEqual([1, 1, 1, 1, 1]);
      expect(byFixture.map((entries) => normalized(entries[0].value))).toEqual([1, 0.5, 0, 0.5, 1]);
    });

    const frame = await bench.tick(3_000);
    const universe = frame.universes.find((candidate: any) => candidate.universe === 1);
    expect(universe.slots.slice(0, 12)).toEqual([255, 128, 0, 128, 255, 0, 0, 0, 0, 0, 0, 0]);
    await expect(page.locator(".hardware-encoder-display").filter({ hasText: "Dimmer" })).toContainText("0%...100%");
  } finally {
    await hardware.close();
  }
});

test("PROG-002 @ui › hardware encoder modal spreads a multi-point Pan over the ordered moving-head selection", async ({ api, bench, desk, page }) => {
  await loadCanonicalCopy(api, bench, "encoder-multi-point-pan-ui", "default-stage");
  await desk.open(api.baseUrl);
  await pressCommand(page, "101 THRU 105", "F101 THRU 105");
  const fixtures = await fixtureIdsByNumber(api);
  const hardware = await bench.osc();
  await hardware.subscribe(`encoder-multi-point-pan-${crypto.randomUUID()}`, api.session!.desk.osc_alias);
  try {
    await expect.poll(async () => (await api.request<any>("GET", "/api/v1/bootstrap", undefined, false)).hardware_connected).toBe(true);

    await page.getByRole("button", { name: "Position" }).click();
    await page.getByRole("button", { name: /^Encoder 1: Pan,/ }).click();
    await typeEncoderModalExpression(page, MULTI_POINT_KEYS);

    await expectProgrammer(api, (state) => {
      expect(state.values).toHaveLength(5);
      expect(Object.keys(state.group_values)).toHaveLength(0);
      const byFixture = [101, 102, 103, 104, 105].map((number) =>
        state.values.filter((value) => value.fixture_id === fixtures[number] && value.attribute === "pan"));
      expect(byFixture.map((entries) => entries.length)).toEqual([1, 1, 1, 1, 1]);
      expect(byFixture.map((entries) => normalized(entries[0].value))).toEqual([1, 0.5, 0, 0.5, 1]);
    });

    // Back Profile heads: universe 2, 6 channels each from address 1; pan is channel 2.
    const frame = await bench.tick(3_000);
    const slots = frame.universes.find((candidate: any) => candidate.universe === 2)!.slots;
    expect([slots[1], slots[7], slots[13], slots[19], slots[25]]).toEqual([255, 128, 0, 128, 255]);
    await expect(page.locator(".hardware-encoder-display").filter({ hasText: "Pan" })).toContainText("0%...100%");
  } finally {
    await hardware.close();
  }
});
