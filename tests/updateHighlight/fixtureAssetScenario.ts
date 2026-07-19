import fs from "node:fs/promises";
import {
	type BenchUiContext,
	expect,
	test,
} from "../../apps/control-ui/e2e/bench/fixtures";
import type { Locator } from "../../apps/control-ui/node_modules/@playwright/test/index.js";
import { fixtureDefinitionFromProfileMode } from "../../apps/control-ui/src/components/setup/fixtureProfileModel";
import {
	loadCanonicalCopy,
	object,
	objects,
	putObject,
} from "../support/catalog";
import {
	chooseCustomSelect,
	extractFixtureAsset,
	selectConfinedFile,
} from "../support/updateHighlight/system";

type FixtureAssetContext = Pick<
	BenchUiContext,
	"api" | "bench" | "desk" | "page"
>;

test("FIXTURE-002 @restart › focused assets and physical metadata remain immutable across edit, patch, and restart", async ({
	api,
	bench,
	desk,
	page,
}) => {
	test.setTimeout(90_000);
	const context = { api, bench, desk, page };
	const data = await prepareFixtureAssets(context);
	const editor = await openConfiguredProfileEditor(context, data);
	await exerciseInitialAssets(context, data, editor);
	const profile = await loadCreatedProfile(context.api, data);
	const revisions = await createProfileRevision(context, data, editor, profile);
	await patchProfileAndRestart(context, revisions);
});

async function prepareFixtureAssets({ api, bench }: FixtureAssetContext) {
	await loadCanonicalCopy(api, bench, "fixture-002", "default-stage");
	const manufacturer = `Feature 21 ${crypto.randomUUID()}`;
	const name = "Complete Asset Fixture";
	const physical = {
		width_millimetres: 420,
		height_millimetres: 680,
		depth_millimetres: 310,
		weight_kilograms: 24.5,
		power_watts: 720,
		color_temperature_kelvin: 6500,
		luminous_output_lumens: 18500,
		beam_angle_degrees: 36,
	};
	const files = {
		photoA: "fixture-002-photo-a.png",
		photoB: "fixture-002-photo-b.png",
		icon: "fixture-002-icon.png",
		modelA: "fixture-002-model-a.glb",
		modelB: "fixture-002-model-b.glb",
	};
	await extractFixtureAsset(
		"generic--dimmer-profile.toskfixture",
		"assets/icon.png",
		`${bench.dataDir}/shows/${files.photoA}`,
	);
	await extractFixtureAsset(
		"generic--dimmer-par-can.toskfixture",
		"assets/icon.png",
		`${bench.dataDir}/shows/${files.photoB}`,
	);
	await fs.copyFile(
		`${bench.dataDir}/shows/${files.photoB}`,
		`${bench.dataDir}/shows/${files.icon}`,
	);
	await extractFixtureAsset(
		"generic--dimmer-profile.toskfixture",
		"assets/model.glb",
		`${bench.dataDir}/shows/${files.modelA}`,
	);
	await extractFixtureAsset(
		"generic--dimmer-par-can.toskfixture",
		"assets/model.glb",
		`${bench.dataDir}/shows/${files.modelB}`,
	);
	const expectedAssets = {
		photoA: `data:image/png;base64,${(await fs.readFile(`${bench.dataDir}/shows/${files.photoA}`)).toString("base64")}`,
		photoB: `data:image/png;base64,${(await fs.readFile(`${bench.dataDir}/shows/${files.photoB}`)).toString("base64")}`,
		icon: `data:image/png;base64,${(await fs.readFile(`${bench.dataDir}/shows/${files.icon}`)).toString("base64")}`,
		modelA: `data:application/octet-stream;base64,${(await fs.readFile(`${bench.dataDir}/shows/${files.modelA}`)).toString("base64")}`,
		modelB: `data:application/octet-stream;base64,${(await fs.readFile(`${bench.dataDir}/shows/${files.modelB}`)).toString("base64")}`,
	};
	return { manufacturer, name, physical, files, expectedAssets };
}

type FixtureAssetData = Awaited<ReturnType<typeof prepareFixtureAssets>>;

