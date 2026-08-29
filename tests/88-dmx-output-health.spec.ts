import { expect, test } from "./bench/core/fixtures";

test.setTimeout(180_000);
test.use({ viewport: { width: 1600, height: 1100 } });

/**
 * The DMX pane's output summary must report the live desk, not the state captured when the show
 * opened. The desk bootstrap carries `output_health`, but it is refetched only on a show change,
 * so the summary polls the runtime for its readings.
 */
test("DMX-HEALTH-001 @ui the DMX output summary reports live measured output health", async ({
	page,
	desk,
	api,
}) => {
	page.setDefaultTimeout(12_000);
	await desk.open(api.baseUrl);
	const toggle = page.getByRole("button", {
		name: "Desktops / Built-ins",
		exact: true,
	});
	if ((await toggle.getAttribute("data-dock-mode")) !== "desks")
		await toggle.click();
	await page.getByRole("button", { name: /New desktop/ }).click();
	await expect(page.locator(".empty-desk")).toBeVisible();
	await page.locator(".empty-desk").click({ position: { x: 10, y: 10 } });

	const dialog = page.getByRole("dialog", { name: "Open Window" });
	const card = dialog
		.getByRole("button")
		.filter({ has: page.getByText("DMX output", { exact: true }) });
	for (const tab of await dialog.getByRole("tab").all()) {
		await tab.click();
		if (await card.count()) {
			await card.first().click();
			break;
		}
	}

	const pane = page.locator(".desk-pane");
	await expect(pane).toBeVisible();
	const aside = pane.locator(".dmx-info-pane");
	await expect(aside).toContainText("Output summary");
	// The removed Packets readout carried no operator meaning and never moved.
	await expect(aside).not.toContainText("Packets");

	// The bench freezes the desk clock, so drive the production scheduler to deliver real frames.
	await api.request(
		"POST",
		"/api/v2/test/clock/free-run",
		{ millis: 3_000 },
		false,
	);

	const rate = aside.locator(".dmx-output-rate");
	await expect(rate).toContainText("Frame rate · last 60 s");
	await expect(rate.locator("dd").first()).not.toContainText("—", {
		timeout: 15_000,
	});
	const readings = await rate.locator("dd").allInnerTexts();
	const [minimum, average, maximumText] = readings;
	// The maximum is capped at the fastest rate the desk is asked about, so a faster measured
	// frame reads as an overflow instead of a number.
	const maximum =
		maximumText.trim() === "> 60 Hz" ? 60 : Number.parseFloat(maximumText);
	expect(Number.parseFloat(minimum)).toBeGreaterThan(0);
	expect(Number.parseFloat(minimum)).toBeLessThanOrEqual(
		Number.parseFloat(average),
	);
	expect(Number.parseFloat(average)).toBeLessThanOrEqual(maximum);

	const histogram = aside.locator(".dmx-output-histogram");
	await expect(histogram).toContainText("Frames at rate · last 60 s");
	expect(await histogram.locator("li small").allInnerTexts()).toEqual([
		"< 20 Hz",
		"20\u201330 Hz",
		"30\u201338 Hz",
		"38\u201340 Hz",
		"40\u201344 Hz",
		"44\u201348 Hz",
		"48\u201352 Hz",
		"52\u201356 Hz",
		"56\u201360 Hz",
		"> 60 Hz",
	]);
	// The bands are disjoint, so every delivered frame is counted exactly once.
	const counts = (await histogram.locator("li span").allInnerTexts()).map(
		Number,
	);
	for (const count of counts) {
		expect(count).toBeGreaterThanOrEqual(0);
	}
	expect(counts.reduce((total, count) => total + count, 0)).toBeGreaterThan(0);

	const errors = aside.locator(".dmx-output-errors");
	await expect(errors).toContainText("Last 60 s");
	await expect(errors).toContainText("Since show start");
});
