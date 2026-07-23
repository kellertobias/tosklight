import type { Page } from "../apps/control-ui/node_modules/@playwright/test/index.js";
import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import { expectProgrammer, fixtureIdsByNumber, loadCanonicalCopy, normalized, pressCommand } from "./support/catalog";

// PROG-002 surface parity (docs/plans/Next/50): the software-only layout's encoder
// value dialog shares the hardware modal's THRU submission path, so the same
// expression must land the same spread through the touch "Set value" dialog.
async function openSoftwareEncoderDialog(page: Page, encoderLabel: string) {
  const encoder = page.locator(".vertical-touch-fader-stack").filter({ hasText: encoderLabel });
  await encoder.getByRole("button", { name: "Set value" }).click();
  const dialog = page.getByRole("dialog", { name: `${encoderLabel} value`, exact: true });
  await expect(dialog).toBeVisible();
  return dialog;
}

async function typeDialogExpression(page: Page, dialog: ReturnType<Page["getByRole"]>, keys: string[], expression: string) {
  for (const key of keys) {
    await dialog.getByRole("button", { name: key, exact: true }).click();
  }
  await expect(dialog.locator("strong").first()).toHaveText(expression);
  await dialog.getByRole("button", { name: "ENTER", exact: true }).click();
  await expect(dialog).toBeHidden();
}

const MULTI_POINT_KEYS = ["1", "0", "0", "THRU", "0", "THRU", "1", "0", "0"];

test("PROG-002 @ui › software encoder value dialog spreads a two-point range over the ordered selection", async ({ api, bench, desk, page }) => {
  await loadCanonicalCopy(api, bench, "software-encoder-two-point-ui", "compact-rig");
  await desk.open(api.baseUrl);
  await pressCommand(page, "1 THRU 5", "F1 THRU 5");

  const dialog = await openSoftwareEncoderDialog(page, "Enc 1 · Dimmer");
  await typeDialogExpression(page, dialog, ["0", "THRU", "5", "0"], "0 THRU 50");

  const frame = await bench.tick(3_000);
  const universe = frame.universes.find((candidate: any) => candidate.universe === 1);
  expect(universe.slots.slice(0, 12)).toEqual([0, 32, 64, 96, 128, 0, 0, 0, 0, 0, 0, 0]);
  await expect(page.locator(".vertical-touch-fader-stack").filter({ hasText: "Enc 1 · Dimmer" })).toContainText("0%...50%");
});

test("PROG-002 @ui › software encoder value dialog lands a multi-point intensity spread once over the ordered selection", async ({ api, bench, desk, page }) => {
  await loadCanonicalCopy(api, bench, "software-encoder-multi-point-intensity-ui", "compact-rig");
  await desk.open(api.baseUrl);
  await pressCommand(page, "1 THRU 5", "F1 THRU 5");
  const fixtures = await fixtureIdsByNumber(api);

  const dialog = await openSoftwareEncoderDialog(page, "Enc 1 · Dimmer");
  await typeDialogExpression(page, dialog, MULTI_POINT_KEYS, "100 THRU 0 THRU 100");

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
  await expect(page.locator(".vertical-touch-fader-stack").filter({ hasText: "Enc 1 · Dimmer" })).toContainText("0%...100%");
});

test("PROG-002 @ui › software encoder value dialog spreads a multi-point Pan over the ordered moving-head selection", async ({ api, bench, desk, page }) => {
  await loadCanonicalCopy(api, bench, "software-encoder-multi-point-pan-ui", "default-stage");
  await desk.open(api.baseUrl);
  await pressCommand(page, "101 THRU 105", "F101 THRU 105");
  const fixtures = await fixtureIdsByNumber(api);

  await page.getByRole("button", { name: "Position" }).click();
  const dialog = await openSoftwareEncoderDialog(page, "Enc 1 · Pan");
  await typeDialogExpression(page, dialog, MULTI_POINT_KEYS, "100 THRU 0 THRU 100");

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
  await expect(page.locator(".vertical-touch-fader-stack").filter({ hasText: "Enc 1 · Pan" })).toContainText("0%...100%");
});
