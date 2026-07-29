import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import type { Locator, Page, TestInfo } from "@playwright/test";
import artifactResolver from "../../../tools/artifact-paths.cjs";
import {
	activeShowId,
	loadCanonicalCopy,
	programmer,
} from "../../support/catalog";
import {
	PLANNED_DEMO_BENCHMARK_ASSIGNMENTS,
	startPlannedDemoBenchmarkLook,
} from "../../support/plannedDemoBenchmark";
import { generatePlannedDemo } from "../../support/plannedDemoGenerator";
import type { ApiDriver } from "../core/api";
import type { DeskDriver } from "../core/desk";
import { expect } from "../core/fixtures";
import type { LightBench } from "../core/lightBench";

const { artifactPaths } = artifactResolver;
const VIDEO = path.join(
	artifactPaths.visual,
	"product-demo",
	"tosklight-product-demo.webm",
);
const SCREENSHOT = path.join(
	artifactPaths.visual,
	"product-demo",
	"tosklight-product-demo-1920x1080.png",
);
const DEMO_SHOW = fileURLToPath(
	new URL("../../../assets/demo.show", import.meta.url),
);
const RECORDING = process.env.LIGHT_VISUAL_RECORDING === "1";
const UPDATE_DEMO_SHOW = process.env.LIGHT_UPDATE_DEMO_SHOW === "1";

export class BrowserProductDemo {
	constructor(
		private readonly page: Page,
		private readonly desk: DeskDriver,
		private readonly bench: LightBench,
		private readonly api: ApiDriver,
		private readonly testInfo: TestInfo,
	) {}

	async run(): Promise<void> {
		const api = this.api;
		const bench = this.bench;
		const desk = this.desk;
		const page = this.page;
		const testInfo = this.testInfo;
		testInfo.setTimeout(RECORDING ? 900_000 : 300_000);
		page.setDefaultTimeout(15_000);
		await loadCanonicalCopy(
			api,
			bench,
			"planned-product-demo",
			"default-stage",
		);
		const video = page.video();
		let completedShow: Buffer | null = null;
		try {
			await desk.open(`${bench.baseUrl}/?demo=product`);
			const demo = page.getByTestId("product-demo");
			const app = demo.locator(".product-demo-application");
			const keypad = demo.locator(".demo-number-block");
			const stage = demo.locator(".stage-3d-canvas");
			await verifyDemoFrame(demo, app, stage);

			await desk.titleCard(
				"SHOW SETUP",
				"Create one empty show, then generate the canonical 262-control, 301-instance lighting venue.",
			);
			const originalShowId = await activeShowId(api);
			await desk.click(app.getByRole("button", { name: /Open show menu/ }));
			await desk.click(
				page.locator(".show-modal").getByRole("button", {
					name: "New Show",
					exact: true,
				}),
			);
			await desk.click(
				page.getByRole("dialog", { name: "New show" }).getByRole("button", {
					name: "Create Empty Show",
					exact: true,
				}),
			);
			await expect.poll(() => activeShowId(api)).not.toBe(originalShowId);
			const showId = await activeShowId(api);
			expect(await api.showObjects(showId, "group")).toHaveLength(0);
			expect(await api.showObjects(showId, "preset")).toHaveLength(0);
			await desk.click(
				page.locator(".show-modal").getByRole("button", {
					name: "Show Patch",
					exact: true,
				}),
			);
			const patchWindow = app.locator(".show-patch-layout");
			await expect(patchWindow).toBeVisible();
			const canonical = await desk.fastForward(
				"Building the canonical trusses, floor clusters, audience grid, auxiliary rig, ACL fans, groups, presets, playbacks and Dynamics.",
				() => generatePlannedDemo(api, showId),
			);
			expect(canonical.patch).toMatchObject({
				fixtureRecords: 262,
				physicalInstances: 301,
				occupiedSlots: 3_783,
			});
			await expect
				.poll(async () => (await api.patch()).fixtures.length)
				.toBe(262);
			await expect(patchWindow.locator(".ui-window-info")).toContainText(
				"262 fixtures",
			);
			await expect(fixtureRow(patchWindow, 101)).toBeVisible();

			await configureOutput(desk, page, app, bench);
			await demonstrateGroups(desk, app, keypad, api);
			await demonstrateFixtureControls(desk, page, demo, app, keypad, api);
			await demonstratePresetRecall(desk, app, keypad, api);
			await demonstrateCueProgramming(desk, app, api, showId);
			await demonstrateBusking(desk, demo, api, bench, showId);
			await demonstratePreload(desk, demo, keypad, api);

			await desk.titleCard(
				"ACL CHASER · SPEED D",
				"The four-step ACL chase remains active on Speed Group D while a final keypad action proves normal post-generation control.",
			);
			await expect.poll(async () => activeNumbers(api)).toContain(17);
			await clearSelection(desk, keypad, api);
			await keypadCommand(desk, keypad, ["1", "0", "1", "ENT"]);
			await keypadCommand(desk, keypad, ["AT", "8", "0", "ENT"]);
			await expectLiveOutput(api);
			if (UPDATE_DEMO_SHOW) {
				completedShow = await downloadCompletedDemoShow(api, showId);
			}
			if (RECORDING) {
				await fs.mkdir(path.dirname(SCREENSHOT), { recursive: true });
				await page.screenshot({ path: SCREENSHOT });
				await testInfo.attach("planned-demo-full-hd-screenshot", {
					path: SCREENSHOT,
					contentType: "image/png",
				});
			}
		} finally {
			if (video) {
				await fs.mkdir(path.dirname(VIDEO), { recursive: true });
				const saveVideo = video.saveAs(VIDEO);
				await page.close();
				await saveVideo;
				await testInfo.attach("planned-demo-video", {
					path: VIDEO,
					contentType: "video/webm",
				});
			}
		}
		if (UPDATE_DEMO_SHOW) {
			if (!completedShow) {
				throw new Error("The completed demo show was not downloaded");
			}
			await publishDemoShowAsset(completedShow);
			await testInfo.attach("completed-demo-show", {
				path: DEMO_SHOW,
				contentType: "application/vnd.light.show",
			});
		}
	}
}

