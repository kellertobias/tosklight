import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import type { Locator, Page, TestInfo } from "@playwright/test";
import type { FixtureProfile } from "../../../apps/light-desktop/src/api/types";
import type { FrontendPerformanceSnapshot } from "../../../apps/light-desktop/src/features/frontendWarmup/diagnostics";
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
import {
	installPlannedDemoDynamicPlaybacks,
	installPlannedDemoDynamics,
} from "../../support/plannedDemoDynamics";
import { ensurePlannedDemoFixtureLibrary } from "../../support/plannedDemoFixtureLibrary";
import { ensurePlannedDemoLayers } from "../../support/plannedDemoGenerator";
import { installPlannedDemoGroups } from "../../support/plannedDemoGroups";
import { installPlannedDemoLayout } from "../../support/plannedDemoLayouts";
import {
	createPlannedDemoPatchInputs,
	installPlannedDemoPatch,
} from "../../support/plannedDemoPatch";
import { installPlannedDemoPlaybacks } from "../../support/plannedDemoPlaybacks";
import { installPlannedDemoPresets } from "../../support/plannedDemoPresets";
import {
	installPlannedDemoScenery,
	PLANNED_DEMO_TOTAL_FIXTURE_RECORDS,
	PLANNED_DEMO_TOTAL_PHYSICAL_INSTANCES,
} from "../../support/plannedDemoScenery";
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
const PERFORMANCE = path.join(
	artifactPaths.visual,
	"product-demo",
	"canonical-demo-performance.json",
);
const DEMO_SHOW = fileURLToPath(
	new URL("../../../assets/demo.show", import.meta.url),
);
const APPLICATION_ICON = fileURLToPath(
	new URL("../../../apps/light-desktop/src-tauri/icons/icon.png", import.meta.url),
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
		let desk = this.desk;
		let page = this.page;
		const testInfo = this.testInfo;
		testInfo.setTimeout(RECORDING ? 900_000 : 300_000);
		page.setDefaultTimeout(15_000);
		await loadCanonicalCopy(
			api,
			bench,
			"planned-product-demo",
			"default-stage",
		);
		let video = page.video();
		let completedShow: Buffer | null = null;
		let performanceBaseline: ProductDemoPerformanceBaseline | null = null;
		try {
			await desk.open(`${bench.baseUrl}/?demo=product`);
			const originalShowId = await activeShowId(api);
			const setupApp = page
				.getByTestId("product-demo")
				.locator(".product-demo-application");
			await desk.click(
				setupApp.getByRole("button", { name: /Open show menu/ }),
			);
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
			const layers = await ensurePlannedDemoLayers(api, showId, {});
			if (RECORDING) {
				const setupPage = page;
				page = await setupPage.context().newPage();
				desk = this.desk.fork(page);
				const icon = await fs.readFile(APPLICATION_ICON);
				await desk.prepareProductIntro(
					`data:image/png;base64,${icon.toString("base64")}`,
				);
				await desk.open(`${bench.baseUrl}/?demo=product`);
				video = page.video();
				await setupPage.close();
			} else await page.reload();
			const demo = page.getByTestId("product-demo");
			const app = demo.locator(".product-demo-application");
			const keypad = demo.locator(".demo-number-block");
			const stage = demo.locator(".stage-3d-canvas");
			await verifyDemoFrame(demo, app, stage);
			await desk.productIntro();
			await desk.titleCard(
				"SHOW SETUP",
				"The recording starts with an empty show. Build the venue through the Touch UI, then accelerate the repetitive patch work.",
			);
			await ensurePlannedDemoFixtureLibrary(api);
			const profiles = (await api.fixtureProfilesSnapshot())
				.profiles as FixtureProfile[];
			const desiredPatch = createPlannedDemoPatchInputs(profiles, {});
			const desiredByNumber = new Map(
				desiredPatch.fixtures.map((fixture) => [
					fixture.fixture_number,
					fixture,
				]),
			);
			const showModal = page.locator(".show-modal");
			if (!(await showModal.isVisible()))
				await desk.click(app.getByRole("button", { name: /Open show menu/ }));
			await desk.click(
				showModal.getByRole("button", {
					name: "Show Patch",
					exact: true,
				}),
			);
			const patchWindow = app.locator(".show-patch-layout");
			await expect(patchWindow).toBeVisible();
			await desk.fastForward(
				"Creating the production Patch layers before fixtures are assigned.",
				async () => layers,
			);
			await selectPatchLayer(desk, patchWindow, "Trusses");
			for (let truss = 1; truss <= 3; truss++) {
				await addFixtureThroughTouchUi(desk, page, {
					search: "Four-Point Truss",
					family: "Four-Point Truss",
					mode: "2 m",
					name: `${["Back", "Mid", "Front"][truss - 1]} Truss Segment 1`,
					fixtureId: `0.${truss}`,
					count: 1,
				});
				await desk.click(fixtureRow(patchWindow, `0.${truss}`));
				for (let segment = 1; segment <= 3; segment++) {
					await desk.click(
						patchWindow.getByRole("button", {
							name: "+ Add multi-patch",
							exact: true,
						}),
					);
					await expect(patchWindow.locator(".multipatch-row")).toHaveCount(
						(truss - 1) * 3 + segment,
					);
					if (truss === 1 && segment === 1)
						desk.setRecordingClickPace("compact");
				}
			}
			const trussIdentities = await fixtureIdentities(api, [
				"0.1",
				"0.2",
				"0.3",
			]);
			await selectPatchLayer(desk, patchWindow, "Stage & Venue");
			await desk.fastForward(
				"Adding the stage elements, curtain, back and side railings, and vertical pipes.",
				() => installPlannedDemoScenery(api, showId, layers),
			);
			expect(await fixtureIdentities(api, ["0.1", "0.2", "0.3"])).toEqual(
				trussIdentities,
			);
			const firstFresnel = requiredFixture(desiredByNumber, 1);
			await selectPatchLayer(desk, patchWindow, "Front Lights");
			await addFixtureThroughTouchUi(desk, page, {
				search: "Dimmer Fresnel",
				family: "Dimmer Fresnel",
				mode: "8-bit",
				name: "Front Fresnel 1",
				fixtureId: "1",
				count: 8,
				address: fixtureAddress(firstFresnel),
			});
			await expect
				.poll(async () => {
					const fixture = (await api.patch()).fixtures.find(
						(candidate: any) => candidate.fixture_number === 1,
					);
					return [fixture?.universe, fixture?.address];
				})
				.toEqual([1, 1]);
			await spreadFixtureLocationThroughTouchUi(
				desk,
				patchWindow,
				1,
				4,
				"X",
				["−", "4", "THRU", "−", "3", "ENTER"],
			);
			await expect
				.poll(async () => {
					const fixtures = (await api.patch()).fixtures;
					return [1, 2, 3, 4].map(
						(number) =>
							fixtures.find((fixture: any) => fixture.fixture_number === number)
								?.location.x,
					);
				})
				.toEqual([-4_000, -3_667, -3_333, -3_000]);
			const acl = requiredFixture(desiredByNumber, 601);
			await selectPatchLayer(desk, patchWindow, "ACLs & Blinder");
			await addFixtureThroughTouchUi(desk, page, {
				search: "ACL",
				family: "ACL",
				mode: "8-bit",
				name: "Back Centre ACL",
				fixtureId: "601",
				count: 1,
				address: fixtureAddress(acl),
			});
			await desk.click(fixtureRow(patchWindow, 601));
			const multipatchesBeforeAcl = await patchWindow
				.locator(".multipatch-row")
				.count();
			for (let instance = 1; instance <= 7; instance++) {
				await desk.click(
					patchWindow.getByRole("button", {
						name: "+ Add multi-patch",
						exact: true,
					}),
				);
				await expect(patchWindow.locator(".multipatch-row")).toHaveCount(
					multipatchesBeforeAcl + instance,
				);
			}
			const profile = requiredFixture(desiredByNumber, 101);
			await selectPatchLayer(desk, patchWindow, "Profile Stage");
			await addFixtureThroughTouchUi(desk, page, {
				search: "Robin DLS Profile",
				family: "Robin DLS Profile",
				mode: "Mode 3",
				name: "Profile Stage 1",
				fixtureId: "101",
				count: 1,
				address: fixtureAddress(profile),
			});
			const visibleLightingNumbers = [1, 2, 3, 4, 5, 6, 7, 8, 601, 101];
			const visibleLightingIdentities = await fixtureIdentities(
				api,
				visibleLightingNumbers,
			);
			const canonical = await desk.fastForward(
				"Patching the rest of the lighting show and reconciling the complete Stage layout.",
				async () => {
					const scenery = await installPlannedDemoScenery(api, showId, layers);
					const patch = await installPlannedDemoPatch(api, showId, layers);
					return { patch, scenery, layers };
				},
			);
			expect(await fixtureIdentities(api, visibleLightingNumbers)).toEqual(
				visibleLightingIdentities,
			);
			expect(canonical.patch).toMatchObject({
				fixtureRecords: 262,
				physicalInstances: 301,
				occupiedSlots: 3_783,
			});
			await expect
				.poll(async () => (await api.patch()).fixtures.length)
				.toBe(PLANNED_DEMO_TOTAL_FIXTURE_RECORDS);
			await expect(patchWindow.locator(".ui-window-info")).toContainText(
				`${PLANNED_DEMO_TOTAL_FIXTURE_RECORDS} fixtures`,
			);
			await expect(fixtureRow(patchWindow, 101)).toBeVisible();
			await configureOutput(desk, page, app, bench, api, showId);
			performanceBaseline = await captureProductDemoPerformance(page);
			await buildGroups(
				desk,
				page,
				demo,
				app,
				keypad,
				api,
				showId,
				canonical.patch.fixtures,
			);
			await buildPresetSetup(
				desk,
				demo,
				app,
				keypad,
				api,
				showId,
				canonical.patch.fixtures,
			);
			await buildDynamicsSetup(desk, page, demo, app, keypad, api, showId);
			await buildCueProgramming(
				desk,
				app,
				keypad,
				api,
				showId,
				canonical.patch.fixtures,
			);
			await demonstrateFixtureControls(desk, page, demo, app, keypad, api);
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
			if (!performanceBaseline) {
				throw new Error(
					"The canonical demo performance baseline is unavailable",
				);
			}
			const performanceEvidence = await finishProductDemoPerformance(
				page,
				performanceBaseline,
			);
			await fs.mkdir(path.dirname(PERFORMANCE), { recursive: true });
			await fs.writeFile(
				PERFORMANCE,
				`${JSON.stringify(performanceEvidence, null, 2)}\n`,
			);
			await testInfo.attach("canonical-demo-performance", {
				path: PERFORMANCE,
				contentType: "application/json",
			});
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

type TouchFixtureInput = {
	search: string;
	family: string;
	mode: string;
	name: string;
	fixtureId: string;
	count: number;
	address?: string;
};

async function addFixtureThroughTouchUi(
	desk: DeskDriver,
	page: Page,
	input: TouchFixtureInput,
) {
	await desk.click(
		page.getByRole("button", { name: "+ Add fixture", exact: true }),
	);
	const browser = page.locator(".fixture-browser-modal");
	await browser
		.getByRole("textbox", { name: "Search", exact: true })
		.fill(input.search);
	const fixtureColumn = browser
		.locator(".fixture-picker-columns > section")
		.nth(1);
	await desk.click(
		fixtureColumn.getByRole("button", {
			name: new RegExp(`^${escapeRegex(input.family)}\\b`),
		}),
	);
	const mode = browser.locator(".fixture-mode-detail select");
	const modeValue = await mode
		.locator("option")
		.evaluateAll(
			(options, label) =>
				options
					.find((option) => option.textContent?.startsWith(`${label} ·`))
					?.getAttribute("value") ?? null,
			input.mode,
		);
	if (!modeValue) throw new Error(`Missing ${input.family} mode ${input.mode}`);
	await mode.selectOption(modeValue);
	await desk.click(
		browser.locator(".fixture-mode-detail").getByRole("button", {
			name: "Add fixture",
			exact: true,
		}),
	);
	const placement = page.locator(".fixture-placement-modal");
	await placement
		.getByRole("textbox", { name: /^Fixture name\b/u })
		.fill(input.name);
	await placement
		.getByRole("textbox", { name: "Start fixture ID", exact: true })
		.fill(input.fixtureId);
	await placement
		.getByRole("textbox", { name: "Count", exact: true })
		.fill(String(input.count));
	if (input.address)
		await placement
			.getByRole("textbox", { name: /^Address \(universe\.address\)/u })
			.fill(input.address);
	await desk.click(
		placement.getByRole("button", {
			name: `Add ${input.count} fixtures`,
			exact: true,
		}),
	);
	await expect(placement).toBeHidden();
}

async function spreadFixtureLocationThroughTouchUi(
	desk: DeskDriver,
	patchWindow: Locator,
	firstFixture: number,
	lastFixture: number,
	axis: "X" | "Y" | "Z",
	keys: string[],
) {
	const first = fixtureRow(patchWindow, firstFixture);
	const last = fixtureRow(patchWindow, lastFixture);
	await desk.setDemoAction(
		`Spread Front Lights ${firstFixture}–${lastFixture} across Location ${axis}: ${keys
			.map((key) => `[${key}]`)
			.join(" ")}`,
	);
	await desk.click(first);
	await desk.click(last, { modifiers: ["Shift"] });
	const cell = { X: 12, Y: 13, Z: 14 }[axis];
	await desk.click(last.locator("td").nth(cell).getByRole("button"), {
		button: "right",
	});
	const edit = patchWindow
		.page()
		.locator(".patch-edit-modal")
		.filter({
			has: patchWindow.page().getByRole("heading", {
				name: `Set fixture location ${axis}`,
				exact: true,
			}),
		});
	const application = patchWindow.page().locator(".product-demo-application");
	await expectCenteredInDemoSurface(edit, application);
	await desk.click(edit.getByRole("button", { name: "Open number pad" }));
	const pad = patchWindow.page().getByRole("dialog", { name: `${axis} (m)` });
	await expectCenteredInDemoSurface(pad, application);
	for (const key of keys)
		await desk.click(pad.getByRole("button", { name: key, exact: true }));
	await expect(pad).toBeHidden();
	await expect(edit).toBeHidden();
}

function requiredFixture(fixtures: Map<number, any>, fixtureNumber: number) {
	const fixture = fixtures.get(fixtureNumber);
	if (!fixture) throw new Error(`Missing canonical fixture ${fixtureNumber}`);
	return fixture;
}

function fixtureAddress(fixture: any) {
	const patch = fixture.split_patches[0];
	if (!patch?.universe || !patch.address)
		throw new Error(`Fixture ${fixture.fixture_number} has no primary patch`);
	return `${patch.universe}.${patch.address}`;
}

async function fixtureIdentities(
	api: ApiDriver,
	numbers: Array<number | string>,
) {
	const patch = await api.patch();
	return numbers.map((number) => {
		const virtual = typeof number === "string" && number.startsWith("0.");
		const expected = virtual ? Number(number.slice(2)) : Number(number);
		const fixture = patch.fixtures.find((candidate: any) =>
			virtual
				? candidate.virtual_fixture_number === expected
				: candidate.fixture_number === expected,
		);
		if (!fixture) throw new Error(`Missing visible fixture ${number}`);
		return fixture.fixture_id;
	});
}

function escapeRegex(value: string) {
	return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

type ProductDemoPerformanceBaseline = {
	recordedAt: string;
	monotonicMs: number;
	frontend: FrontendPerformanceSnapshot;
};

async function captureProductDemoPerformance(
	page: Page,
): Promise<ProductDemoPerformanceBaseline> {
	const frontend = await page.evaluate(() => {
		const snapshot = window.__TOSKLIGHT_FRONTEND_PERFORMANCE__?.snapshot();
		if (!snapshot)
			throw new Error("Frontend performance diagnostics are unavailable");
		return snapshot;
	});
	return {
		recordedAt: new Date().toISOString(),
		monotonicMs: performance.now(),
		frontend,
	};
}

async function finishProductDemoPerformance(
	page: Page,
	baseline: ProductDemoPerformanceBaseline,
) {
	await page.evaluate(
		() =>
			new Promise<void>((resolve) =>
				requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
			),
	);
	const after = await captureProductDemoPerformance(page);
	const elapsedMs = Math.max(after.monotonicMs - baseline.monotonicMs, 1);
	const frames = after.frontend.stage.frames.slice(
		baseline.frontend.stage.frames.length,
	);
	const renders = after.frontend.stage.renders.slice(
		baseline.frontend.stage.renders.length,
	);
	const canvasTimestamps = frames
		.flatMap(({ settledCanvasSubmittedAt }) =>
			settledCanvasSubmittedAt === null ? [] : [settledCanvasSubmittedAt],
		)
		.sort((left, right) => left - right);
	const presentationGaps = canvasTimestamps
		.slice(1)
		.map((timestamp, index) => timestamp - canvasTimestamps[index]);
	const sourceToCanvas = frames.flatMap(({ sourceToSettledCanvasMs }) =>
		sourceToSettledCanvasMs === null ? [] : [sourceToSettledCanvasMs],
	);
	const renderDurations = renders.map(({ durationMs }) => durationMs);
	return {
		schema_version: 1,
		measurement_surface: "browser_playwright_product_demo",
		blocking_release_evidence: false,
		acceptance_gate: "separate_packaged_tauri",
		limitations: [
			"The release CI measurement uses Chromium rather than the packaged Tauri WebView.",
			"The separate packaged 343-instance acceptance remains authoritative for WebView Stage behavior.",
		],
		scene: {
			fixture_records: PLANNED_DEMO_TOTAL_FIXTURE_RECORDS,
			physical_instances: PLANNED_DEMO_TOTAL_PHYSICAL_INSTANCES,
			stage_visible: true,
		},
		window: {
			started_at: baseline.recordedAt,
			finished_at: after.recordedAt,
			elapsed_ms: elapsedMs,
		},
		stage: {
			frames: frames.length,
			renders: renders.length,
			presentation_rate_hz:
				presentationGaps.length === 0
					? null
					: 1_000 / average(presentationGaps),
			presentation_gap_ms: distribution(presentationGaps),
			source_to_settled_canvas_ms: distribution(sourceToCanvas),
			render_duration_ms: distribution(renderDurations),
			max_draw_calls: maximum(renders.map(({ calls }) => calls)),
			max_triangles: maximum(renders.map(({ triangles }) => triangles)),
		},
	};
}

function distribution(values: readonly number[]) {
	const sorted = [...values].sort((left, right) => left - right);
	return {
		samples: sorted.length,
		p50: percentile(sorted, 50),
		p95: percentile(sorted, 95),
		maximum: sorted.at(-1) ?? null,
	};
}

function percentile(values: readonly number[], percentage: number) {
	if (values.length === 0) return null;
	const rank = Math.ceil((percentage / 100) * values.length);
	return values[Math.max(0, rank - 1)];
}

function average(values: readonly number[]) {
	return values.reduce((total, value) => total + value, 0) / values.length;
}

function maximum(values: readonly number[]) {
	return values.length === 0 ? 0 : Math.max(...values);
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
	api: ApiDriver,
	showId: string,
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
	await desk.fastForward(
		"Configuring the remaining occupied universes through sACN and Art-Net routes.",
		async () => {
			for (let universe = 2; universe <= 8; universe++) {
				const sacn = universe % 2 === 0;
				await api.request("POST", "/api/v2/output-routes/actions", {
					request_id: crypto.randomUUID(),
					action: {
						type: "create",
						route_id: `demo-${sacn ? "sacn" : "artnet"}-${universe}`,
						route: {
							protocol: sacn ? "sacn" : "art_net",
							logical_universe: universe,
							destination_universe: universe,
							delivery_mode: "unicast",
							destination: `127.0.0.1:${sacn ? bench.sacn.port : bench.artnet.port}`,
							enabled: true,
							minimum_slots: 128,
						},
					},
				});
			}
		},
	);
	await expect
		.poll(async () => (await api.showObjects(showId, "route")).length)
		.toBe(8);
}

async function buildGroups(
	desk: DeskDriver,
	page: Page,
	demo: Locator,
	app: Locator,
	keypad: Locator,
	api: ApiDriver,
	showId: string,
	fixtures: readonly any[],
) {
			await desk.titleCard(
				"GROUP SETUP · STAGE PROFILES",
				"Select Profile Stage fixtures (101–128) and store them as Group 2: [FIXTURE] [101] [THRU] [128] [ENTER] [RECORD] [Group 2 tile]",
			);
	await openGroups(desk, keypad);
	const groups = app.locator(".group-card");
	await expect(groups.first()).toBeVisible();
	await clearSelection(desk, keypad, api);
	await keypadCommand(desk, keypad, [
		"1",
		"0",
		"1",
		"TRU",
		"1",
		"2",
		"8",
		"ENT",
	]);
	await desk.setDemoAction(
			"Store the selected Profile Stage fixtures as Group 2: [RECORD] [Group 2 tile]",
		);
	await desk.click(keypad.getByRole("button", { name: "RECORD", exact: true }));
	await desk.click(groupTile(app, 2));
	await expect
		.poll(async () => api.showObject(showId, "group", "2"))
		.not.toBeNull();

	await desk.titleCard(
		"GROUP SETUP · WASH STAGE",
		"Create the Wash Stage Group entirely through the faster simulated hardware sequence.",
	);
	await clearSelection(desk, keypad, api);
	await keypadCommand(desk, keypad, [
		"2",
		"0",
		"1",
		"TRU",
		"2",
		"2",
		"6",
		"ENT",
	]);
	await keypadCommand(desk, keypad, ["RECORD", "GRP", "6", "ENT"]);

	await desk.titleCard(
		"GROUP SETUP · FIRST-LEVEL GROUPS",
		"Keep using the simulated hardware controls at the compact recording pace.",
	);
	for (const [selection, destination] of [
		[["1", "2", "9", "TRU", "1", "5", "0", "ENT"], 3],
		[["1", "5", "1", "TRU", "1", "5", "4", "ENT"], 4],
		[["GRP", "2", "+", "GRP", "3", "+", "GRP", "4", "ENT"], 1],
		[["2", "2", "7", "TRU", "2", "4", "2", "ENT"], 7],
		[["2", "4", "3", "TRU", "2", "4", "6", "ENT"], 8],
		[["GRP", "6", "+", "GRP", "7", "+", "GRP", "8", "ENT"], 5],
		[["3", "0", "1", "TRU", "3", "1", "6", "ENT"], 10],
		[["3", "1", "7", "TRU", "4", "1", "6", "ENT"], 11],
		[["4", "1", "7", "TRU", "4", "3", "2", "ENT"], 12],
		[["GRP", "1", "0", "+", "GRP", "1", "1", "+", "GRP", "1", "2", "ENT"], 9],
	] as const) {
		await clearSelection(desk, keypad, api);
		await keypadCommand(desk, keypad, [...selection]);
		await keypadCommand(desk, keypad, [
			"RECORD",
			"GRP",
			...digits(destination),
			"ENT",
		]);
	}

	await desk.titleCard(
		"GROUP SETUP · SHOW PROFILE",
		"Combine Profile Stage and Profile Audience, then Record to a Group tile and name it through touch input.",
	);
	await clearSelection(desk, keypad, api);
	await keypadCommand(desk, keypad, ["GRP", "2", "+", "GRP", "3", "ENT"]);
	await desk.click(keypad.getByRole("button", { name: "RECORD", exact: true }));
	await desk.click(groupTile(app, 13));
	await keypadCommand(desk, keypad, ["SET", "GRP", "1", "3", "ENT"]);
	const properties = page.getByRole("dialog", { name: "Group properties" });
	await touchTypeText(
		desk,
		properties.getByLabel("Group name"),
		"Show Profile",
	);
	await desk.click(
		properties.getByRole("button", { name: "Save group", exact: true }),
	);

	await desk.titleCard(
		"GROUP SETUP · ODD AND EVEN",
		"Derive the Show Profile odd and even selections and store both through simulated hardware buttons.",
	);
	await clearSelection(desk, keypad, api);
	await keypadCommand(desk, keypad, ["GRP", "1", "3", "DIV", "ENT"]);
	await keypadCommand(desk, keypad, ["RECORD", "GRP", "2", "1", "ENT"]);
	await clearSelection(desk, keypad, api);
	await keypadCommand(desk, keypad, ["GRP", "1", "3", "DIV", "DIV", "ENT"]);
	await keypadCommand(desk, keypad, ["RECORD", "GRP", "2", "2", "ENT"]);
	await desk.fastForward(
		"Creating the remaining Show, auxiliary, odd/even, and utility Groups via API.",
		async () => {
			await installPlannedDemoGroups(api, showId, fixtures);
			// Establish the physical page outside the visible assignment so the first
			// touched Group Master still lands on a genuinely empty slot.
			await keypadCommand(desk, keypad, [
				"SET",
				"GRP",
				"3",
				"6",
				"AT",
				"1",
				".",
				"1",
				"2",
				"ENT",
			]);
			const cueListId = crypto.randomUUID();
			await api.seedShowObject(
				showId,
				"cue_list",
				cueListId,
				cueListScaffold(cueListId, "ACL 1"),
				0,
			);
			const playback = await api.showObject(showId, "playback", "12");
			if (!playback) throw new Error("Playback 12 page preparation failed");
			await api.seedShowObject(
				showId,
				"playback",
				"12",
				cueListPlayback(12, "ACL 1", cueListId),
				playback.revision,
			);
		},
	);
	await expect
		.poll(async () => (await api.showObjects(showId, "group")).length)
		.toBe(38);

	await desk.titleCard(
		"GROUP SETUP · GROUP MASTERS",
		"Assign one Group Master by touch, a second by command line, then fast-forward the remaining assignments.",
	);
	await openGroups(desk, keypad);
	await desk.click(keypad.getByRole("button", { name: "SET", exact: true }));
	await desk.click(groupTile(app, 21));
	await toggleProgrammerPlaybacks(desk, demo);
	await desk.click(demo.locator('[data-playback-slot="1"]'));
	await expect
		.poll(async () => playbackTarget(api, showId, 1))
		.toMatchObject({
			type: "group",
			group_id: "21",
		});
	await toggleProgrammerPlaybacks(desk, demo);
	await keypadCommand(desk, keypad, [
		"SET",
		"GRP",
		"2",
		"2",
		"AT",
		"1",
		".",
		"2",
		"ENT",
	]);
	await expect
		.poll(async () => playbackTarget(api, showId, 2))
		.toMatchObject({
			type: "group",
			group_id: "22",
		});
	await desk.fastForward(
		"Assigning the Show LED, Show Wash, All ACLs, and Blinders Group Masters.",
		() => installRemainingGroupMasters(api, showId),
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
	await expectCenteredInDemoSurface(dialog, app);
	for (const name of ["Lamps On", "Fan Auto", "Reset", "Lamp Off"]) {
		const action = dialog.getByRole("button", { name, exact: true });
		await expect(action).toBeVisible();
		await desk.click(action);
	}
	await desk.click(dialog.getByRole("button", { name: "×", exact: true }));
}

async function buildDynamicsSetup(
	desk: DeskDriver,
	page: Page,
	demo: Locator,
	app: Locator,
	keypad: Locator,
	api: ApiDriver,
	showId: string,
) {
	await desk.titleCard(
		"DYNAMICS · SHOW PROFILE CIRCLE",
		"Create a two-lane Circle: Pan on Sinus and Tilt on Cosinus, bound to Show Profile.",
	);
	await clearProgrammer(desk, keypad, api);
	await keypadCommand(desk, keypad, ["GRP", "1", "3", "ENT"]);
	await openBuiltIn(desk, app, "Dynamics");
	await createDynamicThroughTouch(desk, page, app, 19, "Pan");
	await desk.click(
		app.getByRole("button", { name: "+ Add Lane", exact: true }),
	);
	await chooseDynamicAttribute(desk, page, "Tilt");
	await desk.click(app.getByRole("button", { name: /^Select lane 2, Tilt$/ }));
	await chooseDynamicCurve(desk, page, app, "Cosinus");
	await configureDynamicThroughTouch(desk, page, app, "Show Profile Circle");
	await desk.click(
		app.getByRole("button", { name: "← Back to Pool", exact: true }),
	);

	await desk.titleCard(
		"DYNAMICS · SHOW PROFILE PWM",
		"Create the Profile Show PWM chaser through the same production Dynamic editor.",
	);
	await clearSelection(desk, keypad, api);
	await keypadCommand(desk, keypad, ["GRP", "1", "3", "ENT"]);
	await createDynamicThroughTouch(desk, page, app, 1, "Intensity");
	await chooseDynamicCurve(desk, page, app, "PWM");
	await configureDynamicThroughTouch(desk, page, app, "Show Profile PWM");
	await desk.click(
		app.getByRole("button", { name: "← Back to Pool", exact: true }),
	);
	const visibleDynamicIds = await dynamicIdentities(api, showId, [1, 19]);
	const definitions = await desk.fastForward(
		"Creating the remaining 28 Dynamics and reconciling speed, phase, and curve details.",
		() =>
			installPlannedDemoDynamics(api, showId, {
				assignVirtualPlaybacks: false,
			}),
	);
	expect(await dynamicIdentities(api, showId, [1, 19])).toEqual(
		visibleDynamicIds,
	);

	await desk.titleCard(
		"DYNAMICS · VIRTUAL PLAYBACK DESKTOP",
		"Create a Busking desktop, add Virtual Playbacks, and assign the two hand-built Dynamics.",
	);
	const pane = await createVirtualPlaybackDesktop(desk, page);
	await assignVirtualDynamic(
		desk,
		page,
		pane,
		keypad,
		1,
		"Show Profile PWM",
		1,
	);
	await assignVirtualDynamic(
		desk,
		page,
		pane,
		keypad,
		2,
		"Show Profile Circle",
		19,
	);
	await desk.fastForward(
		"Assigning every remaining Dynamic to its stable Virtual Playback and completing the Programming and Theater desktops.",
		async () => {
			await installPlannedDemoDynamicPlaybacks(api, showId, definitions);
			await installPlannedDemoLayout(api, showId);
		},
	);
	await expect
		.poll(async () => (await api.showObjects(showId, "dynamic")).length)
		.toBe(30);
}

async function buildPresetSetup(
	desk: DeskDriver,
	demo: Locator,
	app: Locator,
	keypad: Locator,
	api: ApiDriver,
	showId: string,
	fixtures: readonly any[],
) {
	await desk.titleCard(
		"PRESET SETUP · FIRST COLOR",
		"Bring the Show Profiles on, program the first Color visibly, and Record it through the simulated desk and touch pool.",
	);
	await clearProgrammer(desk, keypad, api);
	await desk.setDemoAction(
		"Bring the Show Profiles to full intensity, set Red to 100%, and store Color preset 2.1: [GRP] [1] [3] [AT] [1] [0] [0] [ENTER]",
	);
	await keypadCommand(desk, keypad, ["GRP", "1", "3", "ENT"]);
	await keypadCommand(desk, keypad, ["AT", "1", "0", "0", "ENT"]);
	await setEncoderValue(desk, demo, "Color", "Red", ["1", "0", "0"]);
	await openBuiltIn(desk, app, "Presets");
	const presets = app.locator(".preset-pool-window");
	await desk.click(presets.getByRole("button", { name: "Color", exact: true }));
	await desk.click(keypad.getByRole("button", { name: "RECORD", exact: true }));
	await desk.click(presetTile(presets, "2.1"));

	await desk.titleCard(
		"PRESET SETUP · FIRST POSITION",
		"Bring the Show Profiles on, program the first Position visibly, and store it to the Position pool.",
	);
	await clearProgrammer(desk, keypad, api);
	await desk.setDemoAction(
		"Bring the Show Profiles to full intensity, aim Pan and Tilt, and store Position preset 3.1: [GRP] [1] [3] [AT] [1] [0] [0] [ENTER]",
	);
	await keypadCommand(desk, keypad, ["GRP", "1", "3", "ENT"]);
	await keypadCommand(desk, keypad, ["AT", "1", "0", "0", "ENT"]);
	await setEncoderValue(desk, demo, "Position", "Pan", ["5", "0"]);
	await setEncoderValue(desk, demo, "Position", "Tilt", ["2", "5"]);
	await openBuiltIn(desk, app, "Presets");
	await desk.click(
		presets.getByRole("button", { name: "Position", exact: true }),
	);
	await desk.click(keypad.getByRole("button", { name: "RECORD", exact: true }));
	await desk.click(presetTile(presets, "3.1"));

	await desk.titleCard(
		"PRESET SETUP · FIRST BEAM",
		"Activate the first optical Beam value and store the first Beam preset through touch.",
	);
	await clearProgrammer(desk, keypad, api);
	await keypadCommand(desk, keypad, ["GRP", "1", "ENT"]);
	await setEncoderValue(desk, demo, "Beam", "Prism 1", ["0"]);
	await openBuiltIn(desk, app, "Presets");
	await desk.click(presets.getByRole("button", { name: "Beam", exact: true }));
	await desk.click(keypad.getByRole("button", { name: "RECORD", exact: true }));
	await desk.click(presetTile(presets, "4.1"));
	await desk.fastForward(
		"Programming the remaining Color, Position, and Beam presets across every compatible fixture.",
		() => installPlannedDemoPresets(api, showId, fixtures),
	);
	await expect
		.poll(async () => (await api.showObjects(showId, "preset")).length)
		.toBe(30);
}

async function buildCueProgramming(
	desk: DeskDriver,
	app: Locator,
	keypad: Locator,
	api: ApiDriver,
	showId: string,
	fixtures: readonly any[],
) {
	await desk.titleCard(
		"CUELIST PROGRAMMING · ACL 1",
		"Program the first ACL Cuelist entirely through the simulated hardware keypad controls.",
	);
	await clearProgrammer(desk, keypad, api);
	await keypadCommand(desk, keypad, [
		"GRP",
		"3",
		"1",
		"AT",
		"1",
		"0",
		"0",
		"ENT",
	]);
	await keypadCommand(desk, keypad, [
		"RECORD",
		"SET",
		"1",
		".",
		"1",
		"2",
		"ENT",
	]);
	await expect
		.poll(async () => {
			const target = await playbackTarget(api, showId, 12);
			return target?.type === "cue_list" ? target.cue_list_id : null;
		})
		.not.toBeNull();
	const visibleTarget = await playbackTarget(api, showId, 12);
	const visibleCueListId =
		visibleTarget?.type === "cue_list" ? visibleTarget.cue_list_id : null;
	await desk.fastForward(
		"Programming Start, ACL 2–4, Hazer, and ACL Chase Cuelists and completing their Playback assignments.",
		() => installPlannedDemoPlaybacks(api, showId, fixtures),
	);
	await openBuiltIn(desk, app, "Cuelists");
	await expect(app.locator(".cuelist-window")).toContainText("Start");
	expect(await api.showObjects(showId, "cue_list")).toHaveLength(7);
	expect(await api.showObjects(showId, "playback")).toHaveLength(13);
	const adopted = await playbackTarget(api, showId, 12);
	expect(adopted).toMatchObject({
		type: "cue_list",
		cue_list_id: visibleCueListId,
	});
	if (!visibleCueListId) {
		throw new Error("The visible ACL Cuelist identity was not retained");
	}
	await desk.titleCard(
		"SHOW COMPLETE",
		"The complete patch, outputs, Groups, presets, Dynamics, desktops, Cuelists, and Playbacks are now ready for operation.",
	);
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

async function selectPatchLayer(
	desk: DeskDriver,
	patchWindow: Locator,
	name: string,
) {
	await desk.setDemoAction(`Select the ${name} layer before adding its fixtures.`);
	await desk.click(
		patchWindow
			.locator(".patch-layers button")
			.filter({ hasText: name })
			.first(),
	);
}

async function openBuiltIn(desk: DeskDriver, app: Locator, name: string) {
	const toggle = app.getByRole("button", {
		name: "Desktops / Built-ins",
		exact: true,
	});
	if ((await toggle.getAttribute("data-dock-mode")) === "desks") {
		await desk.click(toggle);
	}
	await desk.click(
		app
			.getByRole("navigation", { name: "Built-ins", exact: true })
			.getByRole("button", { name, exact: true }),
	);
}

async function openGroups(desk: DeskDriver, keypad: Locator) {
	await desk.click(keypad.getByRole("button", { name: "SHIFT", exact: true }));
	await desk.click(keypad.getByRole("button", { name: "1", exact: true }));
}

async function toggleProgrammerPlaybacks(desk: DeskDriver, demo: Locator) {
	await desk.click(demo.getByRole("button", { name: /PROG\. PLAYBK$/ }));
}

function groupTile(app: Locator, number: number) {
	return app.locator(`.group-card[data-pool-slot-id="${number}"]`).first();
}

function presetTile(presets: Locator, address: string) {
	return presets
		.locator(`.preset-card[data-pool-slot-id="${address}"]`)
		.first();
}

async function setEncoderValue(
	desk: DeskDriver,
	demo: Locator,
	family: string,
	label: string,
	percentage: string[],
) {
	const controls = demo
		.locator(".product-demo-application .parameter-controls")
		.first();
	await desk.click(
		controls
			.getByRole("button", {
				name: new RegExp(`^${escapeRegex(family)}(?: \\d+ of \\d+)?$`),
			})
			.first(),
	);
	await desk.click(
		controls.getByRole("button", {
			name: new RegExp(`^Encoder \\d+: ${escapeRegex(label)},`),
		}),
	);
	const dialog = demo.page().getByRole("dialog", {
		name: /^Encoder \d+ value$/,
	});
	for (const token of [...percentage, "ENTER"])
		await desk.click(dialog.getByRole("button", { name: token, exact: true }));
	await expect(dialog).toBeHidden();
}

async function createDynamicThroughTouch(
	desk: DeskDriver,
	page: Page,
	app: Locator,
	poolNumber: number,
	attribute: string,
) {
	await desk.click(app.locator(".dynamic-pool-card").nth(poolNumber - 1));
	await chooseDynamicAttribute(desk, page, attribute);
	await expect(app.locator(".dynamics-editor")).toBeVisible();
}

async function chooseDynamicAttribute(
	desk: DeskDriver,
	page: Page,
	attribute: string,
) {
	const chooser = page.getByRole("dialog", { name: "Select lane attribute" });
	await expect(chooser).toBeVisible();
	await desk.click(
		chooser.getByRole("button", {
			name: attribute,
			exact: true,
		}),
	);
}

async function chooseDynamicCurve(
	desk: DeskDriver,
	page: Page,
	app: Locator,
	curve: string,
) {
	await desk.click(app.getByRole("button", { name: /^Curve function:/ }));
	const chooser = page.getByRole("dialog", { name: "Choose curve function" });
	await desk.click(
		chooser.getByRole("button", {
			name: new RegExp(`^${escapeRegex(curve)}\\b`),
		}),
	);
}

async function configureDynamicThroughTouch(
	desk: DeskDriver,
	page: Page,
	app: Locator,
	name: string,
) {
	await desk.click(app.getByRole("button", { name: "Settings", exact: true }));
	const settings = page.getByRole("dialog", { name: "Dynamic Settings" });
	await settings.getByLabel("Name").fill(name);
	await desk.click(settings.getByRole("tab", { name: "Targets", exact: true }));
	await desk.click(
		settings.getByRole("button", { name: "Take Selection", exact: true }),
	);
	await desk.click(
		settings.getByRole("button", { name: "Close settings", exact: true }),
	);
}

async function dynamicIdentities(
	api: ApiDriver,
	showId: string,
	poolNumbers: readonly number[],
) {
	const dynamics = await api.showObjects<any>(showId, "dynamic");
	return poolNumbers.map((number) => {
		const dynamic = dynamics.find(
			(candidate) => candidate.body.pool_number === number,
		);
		if (!dynamic) throw new Error(`Missing visible Dynamic ${number}`);
		return dynamic.id;
	});
}

async function createVirtualPlaybackDesktop(desk: DeskDriver, page: Page) {
	const toggle = page.getByRole("button", {
		name: "Desktops / Built-ins",
		exact: true,
	});
	if ((await toggle.getAttribute("data-dock-mode")) !== "desks")
		await desk.click(toggle);
	await desk.click(
		page.getByRole("button", { name: "New desktop", exact: true }),
	);
	const active = page.locator("[data-desktop-id][aria-current=page]");
	await active.hover();
	await page.mouse.down();
	await page.waitForTimeout(700);
	await page.mouse.up();
	const settings = page.getByRole("dialog", { name: "Desktop settings" });
	await touchTypeText(desk, settings.getByLabel("Name"), "Busking");
	await desk.click(settings.getByRole("button", { name: "×", exact: true }));
	await desk.click(page.locator(".desk-grid"));
	await expect(
		page.getByRole("heading", { name: "Open Window" }),
	).toBeVisible();
	await desk.click(
		page.getByRole("button", { name: "Virtual Playbacks", exact: true }),
	);
	const pane = page
		.locator(".desk-pane")
		.filter({ hasText: "Virtual Playbacks" });
	await expect(pane).toBeVisible();
	return pane;
}

async function assignVirtualDynamic(
	desk: DeskDriver,
	page: Page,
	pane: Locator,
	keypad: Locator,
	cell: number,
	name: string,
	poolNumber: number,
) {
	await desk.click(keypad.getByRole("button", { name: "SET", exact: true }));
	await desk.click(
		pane.getByRole("button", {
			name: new RegExp(
				`Virtual playback ${1000 + cell} page 1 cell ${cell} empty`,
			),
		}),
	);
	const modal = page.getByRole("dialog", { name: "Playback Configuration" });
	await desk.click(modal.getByRole("radio", { name: "Dynamic", exact: true }));
	await desk.click(
		modal.getByRole("radio", {
			name: new RegExp(`^Dynamic ${poolNumber} · ${escapeRegex(name)}`),
		}),
	);
	await modal.getByLabel("Playback name").fill(name);
	await desk.click(modal.getByRole("button", { name: "Apply", exact: true }));
	await expect(modal).toBeHidden();
}

function digits(value: number) {
	return String(value).split("");
}

async function touchTypeText(desk: DeskDriver, input: Locator, value: string) {
	const controls = input.locator("..");
	await desk.click(controls.getByRole("button", { name: "Clear input" }));
	await desk.click(controls.getByRole("button", { name: "Open keyboard" }));
	const keyboard = input.page().locator(".modal-text-keyboard");
	for (const character of value) {
		if (character === " ") {
			await desk.click(keyboard.locator("button.space"));
			continue;
		}
		if (character.toUpperCase() === character) {
			await desk.click(
				keyboard.getByRole("button", { name: "Shift", exact: true }),
			);
		}
		await desk.click(
			keyboard.locator(`[data-keyboard-code="Key${character.toUpperCase()}"]`),
		);
	}
	await desk.click(
		keyboard.getByRole("button", { name: "Enter · Confirm", exact: true }),
	);
}

async function playbackTarget(api: ApiDriver, showId: string, number: number) {
	const playback = (await api.showObjects<any>(showId, "playback")).find(
		(candidate) => candidate.body.number === number,
	);
	return playback?.body.target ?? null;
}

async function installRemainingGroupMasters(api: ApiDriver, showId: string) {
	const assignments = [
		[3, "Show LED", "17"],
		[4, "Show Wash", "15"],
		[5, "All ACLs", "35"],
		[6, "Blinders", "36"],
	] as const;
	const existingPlaybacks = await api.showObjects<any>(showId, "playback");
	for (const [number, name, groupId] of assignments) {
		const existing = existingPlaybacks.find(
			(candidate) => candidate.body.number === number,
		);
		await api.seedShowObject(
			showId,
			"playback",
			existing?.id ?? String(number),
			groupMasterPlayback(number, name, groupId),
			existing?.revision ?? 0,
		);
	}
	const pages = await api.showObjects<any>(showId, "playback_page");
	const page = pages.find((candidate) => candidate.body.number === 1);
	await api.seedShowObject(
		showId,
		"playback_page",
		page?.id ?? "1",
		{
			number: 1,
			name: page?.body.name ?? "Busking",
			slots: {
				...(page?.body.slots ?? {}),
				1: 1,
				2: 2,
				3: 3,
				4: 4,
				5: 5,
				6: 6,
				12: 12,
			},
			virtual_playbacks: page?.body.virtual_playbacks ?? {},
		},
		page?.revision ?? 0,
	);
}

function groupMasterPlayback(number: number, name: string, groupId: string) {
	return {
		number,
		name,
		target: { type: "group", group_id: groupId },
		buttons: ["select", "flash", "select_dereferenced"],
		button_count: 3,
		fader: "master",
		has_fader: true,
		go_activates: true,
		auto_off: true,
		xfade_millis: 0,
		color: "#20c997",
		flash_release: "release_all",
		protect_from_swap: false,
	};
}

function cueListScaffold(id: string, name: string) {
	return {
		id,
		name,
		cues: [
			{
				id: crypto.randomUUID(),
				number: 1,
				name: "Cue 1",
				cue_only: false,
				changes: [],
				group_changes: [],
				fade_millis: 0,
				delay_millis: 0,
				trigger: { type: "manual" },
			},
		],
		mode: "sequence",
		priority: 0,
		looped: false,
		intensity_priority_mode: "htp",
		wrap_mode: "off",
		restart_mode: "first_cue",
		force_cue_timing: false,
		disable_cue_timing: false,
		chaser_step_millis: 1_000,
		chaser_xfade_millis: 0,
		speed_group: null,
		speed_multiplier: 1,
	};
}

function cueListPlayback(number: number, name: string, cueListId: string) {
	return {
		...groupMasterPlayback(number, name, "unused"),
		target: { type: "cue_list", cue_list_id: cueListId },
		buttons: ["go", "go_minus", "flash"],
	};
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
