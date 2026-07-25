import { expect, test } from "../apps/control-ui/e2e/bench/core/fixtures";
import { replaceProgrammingSelection } from "../apps/control-ui/e2e/bench/command-selection/programmingSelection";
import { fixtureIdsByNumber, loadCanonicalCopy } from "./support/catalog";

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
    await expect.poll(async () => (await api.request<any>("GET", "/api/v2/bootstrap", undefined, false)).hardware_connected).toBe(true);
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

    const headerPositions = await cards
      .nth(0)
      .locator("header")
      .evaluate((header) => {
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
    const valuePosition = await cards
      .nth(0)
      .locator(".hardware-encoder-target > strong")
      .evaluate((value) => {
        const cardBox = value.closest(".hardware-encoder-display")!.getBoundingClientRect();
        const valueBox = value.getBoundingClientRect();
        return {
          horizontalOffset: Math.abs(valueBox.left + valueBox.width / 2 - (cardBox.left + cardBox.width / 2)),
          verticalOffset: Math.abs(valueBox.top + valueBox.height / 2 - (cardBox.top + cardBox.height / 2)),
        };
      });
    expect(valuePosition.horizontalOffset).toBeLessThan(2);
    expect(valuePosition.verticalOffset).toBeLessThan(2);
    expect(Number(await cards.nth(2).evaluate((element) => getComputedStyle(element).opacity))).toBeLessThan(0.5);

    const boxes = await cards.evaluateAll((elements) =>
      elements.map((element) => {
        const box = element.getBoundingClientRect();
        return {
          left: box.left,
          right: box.right,
          width: box.width,
          height: box.height,
        };
      }),
    );
    expect(boxes.every((box) => box.width >= 70 && box.height >= 80)).toBe(true);
    expect(boxes.every((box, index) => index === 0 || box.left >= boxes[index - 1].right)).toBe(true);

    const beforeText = await cards.nth(0).locator("strong").first().textContent();
    await hardware.send(`/light/${api.session!.desk.osc_alias}/encode/1`, ["up"]);
    await expect.poll(async () => cards.nth(0).locator("strong").first().textContent()).not.toBe(beforeText);

    await expect(cards).toHaveCount(6);
    await expect(cards.nth(0)).toContainText("Pan");
    await expect(cards.nth(1)).toContainText("Tilt");
  } finally {
    await hardware.close();
  }
});