async function verifyDemoFrame(demo: Locator, app: Locator, stage: Locator) {
	await expect(demo).toBeVisible();
	await expect(demo.locator(".product-demo-screen-frame")).toHaveCSS(
		"border-left-width",
		"10px",
	);
	const appBox = await app.boundingBox();
	if (!appBox) {
		throw new Error("The product demo application has no visible bounds");
	}
	expect(appBox.width / appBox.height).toBeCloseTo(16 / 9, 2);
	await expect(demo.locator("[data-demo-chapter]")).toHaveCount(8);
	await expect(
		app.locator(".control-section.hardware-connected"),
	).toBeVisible();
	await expect(stage.locator("canvas")).toBeVisible();
	await stage.locator("canvas").evaluate((canvas) => {
		canvas.dataset.recordingCanvas = "stable";
	});
	await expect(stage).toHaveAttribute("data-camera-position", "0,1.625,8");
	await expect(stage).toHaveAttribute("data-camera-target", "0,2.6,-4");
	await expect(stage).toHaveAttribute("data-environment-brightness", "1");
	await expect(stage).toHaveAttribute("data-floor-grid", "off");
	await expect(stage).toHaveAttribute("data-beam-guides", "off");
	for (const universe of [1, 2, 3, 4]) {
		await expect(
			demo
				.getByLabel(`Live DMX universe ${universe}`)
				.locator(".product-demo-dmx-cell"),
		).toHaveCount(512);
	}
}

async function configureOutput(
	desk: DeskDriver,
	page: Page,
	app: Locator,
	bench: LightBench,
) {
	await desk.titleCard(
		"OUTPUT CONFIGURATION",
		"Configure live Art-Net and sACN routes after the canonical lighting venue exists.",
	);
	await desk.click(app.getByRole("button", { name: /Open show menu/ }));
	await desk.click(
		page.locator(".show-modal").getByRole("button", {
			name: "Enter Setup",
			exact: true,
		}),
	);
	await desk.click(
		app.locator(".setup-window nav").getByRole("button", {
			name: "Outputs",
			exact: true,
		}),
	);
	const routes = app.getByRole("region", { name: "Output routes" });
	await createOutputRoute(
		desk,
		page,
		routes,
		"Art-Net",
		1,
		1,
		bench.artnet.port,
	);
	await createOutputRoute(desk, page, routes, "sACN", 2, 2, bench.sacn.port);
	await createOutputRoute(
		desk,
		page,
		routes,
		"Art-Net",
		3,
		3,
		bench.artnet.port,
	);
	await expect(routes.locator("article")).toHaveCount(3);
}

