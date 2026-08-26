import type { Page } from "@playwright/test";
import { expect, test } from "./bench/core/fixtures";

test.describe("Channels disabled-state explanations", () => {
	test("TL-166 @ui › empty faders explain their state while available faders stay clear", async ({
		bench,
		desk,
		page,
	}) => {
		await desk.open(bench.baseUrl);
		const dockMode = page.getByRole("button", {
			name: "Desktops / Built-ins",
			exact: true,
		});
		if ((await dockMode.getAttribute("data-dock-mode")) !== "desks") {
			await dockMode.click();
		}
		await page.getByRole("button", { name: /New desktop/ }).click();
		await page.locator(".empty-desk").click({ position: { x: 10, y: 10 } });
		await openCatalogCard(page, "Channels");

		const channels = page.locator(".channels-window");
		await expect(channels).toBeVisible();
		const populated = channels.locator(".channel-fader:not(.empty)").first();
		await expect(populated.locator('input[type="range"]')).toBeEnabled();
		await expect(populated).toContainText("Intensity");
		await expect(populated).not.toContainText(/loading|unavailable|inactive/iu);

		const empty = channels.locator(".channel-fader.empty").first();
		await expect(empty.locator('input[type="range"]')).toBeDisabled();
		await expect(empty).toContainText("Empty position");
	});

	test("TL-373 @ui › channel faders are labelled with the fixture name, not the Fixture ID", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		await desk.open(bench.baseUrl);
		const dockMode = page.getByRole("button", {
			name: "Desktops / Built-ins",
			exact: true,
		});
		if ((await dockMode.getAttribute("data-dock-mode")) !== "desks") {
			await dockMode.click();
		}
		await page.getByRole("button", { name: /New desktop/ }).click();
		await page.locator(".empty-desk").click({ position: { x: 10, y: 10 } });
		await openCatalogCard(page, "Channels");

		const channels = page.locator(".channels-window");
		await expect(channels).toBeVisible();

		const patch = await api.patch();
		const patched = patch.fixtures
			.filter((fixture) => fixture.fixture_number != null)
			.sort(
				(left, right) =>
					(left.fixture_number ?? 0) - (right.fixture_number ?? 0),
			);
		expect(patched.length).toBeGreaterThan(0);
		const first = patched[0];
		const expectedName = first.name?.trim() || first.definition.name.trim();
		expect(expectedName).not.toBe("");

		const populated = channels.locator(".channel-fader:not(.empty)").first();
		// The name identifies the fixture; the ID stays available for command-line addressing.
		await expect(populated).toContainText(expectedName);
		await expect(populated.locator(".channel-fader-id")).toHaveText(
			String(first.fixture_number),
		);
		// The old label was the bare "Fixture <id>" with no name at all.
		await expect(populated).not.toContainText(
			`Fixture ${first.fixture_number}`,
		);
	});

	test("TL-392 @ui › a stage element with no dimmer never reaches the fader bank", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		const trussName = `Downstage truss ${crypto.randomUUID().slice(0, 8)}`;
		await patchStageElement(api, trussName);

		await desk.open(bench.baseUrl);
		const dockMode = page.getByRole("button", {
			name: "Desktops / Built-ins",
			exact: true,
		});
		if ((await dockMode.getAttribute("data-dock-mode")) !== "desks") {
			await dockMode.click();
		}
		await page.getByRole("button", { name: /New desktop/ }).click();
		await page.locator(".empty-desk").click({ position: { x: 10, y: 10 } });
		await openCatalogCard(page, "Channels");

		const channels = page.locator(".channels-window");
		await expect(channels).toBeVisible();
		// The bank has to have rendered before its absence means anything.
		await expect(channels.locator(".channel-fader").first()).toBeVisible();

		// A truss owns neither a dimmer channel nor a virtual dimmer, so no page of the bank
		// can offer a fader for it.
		const pages = await channels.getByRole("button", { name: /Page \d+/ }).all();
		await expect(channels).not.toContainText(trussName);
		for (const pageButton of pages) {
			await pageButton.click();
			await expect(channels).not.toContainText(trussName);
		}

		// It is left off the bank, not removed from the show.
		const patch = await api.patch();
		expect(
			patch.fixtures.some((fixture) => fixture.name === trussName),
			"the truss is still patched",
		).toBe(true);
	});
});

/**
 * Patches one shipped venue truss: a stage element with no intensity channel and no emitter that
 * reacts to virtual intensity.
 */
async function patchStageElement(
	api: {
		patch(): Promise<{ fixtures: Array<Record<string, any>> }>;
		request<T>(
			method: string,
			path: string,
			body?: unknown,
			authenticate?: boolean,
			revision?: number,
		): Promise<T>;
	},
	name: string,
): Promise<void> {
	const library = await api.request<{ profiles: Array<Record<string, any>> }>(
		"GET",
		"/api/v2/fixture-library/profiles",
	);
	const profile = library.profiles.find((candidate) =>
		/truss/iu.test(String(candidate.name ?? "")),
	);
	expect(profile, "a shipped truss profile is loaded").toBeTruthy();
	const mode = profile?.modes?.[0];
	expect(mode, "the truss has a mode").toBeTruthy();
	const snapshot = await api.request<{ patch_revision: number }>(
		"GET",
		"/api/v2/patch",
	);
	await api.request(
		"POST",
		"/api/v2/patch/fixtures",
		{
			request_id: crypto.randomUUID(),
			fixtures: [
				{
					fixture_id: crypto.randomUUID(),
					fixture_number: 902,
					virtual_fixture_number: null,
					name,
					profile_id: profile.id,
					profile_revision: profile.revision,
					mode_id: mode.id,
					// A stage element owns no DMX address.
					split_patches: [{ split: 1, universe: null, address: null }],
					layer_id: "default",
					direct_control: null,
					location: { x: 0, y: 0, z: 0 },
					rotation: { x: 0, y: 0, z: 0 },
					multipatch: [],
					move_in_black_enabled: false,
					move_in_black_delay_millis: 0,
					highlight_overrides: [],
				},
			],
			remove_fixture_ids: [],
		},
		true,
		snapshot.patch_revision,
	);
}

/** The Open Window catalog groups its cards into tabs, so the bench searches the tabs. */
async function openCatalogCard(page: Page, title: string): Promise<void> {
	const dialog = page.getByRole("dialog", { name: "Open Window" });
	const card = dialog
		.getByRole("button")
		.filter({ has: page.getByText(title, { exact: true }) });
	for (const tab of await dialog.getByRole("tab").all()) {
		await tab.click();
		if (await card.count()) {
			await card.first().click();
			return;
		}
	}
	throw new Error(`The Open Window catalog offers no "${title}" window`);
}
