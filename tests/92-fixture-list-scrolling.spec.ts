import type { Page } from "@playwright/test";
import { expect, test } from "./bench/core/fixtures";

/**
 * The fixture list virtualizes above one hundred rows, and a virtualized table re-asserts its
 * active row whenever the mounted row window changes. A wheel gesture changes that window, so
 * the re-assert used to fight the operator: the list travelled a little and snapped back.
 */
test("TL-396 @ui › the fixture list stays where the wheel leaves it", async ({
	api,
	bench,
	desk,
	page,
}) => {
	await patchEnoughFixturesToVirtualize(api);

	await desk.open(bench.baseUrl);
	await openFixtureSheet(page);

	const table = page.locator(".ui-data-table.virtualized");
	await expect(table, "the list is virtualized").toHaveCount(1);

	// An operator selects a fixture before scrolling, which is what gives the table focus and
	// arms the active-row behavior this test pins.
	await page.locator(".ui-data-table [data-table-index]").first().click();
	await page.waitForTimeout(400);

	const scroller = page.locator(".ui-window-scroller").last();
	const box = await scroller.boundingBox();
	expect(box, "the fixture list is on screen").toBeTruthy();
	if (!box) return;
	await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);

	const scrollTop = () => scroller.evaluate((el: HTMLElement) => el.scrollTop);
	expect(await scrollTop()).toBe(0);

	const wheel = 240;
	await page.mouse.wheel(0, wheel);
	// Long enough for a re-assert to have landed: the snap-back is not instantaneous.
	await page.waitForTimeout(1200);

	// The whole defect is arriving short of where the wheel asked for, so assert the distance
	// travelled rather than merely that something moved.
	expect(
		await scrollTop(),
		"the list holds the position the wheel scrolled to",
	).toBeGreaterThanOrEqual(wheel - 8);
});

/** Virtualization only engages above one hundred rows, which no bench show reaches on its own. */
async function patchEnoughFixturesToVirtualize(api: {
	request<T>(
		method: string,
		path: string,
		body?: unknown,
		authenticate?: boolean,
		revision?: number,
	): Promise<T>;
}): Promise<void> {
	const library = await api.request<{ profiles: Array<Record<string, any>> }>(
		"GET",
		"/api/v2/fixture-library/profiles",
	);
	const profile = library.profiles.find((candidate) =>
		/dimmer/iu.test(String(candidate.name ?? "")),
	);
	expect(profile, "a shipped dimmer profile is loaded").toBeTruthy();
	const mode = profile?.modes?.[0];
	expect(mode, "the dimmer has a mode").toBeTruthy();
	const snapshot = await api.request<{ patch_revision: number }>(
		"GET",
		"/api/v2/patch",
	);
	await api.request(
		"POST",
		"/api/v2/patch/fixtures",
		{
			request_id: crypto.randomUUID(),
			fixtures: Array.from({ length: 160 }, (_, index) => ({
				fixture_id: crypto.randomUUID(),
				fixture_number: 2000 + index,
				virtual_fixture_number: null,
				name: `Scroll probe ${index}`,
				profile_id: profile.id,
				profile_revision: profile.revision,
				mode_id: mode.id,
				split_patches: [{ split: 1, universe: null, address: null }],
				layer_id: "default",
				direct_control: null,
				location: { x: 0, y: 0, z: 0 },
				rotation: { x: 0, y: 0, z: 0 },
				multipatch: [],
				move_in_black_enabled: false,
				move_in_black_delay_millis: 0,
				highlight_overrides: [],
			})),
			remove_fixture_ids: [],
		},
		true,
		snapshot.patch_revision,
	);
}

/** The Open Window catalog groups its cards into tabs, so the bench searches the tabs. */
async function openFixtureSheet(page: Page): Promise<void> {
	const dockMode = page.getByRole("button", {
		name: "Desktops / Built-ins",
		exact: true,
	});
	if ((await dockMode.getAttribute("data-dock-mode")) !== "desks") {
		await dockMode.click();
	}
	await page.getByRole("button", { name: /New desktop/ }).click();
	await page.locator(".empty-desk").click({ position: { x: 10, y: 10 } });
	const dialog = page.getByRole("dialog", { name: "Open Window" });
	const card = dialog
		.getByRole("button")
		.filter({ has: page.getByText("Fixture sheet", { exact: true }) });
	for (const tab of await dialog.getByRole("tab").all()) {
		await tab.click();
		if (await card.count()) {
			await card.first().click();
			await page.waitForTimeout(1500);
			return;
		}
	}
	throw new Error("The Open Window catalog offers no Fixture sheet");
}