async function openConfiguredProfileEditor(
	{ desk, bench, page }: FixtureAssetContext,
	data: FixtureAssetData,
): Promise<Locator> {
	const { manufacturer, name, physical } = data;
	await desk.open(bench.baseUrl);
	await page.getByRole("button", { name: /Open show menu/ }).click();
	await page.getByRole("button", { name: "Enter Setup", exact: true }).click();
	await page
		.getByRole("button", { name: "Open Fixture Library", exact: true })
		.click();
	await page
		.getByRole("button", { name: "Create fixture", exact: true })
		.click();
	const editor = page.getByRole("dialog", { name: "Create fixture profile" });
	await editor.getByLabel(/^Manufacturer/).fill(manufacturer);
	await editor.getByLabel(/^Fixture name/).fill(name);
	await editor.getByLabel("Fixture short name").fill("Asset Fixture");
	await chooseCustomSelect(editor, "Fixture type", "wash mover");
	await editor
		.getByLabel("Fixture notes")
		.fill("Complete Generic asset and physical metadata acceptance fixture.");
	for (const [label, value] of [
		["Width (mm)", physical.width_millimetres],
		["Height (mm)", physical.height_millimetres],
		["Depth (mm)", physical.depth_millimetres],
		["Weight (kg)", physical.weight_kilograms],
		["Power consumption (W)", physical.power_watts],
		["Color temperature (K)", physical.color_temperature_kelvin],
		["Luminous output (lm)", physical.luminous_output_lumens],
		["Beam angle (degrees)", physical.beam_angle_degrees],
	] as const)
		await editor.getByLabel(label).fill(String(value));
	await expect(editor.getByLabel("Connectors")).toHaveCount(0);
	await expect(editor.getByLabel("Light source")).toHaveCount(0);
	await expect(editor.getByLabel("Color rendering index (CRI)")).toHaveCount(0);
	await expect(editor.getByLabel("Lens")).toHaveCount(0);
	const assetColumns = editor.locator(".fixture-notes-assets > div");
	await expect(assetColumns).toHaveCount(3);
	await expect(
		assetColumns.nth(0).getByRole("heading", { name: "Notes" }),
	).toBeVisible();
	await expect(
		assetColumns.nth(1).getByRole("heading", { name: "Fixture photograph" }),
	).toBeVisible();
	await expect(
		assetColumns.nth(2).getByRole("heading", { name: "Visualizer" }),
	).toBeVisible();
	const assetColumnBoxes = await Promise.all(
		[0, 1, 2].map((index) => assetColumns.nth(index).boundingBox()),
	);
	expect(assetColumnBoxes.every(Boolean)).toBe(true);
	expect(
		Math.max(...assetColumnBoxes.map((box) => box!.width)) -
			Math.min(...assetColumnBoxes.map((box) => box!.width)),
	).toBeLessThan(2);
	expect(assetColumnBoxes[0]!.x).toBeLessThan(assetColumnBoxes[1]!.x);
	expect(assetColumnBoxes[1]!.x).toBeLessThan(assetColumnBoxes[2]!.x);
	return editor;
}

async function exerciseInitialAssets(
	{ page }: FixtureAssetContext,
	data: FixtureAssetData,
	editor: Locator,
): Promise<void> {
	const { files, expectedAssets } = data;
	await editor
		.getByRole("button", { name: "Choose photograph", exact: true })
		.click();
	await selectConfinedFile(page, files.photoA);
	await expect(
		editor.getByAltText("Fixture photograph preview"),
	).toHaveAttribute("src", expectedAssets.photoA);
	await editor
		.getByRole("button", { name: "Replace photograph", exact: true })
		.click();
	await selectConfinedFile(page, files.photoB);
	await expect(
		editor.getByAltText("Fixture photograph preview"),
	).toHaveAttribute("src", expectedAssets.photoB);
	expect(
		await editor.getByAltText("Fixture photograph preview").getAttribute("src"),
	).not.toBe(expectedAssets.photoA);
	await editor
		.getByRole("button", { name: "Remove photograph", exact: true })
		.click();
	await expect(editor.getByAltText("Fixture photograph preview")).toHaveCount(
		0,
	);
	await editor
		.getByRole("button", { name: "Choose fixture icon", exact: true })
		.click();
	await selectConfinedFile(page, files.icon);
	await editor
		.getByRole("button", { name: "Choose visualizer glb model", exact: true })
		.click();
	await selectConfinedFile(page, files.modelA);
	await expect(editor.getByRole("status")).toContainText(
		"GLB 2.0 · 1268 bytes",
	);
	const preview = editor.getByLabel("Visualizer GLB model preview");
	const previewCanvas = preview.locator("canvas");
	await expect(preview).toHaveAttribute(
		"title",
		"Drag to rotate; scroll to zoom",
	);
	await expect(previewCanvas).toBeVisible();
	const beforeOrbit = await previewCanvas.screenshot();
	const previewBox = await previewCanvas.boundingBox();
	expect(previewBox).not.toBeNull();
	await page.mouse.move(
		previewBox!.x + previewBox!.width / 2,
		previewBox!.y + previewBox!.height / 2,
	);
	await page.mouse.down();
	await page.mouse.move(
		previewBox!.x + previewBox!.width * 0.8,
		previewBox!.y + previewBox!.height * 0.6,
		{ steps: 5 },
	);
	await page.mouse.up();
	const afterOrbit = await previewCanvas.screenshot();
	expect(afterOrbit.equals(beforeOrbit)).toBe(false);
	await editor
		.getByRole("button", { name: "Replace visualizer glb model", exact: true })
		.click();
	await selectConfinedFile(page, files.modelB);
	await expect(editor.getByRole("status")).toContainText(
		"GLB 2.0 · 1448 bytes",
	);
	await editor
		.getByRole("button", { name: "Save fixture", exact: true })
		.click();
	await expect(editor).toBeHidden();
}

