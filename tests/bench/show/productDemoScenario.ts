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
import { BrowserDesktops } from "../window-system/desktopScenario";
import { PaneType } from "../window-system/paneTypes";

/**
 * Canonical edit contract for the narrated product demo.
 *
 * Keep this JSON-shaped: the values are intentionally easy to revise after a
 * voice-over pass. Durations and action pacing use whole 25 fps frames so the
 * final edit can be reproduced exactly instead of inheriting network/test time.
 * Patch strings use the desk's THRU notation and are expanded in target order.
 */
export const PRODUCT_DEMO_SCRIPT = {
	fps: 25,
	transitionFrames: 15,
	sections: [
		{ id: "intro", marker: "ToskLight", title: "ToskLight", frames: 175 },
		{
			id: "show-setup",
			marker: "SHOW SETUP",
			title: "Show Setup",
			frames: 6_250,
		},
		{
			id: "outputs",
			marker: "OUTPUT CONFIGURATION",
			title: "Output Configuration",
			frames: 875,
		},
		{
			id: "groups",
			marker: "GROUP SETUP",
			title: "Group Setup",
			frames: 3_250,
		},
		{
			id: "presets",
			marker: "PRESET SETUP",
			title: "Preset Setup",
			frames: 1_625,
		},
		{ id: "dynamics", marker: "DYNAMICS", title: "Dynamics", frames: 1_625 },
		{
			id: "cuelists",
			marker: "CUELIST PROGRAMMING",
			title: "Cuelist Programming",
			frames: 1_750,
		},
		{
			id: "complete",
			marker: "SHOW COMPLETE",
			title: "Show Complete",
			frames: 250,
		},
		{
			id: "fixture-controls",
			marker: "BUILT-IN FIXTURE CONTROL ACTIONS",
			title: "Fixture Controls",
			frames: 625,
		},
		{ id: "busking", marker: "BUSKING", title: "Busking", frames: 1_000 },
		{
			id: "preloading",
			marker: "PRELOADING",
			title: "Preloading",
			frames: 750,
		},
		{ id: "final", marker: "ACL CHASER", title: "Final Look", frames: 500 },
	],
	pacing: {
		titleCardFrames: 125,
		searchClearHoldFrames: 25,
		searchCharacterFrames: 3,
		sceneryItemFrames: 3,
		lightingItemFrames: 5,
		postSpreadHoldFrames: 75,
	},
	patch: {
		layers: [
			"Stage & Venue",
			"Trusses",
			"Profile Stage",
			"Profile Audience",
			"Profile Auxilliary",
			"Wash Stage",
			"Wash Audience",
			"Wash Auxilliary",
			"LED PAR Stage",
			"LED PAR Audience",
			"LED PAR Auxilliary",
			"Front Lights",
			"Front Profiles",
			"ACLs & Blinder",
		],
		addressBands: {
			dimmers: "1.1 THRU 1.64",
			ledPars: "1.65 THRU 1.256",
			movingLights: "1.257 THRU 1.508",
			hazers: "1.509 THRU 1.512",
		},
		placements: [
			{
				targets: "1 THRU 4",
				location: { x: "-3.8 THRU -2.5", y: "-3", z: "4.15" },
			},
			{
				targets: "5 THRU 8",
				location: { x: "2.5 THRU 3.8", y: "-3", z: "4.15" },
			},
			{
				targets: "601 primary THRU multipatch 7",
				location: { x: "-1 THRU 1", y: "4", z: "4.4" },
				rotation: { x: "0", y: "-18 THRU 18", z: "0" },
			},
			{
				targets: "101",
				location: { x: "-4", y: "4", z: "4" },
			},
		],
		backCurtain: { x: "-2.5 THRU 2.5", y: 4.35, z: 2.0 },
		movingFixtureRotation: { x: 0, y: 0, z: 0 },
	},
} as const;

