import type { Page } from "@playwright/test";
import { expect, test } from "./bench/core/fixtures";
import {
	expectProgrammer,
	selectedNumbers,
} from "./support/catalog";
import {
	readPatchSnapshot,
	unpatchFixture,
} from "./support/operator/patch";

test("FIXTURE-SELECTION-005 @ui › missing fixture IDs are skipped and Fixture Thru selects every existing fixture", async ({
	api,
	desk,
	page,
	show,
}) => {
	const initial = await readPatchSnapshot(api, show.id);
	const retained = initial.fixtures.filter((fixture) =>
		[10, 11, 12].includes(fixture.fixture_number),
	).sort((left, right) => left.fixture_number - right.fixture_number);
	expect(retained.map((fixture) => fixture.fixture_number)).toEqual([10, 11, 12]);
	await api.request(
		"POST",
		"/api/v2/patch/fixtures",
		{
			request_id: crypto.randomUUID(),
			fixtures: [],
			remove_fixture_ids: initial.fixtures
				.filter((fixture) => fixture.fixture_number < 10)
				.map((fixture) => fixture.fixture_id),
		},
		true,
		initial.patch_revision,
		{ showId: show.id },
	);
	await unpatchFixture(api, retained[2].fixture_id);

	await expect(api.executeCommandLine("FIXTURE 999")).resolves.toBeDefined();
	await expectProgrammer(api, (programmer) => expect(programmer.selected).toEqual([]));
	await api.executeCommandLine("FIXTURE 1 THRU 999");
	await expect.poll(() => selectedNumbers(api)).toEqual([10, 11, 12]);

	await desk.open(api.baseUrl);
	await executeVisibleCommand(page, "FIXTURE 999");
	await expect.poll(() => selectedNumbers(api)).toEqual([]);
	await expect(page.getByRole("alert")).toHaveCount(0);

	await executeVisibleCommand(page, "FIXTURE 1 THRU 999");
	await expect.poll(() => selectedNumbers(api)).toEqual([10, 11, 12]);

	await executeVisibleCommand(page, "FIXTURE 9 THRU 13");
	await expect.poll(() => selectedNumbers(api)).toEqual([10, 11, 12]);

	await executeVisibleCommand(page, "FIXTURE THRU");
	await expect.poll(() => selectedNumbers(api)).toEqual([10, 11, 12]);

	await executeVisibleCommand(page, "FIXTURE BANANA", false);
	const history = page.getByRole("dialog", { name: "Command line history" });
	await expect(history).toBeVisible();
	await expect(history.getByRole("alert")).toContainText(
		"fixture number is invalid",
	);
});

test("TL-164 @ui › a rejected THRU/PLUS entry cannot poison later Preload work", async ({
	api,
	desk,
	page,
	show,
}) => {
	const fixtureOne = (await readPatchSnapshot(api, show.id)).fixtures.find(
		(fixture) => fixture.fixture_number === 1,
	);
	if (!fixtureOne) throw new Error("The default show is missing Fixture 1");
	await desk.open(api.baseUrl);
	await page.getByRole("button", { name: "PRELOAD", exact: true }).click();

	await executeVisibleCommand(page, "FIXTURE 1 AT 25");
	const invalid = "FIXTURE 2 THRU 3 + 1 AT NOPE";
	await executeVisibleCommand(page, invalid, false);
	const input = page.getByRole("textbox", { name: "Command line" });
	await expect(input).toHaveValue(invalid);
	await expect(
		page
			.getByRole("dialog", { name: "Command line history" })
			.getByRole("alert"),
	).toContainText("level must be a percentage or FULL");

	await input.fill("AT 75");
	await input.press("Enter");
	await expect(input).toHaveClass(/completed/u);
	await expectProgrammer(api, (programmer) => {
		expect(programmer.preload_pending).toHaveLength(1);
		expect(programmer.preload_pending[0]).toMatchObject({
			fixture_id: fixtureOne.fixture_id,
			value: { kind: "normalized", value: 0.75 },
		});
	});

	await page.getByRole("button", { name: /^PRELOAD GO\b/u }).click();
	await expectProgrammer(api, (programmer) => {
		expect(programmer.preload_pending).toEqual([]);
		expect(programmer.preload_active).toHaveLength(1);
		expect(programmer.preload_active[0]).toMatchObject({
			fixture_id: fixtureOne.fixture_id,
			value: { kind: "normalized", value: 0.75 },
		});
	});
});

test("CLOCK-002 @ui › larger clock seconds stay inside the unchanged clock layout with and without attached hardware", async ({
	api,
	bench,
	desk,
	page,
}) => {
	await desk.open(api.baseUrl);
	const software = await clockGeometry(page);
	expect(software.secondsFontSize).toBe(9);
	expect(software.secondsInsideClock).toBe(true);

	const hardware = await bench.osc();
	const clientId = `clock-layout-${crypto.randomUUID()}`;
	try {
		await hardware.subscribe(clientId, api.session!.desk.osc_alias);
		await expect
			.poll(
				async () =>
					(await api.request<{ hardware_connected: boolean }>(
						"GET",
						"/api/v2/bootstrap",
						undefined,
						false,
					)).hardware_connected,
			)
			.toBe(true);
		const attached = await clockGeometry(page);
		expect(attached.secondsFontSize).toBe(9);
		expect(attached.secondsInsideClock).toBe(true);
		expect(attached.clockWidth).toBe(software.clockWidth);
		expect(attached.clockHeight).toBe(software.clockHeight);
	} finally {
		await hardware.send("/light/unsubscribe", [clientId]).catch(() => undefined);
		await hardware.close();
	}
});

async function executeVisibleCommand(
	page: Page,
	value: string,
	expectSuccess = true,
) {
	const input = page.getByRole("textbox", { name: "Command line" });
	if (await input.evaluate((element) => element.classList.contains("completed")))
		await page.locator(".command-escape").click();
	await input.fill(value);
	await input.press("Enter");
	if (expectSuccess) await expect(input).toHaveClass(/completed/u);
	else await expect(input).toHaveClass(/error/u);
}

async function clockGeometry(page: Page) {
	return page.locator(".dock-clock").evaluate((clock) => {
		const seconds = clock.querySelector<HTMLElement>(".dock-clock-seconds");
		if (!seconds) throw new Error("Clock seconds are missing");
		const clockBounds = clock.getBoundingClientRect();
		const secondsBounds = seconds.getBoundingClientRect();
		return {
			clockWidth: clockBounds.width,
			clockHeight: clockBounds.height,
			secondsFontSize: Number.parseFloat(getComputedStyle(seconds).fontSize),
			secondsInsideClock:
				secondsBounds.left >= clockBounds.left &&
				secondsBounds.right <= clockBounds.right &&
				secondsBounds.top >= clockBounds.top &&
				secondsBounds.bottom <= clockBounds.bottom,
		};
	});
}
