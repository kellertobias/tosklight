import {
	type BenchUiContext,
	expect,
	test,
} from "../../apps/control-ui/e2e/bench/fixtures";
import {
	activePlayback,
	activeVirtualPane,
	addVirtualPlaybackPane,
	poolAction,
	prepare,
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
		{ 1: 74, 2: 75, 3: 76 },
	);
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
	expect(await activePlayback(api, 74)).toBeUndefined();
	expect(await activePlayback(api, 75)).toBeUndefined();
	await pane.getByRole("button", { name: "Create Exclusion Zone" }).click();
	const create = page.getByRole("dialog", { name: "Create Exclusion Zone" });
	await create.getByLabel("Zone name").fill("Touring pair");
	await create.getByRole("button", { name: "Create zone" }).click();
	await expect(create).toBeHidden();

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
		.poll(async () => (await activePlayback(api, 74))?.enabled)
		.toBe(false);
	await expect
		.poll(async () => (await activePlayback(api, 75))?.enabled)
		.toBe(true);

	await poolAction(api, 74, "off");
	await poolAction(api, 75, "off");
	await desk.recordStep(
		"KEYBOARD USES THE SAME ZONE",
		"F1 followed by F2 operates the current-page cells through the shared server path; F2 wins.",
	);
	await page.keyboard.press("F1");
	await page.keyboard.press("F2");
	await expect
		.poll(async () => (await activePlayback(api, 74))?.enabled)
		.toBe(false);
	await expect
		.poll(async () => (await activePlayback(api, 75))?.enabled)
		.toBe(true);

	await pane.getByRole("button", { name: "Settings", exact: true }).click();
	let settings = page.getByRole("dialog", { name: "Pane Settings" });
	await settings
		.getByRole("tab", { name: "Virtual Playbacks", exact: true })
		.click();
	await settings.getByLabel("Name for Touring pair").fill("Touring alternates");
	await settings.getByRole("button", { name: "Save name" }).click();
	await settings
		.getByRole("button", { name: "Touring alternates cell 3" })
		.click();
	await settings.getByLabel("Rows").fill("1");
	await settings.getByLabel("Columns").fill("2");
	await expect(
		settings.getByText("1 hidden grid cell is retained:"),
	).toBeVisible();
	await expect(
		settings.getByRole("button", { name: "Touring alternates hidden cell 3" }),
	).toBeVisible();
	await settings.getByRole("button", { name: "Close settings" }).click();

	await page.waitForTimeout(1_000);
	await page.reload();
	await expect(page.locator(".connection-cover")).toBeHidden({
		timeout: 10_000,
	});
	pane = await activeVirtualPane(page);
	await expect(pane.locator(".virtual-playback-cell")).toHaveCount(2);
	await pane.getByRole("button", { name: "Settings", exact: true }).click();
	settings = page.getByRole("dialog", { name: "Pane Settings" });
	await settings
		.getByRole("tab", { name: "Virtual Playbacks", exact: true })
		.click();
	await expect(settings.getByLabel("Name for Touring alternates")).toHaveValue(
		"Touring alternates",
	);
	await expect(
		settings.getByRole("button", { name: "Touring alternates hidden cell 3" }),
	).toBeVisible();
};

export function registerVirtualZoneUiScenario(): void {
	test(
		"VPB-007 @supplemental-ui › Settings edits hidden membership and reload restores it",
		virtualZoneUiSupplement,
	);
}