async function demonstrateGroups(
	desk: DeskDriver,
	app: Locator,
	keypad: Locator,
	api: ApiDriver,
) {
	await desk.titleCard(
		"GROUP PREPARATION",
		"Use the final Profile Stage Group through the normal keypad and Fixture Sheet.",
	);
	await openBuiltIn(desk, app, "Fixtures");
	const fixtureWindow = app.locator(".fixture-window");
	await expect(fixtureWindow).toBeVisible();
	await clearSelection(desk, keypad, api);
	await keypadCommand(desk, keypad, ["GRP", "2", "ENT"]);
	await expect
		.poll(async () => (await programmer(api)).selected.length)
		.toBe(28);
	await expect(fixtureWindow.locator(".group-strip")).toContainText(
		"Profile Stage",
	);
}

async function demonstrateFixtureControls(
	desk: DeskDriver,
	page: Page,
	demo: Locator,
	app: Locator,
	keypad: Locator,
	api: ApiDriver,
) {
	await desk.titleCard(
		"BUILT-IN FIXTURE CONTROL ACTIONS",
		"The selected Profile exposes Lamp On, Fan Auto, Reset and Lamp Off through one centered control surface.",
	);
	await clearSelection(desk, keypad, api);
	await keypadCommand(desk, keypad, ["1", "0", "1", "ENT"]);
	await desk.click(app.getByRole("button", { name: "Control", exact: true }));
	await desk.click(
		app.getByRole("button", { name: "Special Dialog", exact: true }),
	);
	const dialog = page.locator(".special-dialog-card");
	await expect(dialog).toContainText(/1 fixtures? selected/);
	await expectCenteredInDemoSurface(dialog, demo);
	for (const name of ["Lamps On", "Fan Auto", "Reset", "Lamp Off"]) {
		const action = dialog.getByRole("button", { name, exact: true });
		await expect(action).toBeVisible();
		await desk.click(action);
	}
	await desk.click(dialog.getByRole("button", { name: "×", exact: true }));
}

async function demonstratePresetRecall(
	desk: DeskDriver,
	app: Locator,
	keypad: Locator,
	api: ApiDriver,
) {
	await desk.titleCard(
		"PRESET PROGRAMMING",
		"Recall the canonical Red preset on Show Profile; the recording performs no redundant Green or Blue zero edits.",
	);
	await clearProgrammer(desk, keypad, api);
	await keypadCommand(desk, keypad, ["GRP", "1", "3", "ENT"]);
	await openBuiltIn(desk, app, "Presets");
	const presets = app.locator(".preset-pool-window");
	await desk.click(presets.getByRole("button", { name: "Color", exact: true }));
	await desk.click(presets.getByRole("button", { name: /Red Color ·/ }));
	await expect.poll(() => programmerValueCount(api)).toBeGreaterThan(0);
	await expect(
		presets.getByRole("button", { name: /Tungsten White Color ·/ }),
	).toBeVisible();
}

async function demonstrateCueProgramming(
	desk: DeskDriver,
	app: Locator,
	api: ApiDriver,
	showId: string,
) {
	await desk.titleCard(
		"CUE PROGRAMMING",
		"Inspect the final seven-Cuelist topology after using the Fixture Sheet and canonical Color and Position preset pools.",
	);
	await openBuiltIn(desk, app, "Cuelists");
	await expect(app.locator(".cuelist-window")).toContainText("Start");
	expect(await api.showObjects(showId, "cue_list")).toHaveLength(7);
	expect(await api.showObjects(showId, "playback")).toHaveLength(13);
}

