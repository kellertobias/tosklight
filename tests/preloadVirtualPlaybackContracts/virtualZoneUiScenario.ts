import { configurePaneGrid, paneCard } from "./uiSupport";
import type { Locator, Page } from "@playwright/test";
import { type BenchUiContext, expect, test } from "../bench/core/fixtures";
import {
	activeVirtualPane,
	activeAssignedPlayback,
	addVirtualPlaybackPane,
	prepare,
	paneAction,
	soloRegionSnapshot,
	copyPageAssignments,
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
			{ position: 74, fixture: 3, levels: [0.25], name: "Touring A" },
			{ position: 75, fixture: 4, levels: [0.5], name: "Touring B" },
			{ position: 76, fixture: 5, levels: [0.75], name: "Touring C" },
		],
		{},
	);
	await copyPageAssignments(api, 1, { 401: 74, 402: 75, 403: 76 });
	await desk.open(bench.baseUrl);
	let pane = await addVirtualPlaybackPane(page);
	await desk.recordStep(
		"SELECT EXCLUSION MEMBERS",
		"Hold Shift and choose cells 1 and 2. Selection must not operate either playback.",
	);
	const manager = await openSoloManager(page, pane);
	await manager.getByRole("button", { name: "Create New Solo Region", exact: true }).click();
	await expect(manager).toBeHidden();
 await page.keyboard.down("Shift");
 await paneCard(pane,{page:1,position:401}).click();
 await paneCard(pane,{page:1,position:402}).click();
 await page.keyboard.up("Shift");
	expect(await activeAssignedPlayback(api, {page: 1, position: 401})).toBeUndefined();
	expect(await activeAssignedPlayback(api, {page: 1, position: 402})).toBeUndefined();
	await pane.getByLabel("Solo Region name").fill("Touring pair");
 await pane.getByRole("button", { name: "Save Solo Region", exact: true }).click();
	await expect
		.poll(async () => {
			const snapshot = await soloRegionSnapshot(api);
			return snapshot.regions.find((zone) => zone.name === "Touring pair");
		})
		.toMatchObject({
			members: [{page:1,position:401}, {page:1,position:402}],
		});

	await desk.recordStep(
		"NEW ACTIVATION WINS",
		"Turn on cell 1, then cell 2. Cell 2 remains On and cell 1 is released by the server.",
	);
	await paneCard(pane, {page:1,position:401}).getByRole("button",{name:/^(GO \+|TOGGLE|ON)$/})
		.click();
	await paneCard(pane, {page:1,position:402}).getByRole("button",{name:/^(GO \+|TOGGLE|ON)$/})
		.click();
	await expect
		.poll(async () => (await activeAssignedPlayback(api, {page: 1, position: 401}))?.enabled)
		.toBe(false);
	await expect
		.poll(async () => (await activeAssignedPlayback(api, {page: 1, position: 402}))?.enabled)
		.toBe(true);

	await configurePaneGrid(page,pane,14,20,2);

	let settings = await openSoloManager(page, pane);
	await settings.getByRole("button", { name: "Touring pair", exact: true }).click();
	await settings.getByRole("button", { name: "Edit", exact: true }).click();
	await expect(settings).toBeHidden();
	await pane.getByLabel("Solo Region name").fill("Touring alternates");
	await page.keyboard.down("Shift");
	await paneCard(pane, { page: 2, position: 403 }).click();
	await page.keyboard.up("Shift");
	await pane.getByRole("button", { name: "Save Solo Region", exact: true }).click();
	await expect
		.poll(async () => {
			const snapshot = await soloRegionSnapshot(api);
			return snapshot.regions.find((zone) => zone.name === "Touring alternates");
		})
		.toMatchObject({
			members: [{page:1,position:401}, {page:1,position:402}, {page:2,position:403}],
		});

	await configurePaneGrid(page,pane,1,2,2);
	await page.waitForTimeout(1_000);
	await page.reload();
	await expect(page.locator(".connection-cover")).toBeHidden({
		timeout: 10_000,
	});
	pane = await activeVirtualPane(page);
	await expect(pane.locator("[data-playback-slot]")).toHaveCount(2);
	await pane.getByRole("button", { name: "Settings", exact: true }).click();
	settings = page.getByRole("dialog", { name: "Pane Settings" });
	const surfaceSnapshot=await api.request<any>("GET","/api/v2/playback-surfaces",undefined,true,undefined,{deskId:api.session!.desk.id});
 const pinned=Object.values(surfaceSnapshot.playback_surfaces).find((surface:any)=>surface.page_source.type==="pinned"&&surface.page_source.page===2&&surface.layout.rows.length===1) as any;
 expect(pinned).toBeDefined();
 expect(pinned.layout.rows[0].count).toBe(2);
	await settings
		.getByRole("tab", { name: "Manage Solo Regions", exact: true })
		.click();
	await settings.getByRole("button", { name: "Touring alternates", exact: true }).click();
	await expect(settings.getByRole("button", { name: "Edit", exact: true })).toBeVisible();
	await expect(
		settings.getByText(/1\.401.*1\.402.*2\.403/),
	).toBeVisible();
	await settings.getByRole("button", { name: "Delete", exact: true }).click();
	await expect
		.poll(async () => (await soloRegionSnapshot(api)).regions)
		.toEqual([]);
	await settings.getByRole("button", { name: "Close settings" }).click();

	for (const number of [401, 402]) await paneAction(api, {page: 1, position: number}, "off");
	await paneAction(api, {page: 1, position: 401}, "on");
	await paneAction(api, {page: 1, position: 402}, "on");
	expect(await activeAssignedPlayback(api, {page: 1, position: 401})).toMatchObject({
		enabled: true,
	});
	expect(await activeAssignedPlayback(api, {page: 1, position: 402})).toMatchObject({
		enabled: true,
	});

	await page.reload();
	await expect(page.locator(".connection-cover")).toBeHidden({
		timeout: 10_000,
	});
	pane = await activeVirtualPane(page);
	await pane.getByRole("button", { name: "Settings", exact: true }).click();
	settings = page.getByRole("dialog", { name: "Pane Settings" });
	await settings
		.getByRole("tab", { name: "Manage Solo Regions", exact: true })
		.click();
	await expect(settings.getByRole("button", { name: /^Touring/ })).toHaveCount(0);
	await expect(settings.getByRole("button", { name: "Edit", exact: true })).toHaveCount(0);
	await expect(settings.getByRole("button", { name: "Create New Solo Region", exact: true })).toBeEnabled();
};

export function registerVirtualZoneUiScenario(): void {
	test(
		"VPB-007 @ui › Settings preserve exact cross-page members and explicitly delete Solo Region authority",
		virtualZoneUiSupplement,
	);
}

async function openSoloManager(page: Page, pane: Locator): Promise<Locator> {
	await pane.getByRole("button", { name: "Settings", exact: true }).click();
	const settings = page.getByRole("dialog", { name: "Pane Settings" });
	await settings.getByRole("tab", { name: "Manage Solo Regions", exact: true }).click();
	return settings;
}
