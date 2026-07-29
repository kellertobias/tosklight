import { type BenchUiContext, expect, test } from "../bench/core/fixtures";
import {
	activeVirtualPane,
	activeVirtualPlayback,
	addVirtualPlaybackPane,
	prepare,
	virtualZoneSnapshot,
	writeVirtualPage,
} from "./support";

const virtualZoneUiSupplement = async ({
	api,
	bench,
	desk,
	page,
}: BenchUiContext) => {
	await prepare(
		api,
		bench,
		"vpb-007-ui",
		[
			{ number: 74, fixture: 3, levels: [0.25], name: "Touring A" },
			{ number: 75, fixture: 4, levels: [0.5], name: "Touring B" },
			{ number: 76, fixture: 5, levels: [0.75], name: "Touring C" },
		],
		{},
	);
	await writeVirtualPage(api, 1, { 1001: 74, 1002: 75, 1003: 76 });
	await desk.open(bench.baseUrl);
	let pane = await addVirtualPlaybackPane(page);
	await desk.recordStep(
		"SELECT EXCLUSION MEMBERS",
		"Hold Shift and choose cells 1 and 2. Selection must not operate either playback.",
	);
	await page.keyboard.down("Shift");
	await pane
		.getByRole("button", { name: /Virtual playback page 1 cell 1 Touring A/ })
		.click();
	await pane
		.getByRole("button", { name: /Virtual playback page 1 cell 2 Touring B/ })
		.click();
	await page.keyboard.up("Shift");
	expect(await activeVirtualPlayback(api, 1, 1001)).toBeUndefined();
	expect(await activeVirtualPlayback(api, 1, 1002)).toBeUndefined();
	await pane.getByRole("button", { name: "Create Exclusion Zone" }).click();
	const create = page.getByRole("dialog", { name: "Create Exclusion Zone" });
	await create.getByLabel("Zone name").fill("Touring pair");
	await create.getByRole("button", { name: "Create zone" }).click();
	await expect(create).toBeHidden();
	await expect
		.poll(async () => {
			const surfaces = Object.values(
				(await virtualZoneSnapshot(api)).desks[api.session!.desk.id] ?? {},
			);
			return surfaces.find((surface) =>
				surface.zones.some((zone) => zone.name === "Touring pair"),
			);
		})
		.toMatchObject({
			revision: 1,
			page_mode: { type: "follow_main" },
		});

	await desk.recordStep(
		"NEW ACTIVATION WINS",
		"Turn on cell 1, then cell 2. Cell 2 remains On and cell 1 is released by the server.",
	);
	await pane
		.getByRole("button", { name: /Virtual playback page 1 cell 1 Touring A/ })
		.click();
	await pane
		.getByRole("button", { name: /Virtual playback page 1 cell 2 Touring B/ })
		.click();
	await expect
		.poll(async () => (await activeVirtualPlayback(api, 1, 1001))?.enabled)
		.toBe(false);
	await expect
		.poll(async () => (await activeVirtualPlayback(api, 1, 1002))?.enabled)
		.toBe(true);

	await pane.getByRole("button", { name: "Settings", exact: true }).click();
	let settings = page.getByRole("dialog", { name: "Pane Settings" });
	await settings
		.getByRole("tab", { name: "Virtual Playbacks", exact: true })
		.click();
	await settings.getByRole("radio", { name: "Pinned", exact: true }).click();
	await settings.getByLabel("Pinned page").fill("2");
	await settings
		.getByRole("tab", { name: "Exclusion Zones", exact: true })
		.click();
	await settings.getByLabel("Name for Touring pair").fill("Touring alternates");
	await settings.getByRole("button", { name: "Save name" }).click();
	await settings.getByRole("button", { name: "Edit Zone" }).click();
	await expect(settings).toBeHidden();
	await pane
		.getByRole("button", { name: /Virtual playback page 2 cell 3 empty/ })
		.click();
	await pane
		.getByRole("button", { name: "Update Exclusion Zone", exact: true })
		.click();
	await pane.getByRole("button", { name: "Settings", exact: true }).click();
	settings = page.getByRole("dialog", { name: "Pane Settings" });
	await settings
		.getByRole("tab", { name: "Virtual Playbacks", exact: true })
		.click();
	await settings.getByLabel("Rows").fill("1");
	await settings.getByLabel("Columns").fill("2");
	await settings
		.getByRole("tab", { name: "Exclusion Zones", exact: true })
		.click();
	await expect(settings.getByText(/Cells 1, 2, 3/)).toBeVisible();
	await settings.getByRole("button", { name: "Close settings" }).click();
	await expect
		.poll(async () => {
			const surfaces = Object.values(
				(await virtualZoneSnapshot(api)).desks[api.session!.desk.id] ?? {},
			);
			return surfaces.find((surface) =>
				surface.zones.some((zone) => zone.name === "Touring alternates"),
			);
		})
		.toMatchObject({
			revision: expect.any(Number),
			page_mode: { type: "pinned", page: 2 },
		});

	await page.waitForTimeout(1_000);
	await page.reload();
	await expect(page.locator(".connection-cover")).toBeHidden({
		timeout: 10_000,
	});
	pane = await activeVirtualPane(page);
	await expect(pane.locator(".virtual-playback-box")).toHaveCount(2);
	await pane.getByRole("button", { name: "Settings", exact: true }).click();
	settings = page.getByRole("dialog", { name: "Pane Settings" });
	await settings
		.getByRole("tab", { name: "Virtual Playbacks", exact: true })
		.click();
	await expect(
		settings.getByRole("radio", { name: "Pinned", exact: true }),
	).toBeChecked();
	await expect(settings.getByLabel("Pinned page")).toHaveValue("2");
	await settings
		.getByRole("tab", { name: "Exclusion Zones", exact: true })
		.click();
	await expect(settings.getByLabel("Name for Touring alternates")).toHaveValue(
		"Touring alternates",
	);
	await expect(settings.getByText(/Cells 1, 2, 3/)).toBeVisible();
};

export function registerVirtualZoneUiScenario(): void {
	test(
		"VPB-007 @ui › Settings persist Pinned page mode, hidden membership, and revisioned zone authority",
		virtualZoneUiSupplement,
	);
}