async function demonstrateBusking(
	desk: DeskDriver,
	demo: Locator,
	api: ApiDriver,
	bench: LightBench,
	showId: string,
) {
	await desk.titleCard(
		"BUSKING",
		"Start the composed white look, ACL chase and all canonical benchmark Dynamics through their final assignments.",
	);
	await desk.click(
		demo.getByRole("button", {
			name: "Playback 11 button 1",
			exact: true,
		}),
	);
	await desk.click(
		demo.getByRole("button", {
			name: "Playback 17 button 1",
			exact: true,
		}),
	);
	await bench.tick(1_200);
	const runtime = await startPlannedDemoBenchmarkLook(api, showId);
	expect(runtime.projections).toHaveLength(
		PLANNED_DEMO_BENCHMARK_ASSIGNMENTS.length,
	);
	expect(
		runtime.projections.every((projection: any) =>
			projection.target === "cue_list"
				? projection.runtime?.enabled === true
				: projection.target === "dynamic" &&
					projection.runtime?.state === "active",
		),
	).toBe(true);
	await expectLiveOutput(api);
}

async function demonstratePreload(
	desk: DeskDriver,
	demo: Locator,
	keypad: Locator,
	api: ApiDriver,
) {
	await desk.titleCard(
		"PRELOADING",
		"Prepare two canonical ACL playbacks blind and commit them together through the physical Preload Go path.",
	);
	await clearProgrammer(desk, keypad, api);
	await desk.click(
		keypad.getByRole("button", { name: "PRELOAD GO", exact: true }),
	);
	await desk.click(
		demo.getByRole("button", {
			name: "Playback 12 button 1",
			exact: true,
		}),
	);
	await desk.click(
		demo.getByRole("button", {
			name: "Playback 15 button 1",
			exact: true,
		}),
	);
	expect((await preloadState(api)).playbackCount).toBeGreaterThanOrEqual(2);
	await desk.click(
		keypad.getByRole("button", { name: "PRELOAD GO", exact: true }),
	);
	await expect.poll(async () =>
		(await preloadState(api)).toEqual({
			capturesValues: false,
			valueCount: 0,
			playbackCount: 0,
		}),
	);
}

async function downloadCompletedDemoShow(
	api: ApiDriver,
	showId: string,
): Promise<Buffer> {
	for (const route of await api.showObjects<any>(showId, "route")) {
		const port = route.body.protocol === "sacn" ? 5568 : 6454;
		await api.seedShowObject(
			showId,
			"route",
			route.id,
			{
				...route.body,
				destination: `127.0.0.1:${port}`,
			},
			route.revision,
		);
	}
	return api.downloadShow(showId);
}

async function publishDemoShowAsset(show: Buffer): Promise<void> {
	const temporary = `${DEMO_SHOW}.${process.pid}.${crypto.randomUUID()}.tmp`;
	await fs.mkdir(path.dirname(DEMO_SHOW), { recursive: true });
	await fs.writeFile(temporary, show);
	await fs.rename(temporary, DEMO_SHOW);
}

function fixtureRow(patchWindow: Locator, fixtureNumber: number | string) {
	return patchWindow
		.getByRole("cell", { name: String(fixtureNumber), exact: true })
		.locator("..")
		.first();
}

async function openBuiltIn(desk: DeskDriver, app: Locator, name: string) {
	const toggle = app.getByRole("button", {
		name: "Desktops / Built-ins",
		exact: true,
	});
	if ((await toggle.getAttribute("data-dock-mode")) === "desks") {
		await desk.click(toggle);
	}
	await desk.click(app.getByRole("button", { name, exact: true }));
}

async function createOutputRoute(
	desk: DeskDriver,
	page: Page,
	routes: Locator,
	protocol: "Art-Net" | "sACN",
	logicalUniverse: number,
	destinationUniverse: number,
	port: number,
) {
	await desk.click(
		routes.getByRole("button", { name: "Add route", exact: true }),
	);
	const editor = page.getByRole("dialog", { name: "Output route editor" });
	if (protocol === "sACN") {
		await desk.click(
			editor.getByRole("button", { name: "Art-Net", exact: true }),
		);
		await desk.click(page.getByRole("option", { name: "sACN", exact: true }));
	}
	await desk.click(
		editor.getByRole("button", {
			name: protocol === "Art-Net" ? "Broadcast" : "Multicast",
			exact: true,
		}),
	);
	await desk.click(page.getByRole("option", { name: "Unicast", exact: true }));
	await editor.getByLabel("Logical universe").fill(String(logicalUniverse));
	await editor
		.getByLabel("Destination universe")
		.fill(String(destinationUniverse));
	await editor
		.getByLabel("Destination", { exact: true })
		.fill(`127.0.0.1:${port}`);
	await editor.getByLabel("Minimum universe size").fill("128");
	await desk.click(
		editor.getByRole("button", { name: "Save route", exact: true }),
	);
}