async function loadCreatedProfile(
	api: FixtureAssetContext["api"],
	data: FixtureAssetData,
) {
	const { manufacturer, name, physical, expectedAssets } = data;
	const profile = (
		await api.request<any[]>(
			"GET",
			"/api/v1/fixture-profiles",
			undefined,
			false,
		)
	).find(
		(candidate) =>
			candidate.manufacturer === manufacturer && candidate.name === name,
	);
	expect(profile).toBeDefined();
	expect(profile).toMatchObject({
		revision: 1,
		photograph_asset: null,
		stage_icon_asset: expectedAssets.icon,
		model_asset: expectedAssets.modelB,
		physical,
	});
	return profile;
}

async function createProfileRevision(
	{ api, page }: FixtureAssetContext,
	data: FixtureAssetData,
	editor: Locator,
	profile: any,
) {
	const { manufacturer, name, files, expectedAssets, physical } = data;
	await page
		.getByPlaceholder("Search manufacturer, fixture, mode, or type")
		.fill(manufacturer);
	await page.getByRole("button", { name: new RegExp(name) }).click();
	await page.getByRole("button", { name: "Edit fixture", exact: true }).click();
	editor = page.getByRole("dialog", { name: "Edit fixture profile" });
	await expect(editor.getByLabel("Width (mm)")).toHaveValue("420");
	await expect(editor.getByLabel("Color temperature (K)")).toHaveValue("6500");
	await expect(editor.getByLabel("Luminous output (lm)")).toHaveValue("18500");
	await expect(editor.getByLabel("Beam angle (degrees)")).toHaveValue("36");
	await expect(editor.getByAltText("Fixture photograph preview")).toHaveCount(
		0,
	);
	await expect(editor.getByText("Fixture icon assigned")).toBeVisible();
	await expect(editor.getByText("Visualizer GLB model assigned")).toBeVisible();
	await expect(editor.getByRole("status")).toContainText(
		"GLB 2.0 · 1448 bytes",
	);

	await editor.getByLabel("Beam angle (degrees)").fill("42");
	await editor
		.getByRole("button", { name: "Choose photograph", exact: true })
		.click();
	await selectConfinedFile(page, files.photoA);
	await editor
		.getByRole("button", { name: "Replace visualizer glb model", exact: true })
		.click();
	await selectConfinedFile(page, files.modelA);
	await expect(editor.getByRole("status")).toContainText(
		"GLB 2.0 · 1268 bytes",
	);
	await editor
		.getByRole("button", { name: "Save fixture", exact: true })
		.click();
	await page
		.getByRole("alertdialog", { name: "Create a new fixture revision?" })
		.getByRole("button", { name: "Save and create revision" })
		.click();
	await expect(editor).toBeHidden();

	const revisions = await api.request<any[]>(
		"GET",
		`/api/v1/fixture-profiles/${profile.id}/revisions`,
		undefined,
		false,
	);
	expect(revisions.map((candidate) => candidate.revision)).toEqual([1, 2]);
	expect(revisions[0]).toMatchObject({
		photograph_asset: null,
		stage_icon_asset: expectedAssets.icon,
		model_asset: expectedAssets.modelB,
		physical: { beam_angle_degrees: 36 },
	});
	expect(revisions[1]).toMatchObject({
		photograph_asset: expectedAssets.photoA,
		stage_icon_asset: expectedAssets.icon,
		model_asset: expectedAssets.modelA,
		physical: { ...physical, beam_angle_degrees: 42 },
	});
	return revisions;
}

async function patchProfileAndRestart(
	{ api, bench }: FixtureAssetContext,
	revisions: any[],
): Promise<void> {
	const definition = fixtureDefinitionFromProfileMode(
		revisions[1],
		revisions[1].modes[0],
	);
	const fixture = (await objects<any>(api, "patched_fixture"))[0];
	await putObject(
		api,
		"patched_fixture",
		fixture.id,
		{
			...fixture.body,
			definition,
			split_patches: [
				{
					split: 1,
					universe: fixture.body.universe,
					address: fixture.body.address,
				},
			],
		},
		fixture.revision,
	);
	const patched = await object<any>(api, "patched_fixture", fixture.id);
	expect(patched.body.definition.profile_snapshot).toMatchObject(revisions[1]);

	await bench.stopServerGracefully(api.session!.token);
	await bench.startServer();
	await api.login();
	const reopened = await object<any>(api, "patched_fixture", fixture.id);
	expect(reopened.body.definition.profile_snapshot).toMatchObject(revisions[1]);
	expect(
		reopened.body.definition.profile_snapshot.physical.beam_angle_degrees,
	).toBe(42);
}
