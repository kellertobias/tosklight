import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import { fixtureIdsByNumber, loadCanonicalCopy } from "./support/catalog";

test("ENCODER-DISPLAY-001 @supplemental-ui › six stable slots mirror physical encoder targets", async ({ api, bench, desk, page }) => {
  await loadCanonicalCopy(api, bench, "encoder-display-001", "default-stage");
  const fixtures = await fixtureIdsByNumber(api);
  await api.command("selection.set", { fixtures: [fixtures[101]] });
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
    }
    await expect(page.locator(".parameter-surfaces").getByRole("slider")).toHaveCount(0);

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