async function keypadCommand(
	desk: DeskDriver,
	keypad: Locator,
	labels: string[],
) {
	await desk.click(keypad.getByRole("button", { name: "ESCAPE", exact: true }));
	for (const label of labels) {
		await desk.click(keypad.getByRole("button", { name: label, exact: true }));
		if (!RECORDING) await keypad.page().waitForTimeout(20);
	}
}

async function clearSelection(
	desk: DeskDriver,
	keypad: Locator,
	api: ApiDriver,
) {
	for (
		let attempt = 0;
		attempt < 12 && (await programmer(api)).selected.length;
		attempt++
	) {
		await desk.click(keypad.getByRole("button", { name: "CLR", exact: true }));
	}
	await expect.poll(async () => (await programmer(api)).selected).toEqual([]);
}

async function clearProgrammer(
	desk: DeskDriver,
	keypad: Locator,
	api: ApiDriver,
) {
	for (let attempt = 0; attempt < 16; attempt++) {
		const state = await programmer(api);
		if (
			!state.selected.length &&
			!state.values.length &&
			!Object.keys(state.group_values).length
		) {
			return;
		}
		await desk.click(keypad.getByRole("button", { name: "CLR", exact: true }));
	}
	throw new Error("Product demo could not clear the Programmer");
}

async function preloadState(api: ApiDriver) {
	const userId = api.session?.user.id;
	if (!userId)
		throw new Error("The product demo requires an authenticated user");
	const [capture, values, playback] = await Promise.all([
		api.request<any>(
			"GET",
			`/api/v2/users/${userId}/programmer-capture-mode/snapshot`,
		),
		api.request<any>(
			"GET",
			`/api/v2/users/${userId}/programmer-preload-values/snapshot`,
		),
		api.request<any>(
			"GET",
			`/api/v2/users/${userId}/programmer-preload-playback-queue/snapshot`,
		),
	]);
	return {
		capturesValues:
			capture.projection.blind && capture.projection.preload_capture_programmer,
		valueCount:
			values.projection.fixture_values.length +
			values.projection.group_values.length,
		playbackCount: playback.projection.actions.length,
	};
}

async function programmerValueCount(api: ApiDriver) {
	const state = await programmer(api);
	return state.values.length + Object.keys(state.group_values).length;
}

async function activeNumbers(api: ApiDriver): Promise<number[]> {
	return (await api.request<any>("GET", "/api/v2/playback-overview")).active
		.filter((item: any) => item.enabled)
		.map((item: any) => item.playback_number);
}

async function expectLiveOutput(api: ApiDriver) {
	await expect
		.poll(async () =>
			(
				await api.request<any>("GET", "/api/v2/output/dmx", undefined, false)
			).universes.some((frame: any) =>
				frame.slots.some((value: number) => value > 0),
			),
		)
		.toBe(true);
}

async function expectCenteredInDemoSurface(dialog: Locator, surface: Locator) {
	const [dialogBox, surfaceBox] = await Promise.all([
		dialog.boundingBox(),
		surface.boundingBox(),
	]);
	if (!dialogBox || !surfaceBox) {
		throw new Error("The product demo dialog or surface has no visible bounds");
	}
	const dialogCenterX = dialogBox.x + dialogBox.width / 2;
	const dialogCenterY = dialogBox.y + dialogBox.height / 2;
	const surfaceCenterX = surfaceBox.x + surfaceBox.width / 2;
	const surfaceCenterY = surfaceBox.y + surfaceBox.height / 2;
	expect(Math.abs(dialogCenterX - surfaceCenterX)).toBeLessThanOrEqual(4);
	expect(Math.abs(dialogCenterY - surfaceCenterY)).toBeLessThanOrEqual(4);
}