const { artifactPaths } = artifactResolver;
const RAW_VIDEO = path.join(
	artifactPaths.visual,
	"product-demo",
	"tosklight-product-demo-raw.webm",
);
const EDIT_TIMELINE = path.join(
	artifactPaths.visual,
	"product-demo",
	"product-demo-edit-timeline.json",
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
	new URL(
		"../../../apps/light-desktop/src-tauri/icons/icon.png",
		import.meta.url,
	),
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
		if (RECORDING)
			process.env.LIGHT_VISUAL_TITLE_CARD_PAUSE = String(
				framesToMillis(PRODUCT_DEMO_SCRIPT.pacing.titleCardFrames),
			);
		const api = this.api;
		const bench = this.bench;
		let desk = this.desk;
		let page = this.page;
		const testInfo = this.testInfo;
		testInfo.setTimeout(RECORDING ? 2_700_000 : 300_000);
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
		let recordingStartedAtMillis = 0;
		let recordingEndedAtMillis = 0;
		const titleMarkers = new Map<string, number>();
		let stopObservingTitleCards = () => {};
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
			if (RECORDING) {
				const setupPage = page;
				page = await setupPage.context().newPage();
				recordingStartedAtMillis = Date.now();
				desk = this.desk.fork(page);
				stopObservingTitleCards = desk.observeTitleCards((title) => {
					const section = PRODUCT_DEMO_SCRIPT.sections.find(
						(candidate) =>
							title === candidate.marker ||
							title.startsWith(`${candidate.marker} ·`),
					);
					if (section && !titleMarkers.has(section.id))
						titleMarkers.set(section.id, Date.now() - recordingStartedAtMillis);
				});
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
			const stageWindow = stage.locator(
				"xpath=ancestor::*[contains(@class,'stage-window')][1]",
			);
			await expect(stageWindow).toHaveAttribute(
				"data-live-visualization-state",
				"ready",
				{ timeout: 20_000 },
			);
			await expect(stageWindow).not.toHaveAttribute(
				"data-visualization-revision",
				"",
			);
			await desk.productIntro();
			await desk.titleCard(
				"SHOW SETUP",
				"Let’s start with an empty show. We build the venue and rig through the Touch UI. We will fast forward through repetitive work.",
				framesToMillis(PRODUCT_DEMO_SCRIPT.pacing.titleCardFrames),
			);
			await ensurePlannedDemoFixtureLibrary(api);
			const profiles = (await api.fixtureProfilesSnapshot())
				.profiles as FixtureProfile[];
			const desiredPatch = createPlannedDemoPatchInputs(
				profiles,
				{},
				PRODUCT_DEMO_SCRIPT.patch,
			);
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
			await addPatchLayerThroughTouchUi(desk, page, "Stage & Venue");
			desk.setRecordingClickPace("rapid");
			await addPatchLayerThroughTouchUi(desk, page, "Trusses");
			desk.setRecordingClickPace("compact");
			await desk.setDemoAction(
				"Create the remaining production Patch layers at the accelerated Touch UI pace.",
			);
			for (const layer of PRODUCT_DEMO_SCRIPT.patch.layers.slice(2))
				await addPatchLayerThroughTouchUi(desk, page, layer);
			const layers = Object.fromEntries(
				(await api.showObjects<any>(showId, "patch_layer")).map((layer) => [
					layer.body.name,
					layer.id,
				]),
			);
			await selectPatchLayer(desk, patchWindow, "Trusses");
			for (let truss = 1; truss <= 3; truss++) {
				await addFixtureThroughTouchUi(desk, page, {
					search: "Four-Point Truss",
					family: "Four-Point Truss",
					mode: "2 m",
					name:
						truss === 1
							? "Vectra Segment 1"
							: `${["Back", "Mid", "Front"][truss - 1]} Truss Segment 1`,
					fixtureId: `0.${truss}`,
					count: 1,
					leaveDefaultFixtureIdAndCount: truss === 1,
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
					if (truss === 1 && segment === 1) desk.setRecordingClickPace("rapid");
				}
			}
			desk.setRecordingClickPace("compact");
			const trussIdentities = await fixtureIdentities(api, [
				"0.1",
				"0.2",
				"0.3",
			]);
			await selectPatchLayer(desk, patchWindow, "Stage & Venue");
			await desk.fastForward(
				"Adding the stage elements, curtain, back and side railings, and vertical pipes.",
				() =>
					installPlannedDemoScenery(api, showId, layers, {
						backCurtain: PRODUCT_DEMO_SCRIPT.patch.backCurtain,
						progressive: true,
						onItem: () =>
							RECORDING
								? page.waitForTimeout(
										framesToMillis(
											PRODUCT_DEMO_SCRIPT.pacing.sceneryItemFrames,
										),
									)
								: Promise.resolve(),
					}),
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
				slowSearch: true,
				visibleModeSelection: true,
				touchTypePlacement: true,
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
				valuePadKeys(demoPatchPlacement("1 THRU 4").location.x),
			);
			await expect
				.poll(async () => {
					const fixtures = (await api.patch()).fixtures;
					return [1, 2, 3, 4].map(
						(number) =>
							fixtures.find((fixture: any) => fixture.fixture_number === number)
								?.location?.x,
					);
				})
				.toEqual([-3_800, -3_367, -2_933, -2_500]);
			await page.waitForTimeout(
				framesToMillis(PRODUCT_DEMO_SCRIPT.pacing.postSpreadHoldFrames),
			);
			await spreadFixtureLocationThroughTouchUi(
				desk,
				patchWindow,
				1,
				4,
				"Y",
				valuePadKeys(demoPatchPlacement("1 THRU 4").location.y),
				false,
			);
			await spreadFixtureLocationThroughTouchUi(
				desk,
				patchWindow,
				1,
				4,
				"Z",
				valuePadKeys(demoPatchPlacement("1 THRU 4").location.z),
				false,
			);
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
				if (instance === 1) desk.setRecordingClickPace("steady");
			}
			desk.setRecordingClickPace("compact");
			const aclPrimaryRow = fixtureRow(patchWindow, 601);
			const aclPhysicalRows = aclPrimaryRow.locator(
				"xpath=following-sibling::tr[contains(@class,'multipatch-row')]",
			);
			await expect(aclPhysicalRows).toHaveCount(7);
			await desk.click(aclPrimaryRow);
			await desk.click(aclPhysicalRows.last(), { modifiers: ["Shift"] });
			const aclPlacement = demoPatchPlacement("601 primary THRU multipatch 7");
			for (const [kind, axis, keys] of [
				["location", "X", valuePadKeys(aclPlacement.location.x)],
				["location", "Y", valuePadKeys(aclPlacement.location.y)],
				["location", "Z", valuePadKeys(aclPlacement.location.z)],
				["rotation", "Y", valuePadKeys(aclPlacement.rotation?.y ?? "0")],
			] as const)
				await spreadPhysicalPatchVectorThroughTouchUi(
					desk,
					aclPhysicalRows.last(),
					kind,
					axis,
					keys,
				);
			const profile = requiredFixture(desiredByNumber, 101);
			await selectPatchLayer(desk, patchWindow, "Profile Stage");
			await addFixtureThroughTouchUi(desk, page, {
				search: "Robin DLS Profile",
				family: "Robin DLS Profile",
				mode: "Mode 3",
				name: "Profile Stage 1",
				fixtureId: "101",
				count: 7,
				address: fixtureAddress(profile),
				visibleModeSelection: true,
			});
			const firstMoverPlacement = demoPatchPlacement("101");
			for (const [axis, keys] of [
				["X", valuePadKeys(firstMoverPlacement.location.x)],
				["Y", valuePadKeys(firstMoverPlacement.location.y)],
				["Z", valuePadKeys(firstMoverPlacement.location.z)],
			] as const)
				await spreadFixtureLocationThroughTouchUi(
					desk,
					patchWindow,
					101,
					101,
					axis,
					[...keys],
				);
			const visibleLightingNumbers = [
				1, 2, 3, 4, 5, 6, 7, 8, 601, 101, 102, 103, 104, 105, 106, 107,
			];
			const visibleLightingIdentities = await fixtureIdentities(
				api,
				visibleLightingNumbers,
			);
			const canonical = await desk.fastForward(
				"Patching the rest of the lighting show and reconciling the complete Stage layout.",
				async () => {
					const scenery = await installPlannedDemoScenery(api, showId, layers, {
						backCurtain: PRODUCT_DEMO_SCRIPT.patch.backCurtain,
					});
					const patch = await installPlannedDemoPatch(api, showId, layers, {
						progressive: true,
						configuration: PRODUCT_DEMO_SCRIPT.patch,
						onItem: () =>
							RECORDING
								? page.waitForTimeout(
										framesToMillis(
											PRODUCT_DEMO_SCRIPT.pacing.lightingItemFrames,
										),
									)
								: Promise.resolve(),
					});
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
				const expectedLayout = await installPlannedDemoLayout(api, showId);
				await expect
					.poll(async () => {
						const [stored] = await api.showObjects<any>(showId, "user_layout");
						return stored?.body;
					})
					.toMatchObject(expectedLayout);
				completedShow = await downloadCompletedDemoShow(api, showId);
			}
			if (RECORDING) {
				await fs.mkdir(path.dirname(SCREENSHOT), { recursive: true });
				await page.screenshot({ path: SCREENSHOT });
				await testInfo.attach("planned-demo-full-hd-screenshot", {
					path: SCREENSHOT,
					contentType: "image/png",
				});
				recordingEndedAtMillis = Date.now() - recordingStartedAtMillis;
				const timeline = buildProductDemoEditTimeline(
					titleMarkers,
					recordingEndedAtMillis,
				);
				await fs.writeFile(
					EDIT_TIMELINE,
					`${JSON.stringify(timeline, null, 2)}\n`,
					"utf8",
				);
				await testInfo.attach("product-demo-edit-timeline", {
					path: EDIT_TIMELINE,
					contentType: "application/json",
				});
			}
		} finally {
			stopObservingTitleCards();
			if (video) {
				await fs.mkdir(path.dirname(RAW_VIDEO), { recursive: true });
				const saveVideo = video.saveAs(RAW_VIDEO);
				await page.close();
				await saveVideo;
				await testInfo.attach("planned-demo-raw-video", {
					path: RAW_VIDEO,
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
	leaveDefaultFixtureIdAndCount?: boolean;
	slowSearch?: boolean;
	visibleModeSelection?: boolean;
	touchTypePlacement?: boolean;
};

function framesToMillis(frames: number) {
	return (frames / PRODUCT_DEMO_SCRIPT.fps) * 1_000;
}

function demoPatchPlacement(targets: string) {
	const placement = PRODUCT_DEMO_SCRIPT.patch.placements.find(
		(candidate) => candidate.targets === targets,
	);
	if (!placement) throw new Error(`Missing product-demo placement ${targets}`);
	return placement;
}

function valuePadKeys(value: string) {
	const tokens = value.match(/THRU|-|\d|\./gu);
	if (!tokens) throw new Error(`Invalid product-demo value ${value}`);
	return [...tokens.map((token) => (token === "-" ? "−" : token)), "ENTER"];
}

function buildProductDemoEditTimeline(
	markers: ReadonlyMap<string, number>,
	recordingEndMillis: number,
) {
	let targetStartFrame = 0;
	const sections = PRODUCT_DEMO_SCRIPT.sections.map((section, index) => {
		const sourceStartMillis = markers.get(section.id);
		const next = PRODUCT_DEMO_SCRIPT.sections[index + 1];
		const sourceEndMillis = next ? markers.get(next.id) : recordingEndMillis;
		if (sourceStartMillis == null || sourceEndMillis == null)
			throw new Error(
				`Missing recording marker for product-demo section ${section.id}`,
			);
		if (sourceEndMillis <= sourceStartMillis)
			throw new Error(
				`Invalid recording marker order for product-demo section ${section.id}`,
			);
		const timelineSection = {
			...section,
			sourceStartMillis,
			sourceEndMillis,
			targetStartFrame,
			targetEndFrame: targetStartFrame + section.frames,
			targetStartTimecode: frameTimecode(targetStartFrame),
			targetEndTimecode: frameTimecode(targetStartFrame + section.frames),
		};
		targetStartFrame +=
			section.frames -
			(index < PRODUCT_DEMO_SCRIPT.sections.length - 1
				? PRODUCT_DEMO_SCRIPT.transitionFrames
				: 0);
		return timelineSection;
	});
	return {
		version: 1,
		fps: PRODUCT_DEMO_SCRIPT.fps,
		transitionFrames: PRODUCT_DEMO_SCRIPT.transitionFrames,
		source: path.basename(RAW_VIDEO),
		output: "tosklight-product-demo.webm",
		totalFrames: targetStartFrame,
		durationMillis: framesToMillis(targetStartFrame),
		sections,
	};
}

function frameTimecode(frame: number) {
	const fps = PRODUCT_DEMO_SCRIPT.fps;
	const seconds = Math.floor(frame / fps);
	return `${String(Math.floor(seconds / 60)).padStart(2, "0")}:${String(
		seconds % 60,
	).padStart(2, "0")}:${String(frame % fps).padStart(2, "0")}`;
}

async function addPatchLayerThroughTouchUi(
	desk: DeskDriver,
	page: Page,
	name: string,
) {
	await desk.setDemoAction(`Create the ${name} Patch layer through Touch UI.`);
	await desk.click(
		page.getByRole("button", { name: "+ Add layer", exact: true }),
	);
	const dialog = page
		.getByRole("heading", { name: "Add layer", exact: true })
		.locator("xpath=ancestor::section[1]");
	await expect(dialog).toBeVisible();
	desk.setRecordingClickPace("rapid");
	await touchTypeText(
		desk,
		dialog.getByRole("textbox", { name: "Layer name" }),
		name,
	);
	desk.setRecordingClickPace("compact");
	await expect(dialog).toBeHidden();
}

async function addFixtureThroughTouchUi(
	desk: DeskDriver,
	page: Page,
	input: TouchFixtureInput,
) {
	await desk.click(
		page.getByRole("button", { name: "+ Add fixture", exact: true }),
	);
	const browser = page.locator(".fixture-browser-modal");
	const search = browser.getByRole("textbox", { name: "Search", exact: true });
	await search.fill("");
	if (input.slowSearch) {
		await page.waitForTimeout(
			framesToMillis(PRODUCT_DEMO_SCRIPT.pacing.searchClearHoldFrames),
		);
		await search.pressSequentially(input.search, {
			delay: framesToMillis(PRODUCT_DEMO_SCRIPT.pacing.searchCharacterFrames),
		});
	} else await search.fill(input.search);
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
	if (input.visibleModeSelection) {
		await desk.click(
			browser.locator(".fixture-mode-detail").locator(".ui-select-trigger"),
		);
		await desk.click(
			page.getByRole("option", {
				name: new RegExp(`^${escapeRegex(input.mode)}\\b`),
			}),
		);
		await page.waitForTimeout(framesToMillis(12));
	} else await mode.selectOption(modeValue);
	await desk.click(
		browser.locator(".fixture-mode-detail").getByRole("button", {
			name: "Add fixture",
			exact: true,
		}),
	);
	const placement = page.locator(".fixture-placement-modal");
	const name = placement.getByRole("textbox", { name: /^Fixture name\b/u });
	const fixtureId = placement.getByRole("textbox", {
		name: "Start fixture ID",
		exact: true,
	});
	const count = placement.getByRole("textbox", { name: "Count", exact: true });
	if (input.touchTypePlacement) {
		await desk.click(name);
		await name.press("ControlOrMeta+A");
		await name.pressSequentially(input.name, { delay: 70 });
		await desk.click(fixtureId);
		await fixtureId.press("ControlOrMeta+A");
		await fixtureId.pressSequentially(input.fixtureId, { delay: 160 });
		await desk.click(count);
		await count.press("ControlOrMeta+A");
		await count.pressSequentially(String(input.count), { delay: 160 });
	} else {
		await name.fill(input.name);
		if (!input.leaveDefaultFixtureIdAndCount) {
			await fixtureId.fill(input.fixtureId);
			await count.fill(String(input.count));
		}
	}
	if (input.address) {
		await placement
			.getByRole("textbox", { name: /^Address \(universe\.address\)/u })
			.fill(input.address);
		if (input.touchTypePlacement)
			await demonstrateDmxAddressDrag(desk, page, placement, 1, 30);
	}
	await desk.click(
		placement.getByRole("button", {
			name: `Add ${input.count} fixtures`,
			exact: true,
		}),
	);
	await expect(placement).toBeHidden();
}

async function demonstrateDmxAddressDrag(
	desk: DeskDriver,
	page: Page,
	placement: Locator,
	startAddress: number,
	previewAddress: number,
) {
	await desk.setDemoAction(
		`Choose DMX address 1.${startAddress} by dragging its Patch preview to ${previewAddress}, then back to ${startAddress}.`,
	);
	const grid = placement.getByRole("grid", { name: "DMX universe 1" });
	const cell = (address: number) =>
		grid.locator(`[data-dmx-address="${address}"]`);
	await dragPointer(page, cell(startAddress), cell(previewAddress));
	await page.waitForTimeout(framesToMillis(12));
	await dragPointer(page, cell(previewAddress), cell(startAddress));
	await page.waitForTimeout(framesToMillis(12));
}

async function dragPointer(page: Page, source: Locator, target: Locator) {
	const from = await source.boundingBox();
	const to = await target.boundingBox();
	if (!from || !to)
		throw new Error("The DMX address drag target is not visible");
	await page.mouse.move(from.x + from.width / 2, from.y + from.height / 2);
	await page.mouse.down();
	await page.mouse.move(to.x + to.width / 2, to.y + to.height / 2, {
		steps: 18,
	});
	await page.mouse.up();
}

async function spreadFixtureLocationThroughTouchUi(
	desk: DeskDriver,
	patchWindow: Locator,
	firstFixture: number,
	lastFixture: number,
	axis: "X" | "Y" | "Z",
	keys: string[],
	selectRange = true,
) {
	const first = fixtureRow(patchWindow, firstFixture);
	const last = fixtureRow(patchWindow, lastFixture);
	await desk.setDemoAction(
		`Set fixtures ${firstFixture}–${lastFixture} across Location ${axis}: ${keys
			.map((key) => `[${key}]`)
			.join(" ")}`,
	);
	if (selectRange) {
		await desk.click(
			first.getByRole("cell", { name: String(firstFixture), exact: true }),
		);
		if (lastFixture !== firstFixture)
			await desk.click(
				last.getByRole("cell", { name: String(lastFixture), exact: true }),
				{ modifiers: ["Shift"] },
			);
	}
	await desk.click(
		last.getByRole("button", {
			name: `Location ${axis} ${lastFixture}`,
			exact: true,
		}),
		{ button: "right" },
	);
	const application = patchWindow.page().locator(".product-demo-application");
	const pad = patchWindow.page().getByRole("dialog", {
		name: `Location ${axis} (meter)`,
	});
	await expectCenteredInDemoSurface(pad, application);
	for (const key of keys)
		await desk.click(pad.getByRole("button", { name: key, exact: true }));
	await expect(pad).toBeHidden();
}

async function spreadPhysicalPatchVectorThroughTouchUi(
	desk: DeskDriver,
	lastPhysicalRow: Locator,
	kind: "location" | "rotation",
	axis: "X" | "Y" | "Z",
	keys: readonly string[],
) {
	const offset = kind === "location" ? 12 : 15;
	const cell = offset + { X: 0, Y: 1, Z: 2 }[axis];
	await desk.setDemoAction(
		`Arrange the selected ACL physical instances with ${kind} ${axis}: ${keys
			.map((key) => `[${key}]`)
			.join(" ")}`,
	);
	await desk.click(
		lastPhysicalRow.locator("td").nth(cell).getByRole("button"),
		{
			button: "right",
		},
	);
	const pad = lastPhysicalRow.page().getByRole("dialog", {
		name: `${kind === "location" ? "Location" : "Rotation"} ${axis} (${kind === "location" ? "meter" : "degree"})`,
	});
	for (const key of keys)
		await desk.click(pad.getByRole("button", { name: key, exact: true }));
	await expect(pad).toBeHidden();
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
		"Configuring the remaining occupied universes through accelerated sACN and Art-Net setup.",
		async () => {
			desk.setRecordingClickPace("rapid");
			for (let universe = 2; universe <= 8; universe++) {
				const sacn = universe % 2 === 0;
				await createOutputRoute(
					desk,
					page,
					routes,
					sacn ? "sACN" : "Art-Net",
					universe,
					universe,
					sacn ? bench.sacn.port : bench.artnet.port,
				);
			}
			desk.setRecordingClickPace("compact");
		},
	);
	await expect
		.poll(async () => (await api.showObjects(showId, "route")).length)
		.toBe(8);
	await expect(routes.locator(".output-route-list > article")).toHaveCount(8);
	for (let universe = 1; universe <= 8; universe++)
		await expect(routes).toContainText(`Logical ${universe} →`);
	await routes
		.locator(".output-route-list > article")
		.last()
		.scrollIntoViewIfNeeded();
	await page.waitForTimeout(framesToMillis(25));
	await page.locator("body").click({ position: { x: 2, y: 2 } });
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
	await desk.fastForward(
		"Creating a Group Programming desktop with the Fixture Sheet and a seven-column Group Pool.",
		async () => {
			const desktops = new BrowserDesktops(page, async () => undefined);
			const configuration = desktops.configure("Group Programming");
			configuration.addPane(PaneType.Fixtures, {
				slug: "group-programming-fixtures",
				column: 1,
				row: 1,
				width: 16,
				height: 18,
			});
			configuration.addPane(
				PaneType.Groups,
				{
					slug: "group-programming-groups",
					column: 17,
					row: 1,
					width: 8,
					height: 18,
				},
				{ columns: 7 },
			);
			await configuration.apply();
		},
	);
	await expect(app.locator(".fixture-window")).toBeVisible();
	await expect(app.locator(".group-pool-window")).toBeVisible();
	await desk.titleCard(
		"GROUP SETUP · BEAM STAGE",
		"Select Beam Stage fixtures (101–128) and store them as Group 1: [FIXTURE] [101] [THRU] [128] [ENTER] [RECORD] [Group 1 tile]",
	);
	const groups = app.locator(".group-card");
	await expect(groups.first()).toBeVisible();
	await desk.click(keypad.locator('[data-keypad-key="HIGH"]'));
	await expect(keypad.locator('[data-keypad-key="HIGH"]')).toHaveClass(
		/highlight-armed/,
	);
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
		"Store the selected Beam Stage fixtures as Group 1: [RECORD] [Group 1 tile]",
	);
	await desk.click(keypad.getByRole("button", { name: "RECORD", exact: true }));
	await desk.click(groupTile(app, 1));
	await expect
		.poll(async () => api.showObject(showId, "group", "1"))
		.not.toBeNull();
	await nameGroupThroughTouch(desk, page, keypad, 1, "Beam Stage");

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
	await keypadCommand(desk, keypad, ["RECORD", "GRP", "8", "ENT"]);
	await nameGroupThroughTouch(desk, page, keypad, 8, "Wash Stage");

	await desk.titleCard(
		"GROUP SETUP · FIRST-LEVEL GROUPS",
		"Fast forward the repetitive first-level Groups through the same simulated hardware controls.",
	);
	desk.setRecordingClickPace("rapid");
	for (const [selection, destination, name] of [
		[["1", "2", "9", "TRU", "1", "5", "0", "ENT"], 2, "Beam Audience"],
		[["1", "5", "1", "TRU", "1", "5", "4", "ENT"], 3, "Beam Auxiliary"],
		[["2", "2", "7", "TRU", "2", "4", "2", "ENT"], 9, "Wash Audience"],
		[["2", "4", "3", "TRU", "2", "4", "6", "ENT"], 10, "Wash Auxiliary"],
		[["3", "0", "1", "TRU", "3", "1", "6", "ENT"], 15, "LED Stage"],
		[["3", "1", "7", "TRU", "4", "1", "6", "ENT"], 16, "LED Audience"],
		[["4", "1", "7", "TRU", "4", "3", "2", "ENT"], 17, "LED Auxiliary"],
	] as const) {
		await clearSelection(desk, keypad, api);
		await keypadCommand(desk, keypad, [...selection]);
		await keypadCommand(desk, keypad, [
			"RECORD",
			"GRP",
			...digits(destination),
			"ENT",
		]);
		await nameGroupThroughTouch(desk, page, keypad, destination, name);
	}
	desk.setRecordingClickPace("compact");

	await desk.titleCard(
		"GROUP SETUP · BEAM SHOW",
		"Combine Beam Stage and Beam Audience, then Record to a Group tile and name it through touch input.",
	);
	await clearSelection(desk, keypad, api);
	await keypadCommand(desk, keypad, ["GRP", "1", "+", "GRP", "2", "ENT"]);
	await desk.click(keypad.getByRole("button", { name: "RECORD", exact: true }));
	await desk.click(groupTile(app, 4));
	await keypadCommand(desk, keypad, ["SET", "GRP", "4", "ENT"]);
	const properties = page.getByRole("dialog", { name: "Group properties" });
	await touchTypeText(desk, properties.getByLabel("Group name"), "Beam Show");
	await desk.click(
		properties.getByRole("button", { name: "Save group", exact: true }),
	);

	await desk.titleCard(
		"GROUP SETUP · ODD AND EVEN",
		"Derive the Beam Show odd and even selections and store both through simulated hardware buttons.",
	);
	await clearSelection(desk, keypad, api);
	await keypadCommand(desk, keypad, ["GRP", "4", "DIV", "ENT"]);
	await keypadCommand(desk, keypad, ["RECORD", "GRP", "6", "ENT"]);
	await nameGroupThroughTouch(desk, page, keypad, 6, "Beam Show Odd");
	await clearSelection(desk, keypad, api);
	await keypadCommand(desk, keypad, ["GRP", "4", "DIV", "DIV", "ENT"]);
	await keypadCommand(desk, keypad, ["RECORD", "GRP", "7", "ENT"]);
	await nameGroupThroughTouch(desk, page, keypad, 7, "Beam Show Even");
	await desk.fastForward(
		"Creating the remaining Show, auxiliary, odd/even, and utility Groups via API.",
		async () => {
			await installPlannedDemoGroups(api, showId, fixtures);
			// Establish the physical page outside the visible assignment so the first
			// touched Group Master still lands on a genuinely empty slot.
			await keypadCommand(desk, keypad, [
				"SET",
				"GRP",
				"2",
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
		.toBe(35);

	await desk.titleCard(
		"GROUP SETUP · GROUP MASTERS",
		"Assign one Group Master by touch, a second by command line, then fast-forward the remaining assignments.",
	);
	await desk.click(keypad.getByRole("button", { name: "SET", exact: true }));
	await desk.click(groupTile(app, 6));
	await toggleProgrammerPlaybacks(desk, demo);
	await desk.click(demo.locator('[data-playback-slot="1"]'));
	await expect
		.poll(async () => playbackTarget(api, showId, 1))
		.toMatchObject({
			type: "group",
			group_id: "6",
		});
	await toggleProgrammerPlaybacks(desk, demo);
	await keypadCommand(desk, keypad, [
		"SET",
		"GRP",
		"7",
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
			group_id: "7",
		});
	await desk.fastForward(
		"Assigning the LED Show, Wash Show, All ACLs, and Blinders Group Masters.",
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
		"DYNAMICS · BEAM SHOW CIRCLE",
		"Create a two-lane Circle: Pan on Sinus and Tilt on Cosinus, bound to Beam Show.",
	);
	await clearProgrammer(desk, keypad, api);
	await keypadCommand(desk, keypad, ["GRP", "4", "ENT"]);
	await openBuiltIn(desk, app, "Dynamics");
	await createDynamicThroughTouch(desk, page, app, 19, "Pan");
	await desk.click(
		app.getByRole("button", { name: "+ Add Lane", exact: true }),
	);
	await chooseDynamicAttribute(desk, page, "Tilt");
	await desk.click(app.getByRole("button", { name: /^Select lane 2, Tilt$/ }));
	await chooseDynamicCurve(desk, page, app, "Cosinus");
	await configureDynamicThroughTouch(desk, page, app, "Beam Show Circle");
	await desk.click(
		app.getByRole("button", { name: "← Back to Pool", exact: true }),
	);

	await desk.titleCard(
		"DYNAMICS · BEAM SHOW PWM",
		"Create the Beam Show PWM chaser through the same production Dynamic editor.",
	);
	await clearSelection(desk, keypad, api);
	await keypadCommand(desk, keypad, ["GRP", "4", "ENT"]);
	await createDynamicThroughTouch(desk, page, app, 1, "Intensity");
	await chooseDynamicCurve(desk, page, app, "PWM");
	await configureDynamicThroughTouch(desk, page, app, "Beam Show PWM");
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
	await assignVirtualDynamic(desk, page, pane, keypad, 1, "Beam Show PWM", 1);
	await assignVirtualDynamic(
		desk,
		page,
		pane,
		keypad,
		2,
		"Beam Show Circle",
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
		"Bring Beam Show on, program the first Color visibly, and Record it through the simulated desk and touch pool.",
	);
	await clearProgrammer(desk, keypad, api);
	await desk.setDemoAction(
		"Bring Beam Show to full intensity, set Red to 100%, and store Color preset 2.1: [GRP] [4] [AT] [1] [0] [0] [ENTER]",
	);
	await keypadCommand(desk, keypad, ["GRP", "4", "ENT"]);
	await keypadCommand(desk, keypad, ["AT", "1", "0", "0", "ENT"]);
	await setEncoderValue(desk, demo, "Color", "Red", ["1", "0", "0"]);
	await openBuiltIn(desk, app, "Presets");
	const presets = app.locator(".preset-pool-window");
	await desk.click(presets.getByRole("button", { name: "Color", exact: true }));
	await desk.click(keypad.getByRole("button", { name: "RECORD", exact: true }));
	await desk.click(presetTile(presets, "2.1"));

	await desk.titleCard(
		"PRESET SETUP · FIRST POSITION",
		"Bring Beam Show on, program the first Position visibly, and store it to the Position pool.",
	);
	await clearProgrammer(desk, keypad, api);
	await desk.setDemoAction(
		"Bring Beam Show to full intensity, aim Pan and Tilt, and store Position preset 3.1: [GRP] [4] [AT] [1] [0] [0] [ENTER]",
	);
	await keypadCommand(desk, keypad, ["GRP", "4", "ENT"]);
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
	await keypadCommand(desk, keypad, ["GRP", "4", "ENT"]);
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
	if (
		await keypad
			.locator('[data-keypad-key="HIGH"]')
			.evaluate((element) => element.classList.contains("highlight-armed"))
	)
		await desk.click(keypad.locator('[data-keypad-key="HIGH"]'));
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
		"2",
		"2",
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
	await desk.setDemoAction(
		`Select the ${name} layer before adding its fixtures.`,
	);
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
	await settings.getByLabel("Name").blur();
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
	await desk.click(pane.getByRole("button", { name: "Settings", exact: true }));
	const paneSettings = page.getByRole("dialog", { name: "Pane Settings" });
	await desk.click(
		paneSettings.getByRole("tab", { name: "Virtual Playbacks", exact: true }),
	);
	await paneSettings.getByLabel("Rows").fill("5");
	await paneSettings.getByLabel("Columns").fill("10");
	await paneSettings.getByLabel("Columns").blur();
	await desk.click(
		paneSettings.getByRole("button", { name: "Close settings", exact: true }),
	);
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
	const clear = controls.getByRole("button", { name: "Clear input" });
	if ((await clear.count()) > 0) await desk.click(clear);
	await desk.click(controls.getByRole("button", { name: "Open keyboard" }));
	const keyboard = input.page().locator(".modal-text-keyboard");
	for (const character of value) {
		if (character === " ") {
			await desk.click(keyboard.locator("button.space"));
			continue;
		}
		const shiftedCode = character === "&" ? "Digit7" : undefined;
		if (/[A-Z]/u.test(character) || shiftedCode) {
			await desk.click(
				keyboard.getByRole("button", { name: "Shift", exact: true }),
			);
		}
		await desk.click(
			/[A-Z]/iu.test(character)
				? keyboard.getByRole("button", {
						name: character.toLocaleUpperCase(),
						exact: true,
					})
				: keyboard.locator(
						`[data-keyboard-code="${shiftedCode ?? textKeyboardCode(character)}"]`,
					),
		);
	}
	await desk.click(
		keyboard.getByRole("button", { name: "Enter · Confirm", exact: true }),
	);
}

function textKeyboardCode(character: string) {
	if (/\d/u.test(character)) return `Digit${character}`;
	if (/[A-Z]/iu.test(character)) return `Key${character.toUpperCase()}`;
	if (character === ".") return "Period";
	if (character === "-") return "Minus";
	throw new Error(`Unsupported Touch UI keyboard character ${character}`);
}

async function nameGroupThroughTouch(
	desk: DeskDriver,
	page: Page,
	keypad: Locator,
	groupId: number,
	name: string,
) {
	await keypadCommand(desk, keypad, ["SET", "GRP", ...digits(groupId), "ENT"]);
	const properties = page.getByRole("dialog", { name: "Group properties" });
	await expect(properties).toBeVisible();
	await touchTypeText(desk, properties.getByLabel("Group name"), name);
	await desk.click(
		properties.getByRole("button", { name: "Save group", exact: true }),
	);
	await expect(properties).toBeHidden();
}

async function playbackTarget(api: ApiDriver, showId: string, number: number) {
	const playback = (await api.showObjects<any>(showId, "playback")).find(
		(candidate) => candidate.body.number === number,
	);
	return playback?.body.target ?? null;
}

async function installRemainingGroupMasters(api: ApiDriver, showId: string) {
	const assignments = [
		[3, "LED Show", "18"],
		[4, "Wash Show", "11"],
		[5, "All ACLs", "32"],
		[6, "Blinders", "26"],
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
