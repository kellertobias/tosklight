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
import {
	installPlannedDemoGroups,
	plannedDemoGroupIcon,
} from "../../support/plannedDemoGroups";
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
import { PLANNED_DEMO_VIRTUAL_PLAYBACK_EXCLUSION_ZONES } from "../../support/plannedDemoVirtualPlaybackZones";
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
			id: "group-basics",
			marker: "SETTING UP THE BASICS",
			title: "Setting up the Basics",
			frames: 500,
		},
		{
			id: "groups",
			marker: "DEFINING GROUPS",
			title: "Defining Groups",
			frames: 2_750,
		},
		{
			id: "group-masters",
			marker: "ASSIGNING GROUP MASTERS",
			title: "Assigning Group Masters",
			frames: 750,
		},
		{
			id: "presets",
			marker: "PRESET SETUP",
			title: "Preset Setup",
			frames: 1_625,
		},
		{
			id: "cuelists",
			marker: "Programming Cues & Cuelists",
			title: "Programming Cues & Cuelists",
			frames: 2_500,
		},
		{ id: "dynamics", marker: "DYNAMICS", title: "Dynamics", frames: 2_250 },
		{
			id: "virtual-playbacks",
			marker: "VIRTUAL PLAYBACKS",
			title: "Virtual Playbacks",
			frames: 2_500,
		},
		{
			id: "busking-preload",
			marker: "Busking",
			title: "Busking",
			frames: 1_400,
		},
	],
	pacing: {
		titleCardFrames: 125,
		patchOpenedHoldFrames: 75,
		nameConfirmHoldFrames: 25,
		firstLayerCommittedHoldFrames: 75,
		layersCommittedHoldFrames: 25,
		fixtureBrowserHoldFrames: 75,
		modeDropdownHoldFrames: 75,
		fixturePlacementHoldFrames: 75,
		searchClearHoldFrames: 25,
		searchCharacterFrames: 2.5,
		sceneryItemFrames: 3,
		lightingItemFrames: 1,
		postSpreadHoldFrames: 75,
		aclCommittedHoldFrames: 75,
		outputSurfaceHoldFrames: 75,
		outputModalHoldFrames: 75,
		outputFieldsCompleteHoldFrames: 75,
		desktopConfigurationStepFrames: 50,
		groupSelectionHoldFrames: 75,
		groupRecordHoldFrames: 50,
		groupTileHoldFrames: 50,
		groupPropertiesHoldFrames: 250,
		groupFixtureSelectionClickMillis: 70,
		groupBulkHardwareClickMillis: 12,
		groupNameClickMillis: 3,
		groupNameConfirmHoldFrames: 25,
		groupSaveHoldFrames: 50,
		beamShowHoldFrames: 75,
		oddRuleHoldFrames: 125,
		presetItemFrames: 25,
		colorPresetFastForwardItemFrames: 8,
		cuelistSelectionHoldFrames: 50,
		cuelistRecordHoldFrames: 50,
		cuelistStoredHoldFrames: 75,
		dynamicsSurfaceHoldFrames: 75,
		dynamicsChoiceHoldFrames: 50,
		dynamicsResultHoldFrames: 50,
		dynamicsSettingsHoldFrames: 75,
		dynamicsConfiguredHoldFrames: 75,
		virtualPlaybackSurfaceHoldFrames: 75,
		virtualPlaybackChoiceHoldFrames: 50,
		virtualPlaybackResultHoldFrames: 75,
		virtualPlaybackZoneSelectionHoldFrames: 50,
		virtualPlaybackZoneDialogHoldFrames: 75,
		virtualPlaybackZoneCreatedHoldFrames: 75,
		bpm: 120,
		beatsPerBar: 4,
		buskingBars: 16,
		preloadStartBar: 8,
		buskingDynamicRevealFrames: 100,
		preloadProgrammingStepFrames: 25,
		programmerFadeMillis: 2_000,
		finalLookHoldFrames: 75,
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
			"Conventional Light",
		],
		addressBands: {
			dimmers: "1.1 THRU 1.24",
			stageLedPars: "1.25 THRU 1.508",
			stageMovingLights: "2.1 THRU 4.512",
			audience: "5.1 THRU 6.512",
			auxiliary: "8.1 THRU 8.512",
			hazers: "1.509 THRU 1.512",
		},
		placements: [
			{
				targets: "0.1 primary THRU multipatch 3",
				location: { x: "-3 THRU 3", y: "4", z: "4.15" },
			},
			{
				targets: "0.2 primary THRU multipatch 3",
				location: { x: "-3 THRU 3", y: "0", z: "4.15" },
			},
			{
				targets: "0.3 primary THRU multipatch 3",
				location: { x: "-3 THRU 3", y: "-3", z: "4.15" },
			},
			{
				targets: "1 THRU 3",
				location: { x: "-3.8 THRU -2.5", y: "-3", z: "4.15" },
			},
			{
				targets: "4 THRU 6",
				location: { x: "2.5 THRU 3.8", y: "-3", z: "4.15" },
			},
			{
				targets: "7 THRU 8",
				location: { x: "-4 THRU 4", y: "1", z: "2.4" },
			},
			{
				targets: "9 primary THRU multipatch 1",
				location: { x: "-0.7 THRU 0.7", y: "4", z: "4.15" },
			},
			{
				targets: "13 primary THRU multipatch 1",
				location: { x: "-0.4 THRU 0.4", y: "-3", z: "4.15" },
			},
			{
				targets: "601 primary THRU multipatch 7",
				location: { x: "-1 THRU 1", y: "4", z: "4.4" },
				rotation: { x: "0", y: "-18 THRU 18", z: "0" },
			},
			{
				targets: "901 primary THRU multipatch 3",
				location: { x: "-3 THRU 3", y: "-5", z: "5" },
			},
			{
				targets: "101 THRU 107",
				location: { x: "-4 THRU -0.8", y: "4", z: "4" },
			},
		],
		backCurtain: { x: "-2.5 THRU 2.5", y: 4.35, z: 0 },
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
const PRODUCT_DEMO_TIMING = {
	busking: {
		bpm: 120,
		beatsPerBar: 4,
		liveBars: 8,
		preloadBars: 8,
		commitFadeMillis: 2_000,
	},
} as const;

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
			await demoPause(page, PRODUCT_DEMO_SCRIPT.pacing.patchOpenedHoldFrames);
			await addPatchLayerThroughTouchUi(desk, page, "Stage & Venue", {
				afterCommitFrames:
					PRODUCT_DEMO_SCRIPT.pacing.firstLayerCommittedHoldFrames,
			});
			await desk.setDemoAction(
				"Add the second Patch layer through Touch UI, then accelerate the remaining repetitive layer creation.",
			);
			desk.setRecordingClickPace("rapid");
			await addPatchLayerThroughTouchUi(desk, page, "Trusses");
			for (const layer of PRODUCT_DEMO_SCRIPT.patch.layers.slice(2)) {
				desk.setRecordingClickPace("typing");
				await addPatchLayerThroughTouchUi(desk, page, layer, {
					pauseBeforeConfirm: false,
				});
			}
			desk.setRecordingClickPace("compact");
			await demoPause(
				page,
				PRODUCT_DEMO_SCRIPT.pacing.layersCommittedHoldFrames,
			);
			const layers = Object.fromEntries(
				(await api.showObjects<any>(showId, "patch_layer")).map((layer) => [
					layer.body.name,
					layer.id,
				]),
			);
			await selectPatchLayer(desk, patchWindow, "Trusses");
			for (let truss = 1; truss <= 3; truss++) {
				if (truss > 1) desk.setRecordingClickPace("typing");
				await demoPause(page, truss === 1 ? 25 : 0);
				await addFixtureThroughTouchUi(desk, page, {
					search: "4Point Truss",
					family: "Four-Point Truss",
					mode: "2 m",
					name:
						truss === 1
							? "Vectra Segment 1"
							: `${["Back", "Mid", "Front"][truss - 1]} Truss Segment 1`,
					fixtureId: `0.${truss}`,
					count: 1,
					leaveDefaultFixtureIdAndCount: truss === 1,
					slowSearch: truss === 1,
					visibleModeSelection: truss === 1,
					pauseForFixtureLibrary: truss === 1,
					pauseForPlacement: truss === 1,
				});
				const trussPrimary = fixtureRow(patchWindow, `0.${truss}`);
				if (truss > 1) desk.setRecordingClickPace("typing");
				await desk.click(trussPrimary);
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
				const allMultipatches = patchWindow.locator(".multipatch-row");
				const lastPhysicalRow = allMultipatches.nth(truss * 3 - 1);
				if (truss > 1) desk.setRecordingClickPace("typing");
				await desk.click(trussPrimary);
				await desk.click(lastPhysicalRow.locator("td").first(), {
					modifiers: ["Shift"],
				});
				const placement = demoPatchPlacement(
					`0.${truss} primary THRU multipatch 3`,
				);
				for (const [axis, keys] of [
					["X", valuePadKeys(placement.location.x)],
					["Y", valuePadKeys(placement.location.y)],
					["Z", valuePadKeys(placement.location.z)],
				] as const)
					await spreadPhysicalPatchVectorThroughTouchUi(
						desk,
						lastPhysicalRow,
						"location",
						axis,
						keys,
					);
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
			await selectPatchLayer(desk, patchWindow, "Conventional Light");
			await addFixtureThroughTouchUi(desk, page, {
				search: "Dimmer Fresnel",
				family: "Dimmer Fresnel",
				mode: "8-bit",
				name: "Front Truss Left 1",
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
			for (const [first, last] of [
				[1, 3],
				[4, 6],
				[7, 8],
			] as const) {
				const placement = demoPatchPlacement(`${first} THRU ${last}`);
				for (const [axis, keys] of [
					["X", valuePadKeys(placement.location.x)],
					["Y", valuePadKeys(placement.location.y)],
					["Z", valuePadKeys(placement.location.z)],
				] as const)
					await spreadFixtureLocationThroughTouchUi(
						desk,
						patchWindow,
						first,
						last,
						axis,
						keys,
						axis === "X",
					);
				if (first === 1)
					await demoPause(
						page,
						PRODUCT_DEMO_SCRIPT.pacing.postSpreadHoldFrames,
					);
			}
			await expectFixtureLocations(api, [1, 2, 3, 4, 5, 6], {
				y: -3_000,
				z: 4_150,
			});
			await expectFixtureLocations(api, [7, 8], { y: 1_000, z: 2_400 });

			const centerProfile = requiredFixture(desiredByNumber, 13);
			await desk.setDemoAction(
				"Patch Profile Stage Center at 1.12 and add its second, unaddressed physical lamp as a true multi-patch.",
			);
			await addFixtureThroughTouchUi(desk, page, {
				search: "Dimmer Profile",
				family: "Dimmer Profile",
				mode: "8-bit",
				name: "Profile Stage Center",
				fixtureId: "13",
				count: 1,
				address: fixtureAddress(centerProfile),
				visibleModeSelection: true,
			});
			await desk.click(fixtureRow(patchWindow, 13));
			await addMultipatchesThroughTouchUi(desk, patchWindow, 13, 1);

			const houseLights = requiredFixture(desiredByNumber, 901);
			await desk.setDemoAction(
				"Patch four independently addressed House Lights at 1.17 through 1.20 using one PAR control and three real multi-patches.",
			);
			await addFixtureThroughTouchUi(desk, page, {
				search: "Dimmer PAR Can",
				family: "Dimmer PAR Can",
				mode: "8-bit",
				name: "House Lights",
				fixtureId: "901",
				count: 1,
				address: fixtureAddress(houseLights),
				visibleModeSelection: true,
			});
			await desk.click(fixtureRow(patchWindow, 901));
			const houseRows = await addMultipatchesThroughTouchUi(
				desk,
				patchWindow,
				901,
				3,
			);
			for (const [index, address] of [18, 19, 20].entries())
				await setMultipatchAddressThroughTouchUi(
					desk,
					page,
					houseRows[index],
					`1.${address}`,
				);

			const acl = requiredFixture(desiredByNumber, 601);
			await addFixtureThroughTouchUi(desk, page, {
				search: "ACL",
				family: "ACL",
				mode: "8-bit",
				name: "ACL Back Center",
				fixtureId: "601",
				count: 1,
				address: fixtureAddress(acl),
			});
			await desk.click(
				fixtureRow(patchWindow, 601).getByRole("cell", {
					name: "601",
					exact: true,
				}),
			);
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
			const aclLastPhysicalRow = aclPrimaryRow.locator(
				'xpath=following-sibling::tr[contains(@class, "multipatch-row")][7]',
			);
			await expect(aclLastPhysicalRow).toHaveCount(1);
			await desk.click(
				aclPrimaryRow.getByRole("cell", { name: "601", exact: true }),
			);
			await desk.click(aclLastPhysicalRow.locator("td").first(), {
				modifiers: ["Shift"],
			});
			const aclPlacement = demoPatchPlacement("601 primary THRU multipatch 7");
			const aclRotation =
				"rotation" in aclPlacement ? aclPlacement.rotation.y : "0";
			for (const [kind, axis, keys] of [
				["location", "X", valuePadKeys(aclPlacement.location.x)],
				["location", "Y", valuePadKeys(aclPlacement.location.y)],
				["location", "Z", valuePadKeys(aclPlacement.location.z)],
				["rotation", "Y", valuePadKeys(aclRotation)],
			] as const)
				await spreadPhysicalPatchVectorThroughTouchUi(
					desk,
					aclLastPhysicalRow,
					kind,
					axis,
					keys,
				);
			await demoPause(page, PRODUCT_DEMO_SCRIPT.pacing.aclCommittedHoldFrames);
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
			const firstMoverPlacement = demoPatchPlacement("101 THRU 107");
			for (const [axis, keys] of [
				["X", valuePadKeys(firstMoverPlacement.location.x)],
				["Y", valuePadKeys(firstMoverPlacement.location.y)],
				["Z", valuePadKeys(firstMoverPlacement.location.z)],
			] as const)
				await spreadFixtureLocationThroughTouchUi(
					desk,
					patchWindow,
					101,
					107,
					axis,
					[...keys],
					axis === "X",
				);
			await expectFixtureLocations(api, [101, 102, 103, 104, 105, 106, 107], {
				y: 4_000,
				z: 4_000,
			});
			const visibleLightingNumbers = [
				1, 2, 3, 4, 5, 6, 7, 8, 13, 601, 901, 101, 102, 103, 104, 105, 106, 107,
			];
			const visibleLightingIdentities = await fixtureIdentities(
				api,
				visibleLightingNumbers,
			);
			const canonical = await desk.fastForward(
				"Patching the rest of the lighting show and reconciling the complete Stage layout.",
				async () => {
					let visibleLayerId: string | undefined;
					const layerNameById = new Map(
						Object.entries(layers).map(([name, id]) => [id, name]),
					);
					const scenery = await installPlannedDemoScenery(api, showId, layers, {
						backCurtain: PRODUCT_DEMO_SCRIPT.patch.backCurtain,
					});
					const patch = await installPlannedDemoPatch(api, showId, layers, {
						progressive: true,
						configuration: PRODUCT_DEMO_SCRIPT.patch,
						onBeforeItem: async ({ layerId }) => {
							if (layerId === visibleLayerId) return;
							const layerName = layerNameById.get(layerId);
							if (!layerName)
								throw new Error(`Missing visible Patch layer for ${layerId}`);
							await selectPatchLayer(desk, patchWindow, layerName, false);
							visibleLayerId = layerId;
						},
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
				fixtureRecords: 231,
				physicalInstances: 264,
				occupiedSlots: 2_988,
			});
			await expect
				.poll(async () => (await api.patch()).fixtures.length)
				.toBe(PLANNED_DEMO_TOTAL_FIXTURE_RECORDS);
			await expect(patchWindow.locator(".ui-window-info")).toContainText(
				`${PLANNED_DEMO_TOTAL_FIXTURE_RECORDS} fixtures`,
			);
			await expect(fixtureRow(patchWindow, 417)).toBeVisible();
			await expect(fixtureRow(patchWindow, 101)).toHaveCount(0);
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
				page,
				demo,
				app,
				keypad,
				api,
				showId,
				canonical.patch.fixtures,
			);
			await buildCueProgramming(
				desk,
				page,
				demo,
				app,
				keypad,
				api,
				showId,
				canonical.patch.fixtures,
			);
			await buildDynamicsSetup(desk, page, app, keypad, api, showId);
			await demonstrateBuskingAndPreload(
				desk,
				demo,
				app,
				keypad,
				api,
				bench,
				showId,
			);
			if (RECORDING) {
				await fs.mkdir(path.dirname(SCREENSHOT), { recursive: true });
				await page.screenshot({ path: SCREENSHOT });
				await testInfo.attach("planned-demo-full-hd-screenshot", {
					path: SCREENSHOT,
					contentType: "image/png",
				});
				recordingEndedAtMillis = Date.now() - recordingStartedAtMillis;
			}
			await keypadCommand(desk, keypad, ["1", "0", "1", "ENT"]);
			await expect.poll(async () => activeNumbers(api)).toContain(101);
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
	pauseForFixtureLibrary?: boolean;
	pauseForPlacement?: boolean;
};

function framesToMillis(frames: number) {
	return (frames / PRODUCT_DEMO_SCRIPT.fps) * 1_000;
}

async function demoPause(page: Page, frames: number) {
	if (RECORDING && frames > 0)
		await page.waitForTimeout(framesToMillis(frames));
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
	options: {
		pauseBeforeConfirm?: boolean;
		afterCommitFrames?: number;
	} = {},
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
		{
			beforeConfirmFrames:
				options.pauseBeforeConfirm === false
					? 0
					: PRODUCT_DEMO_SCRIPT.pacing.nameConfirmHoldFrames,
		},
	);
	desk.setRecordingClickPace("compact");
	if (await dialog.isVisible())
		await desk.click(
			dialog.getByRole("button", { name: "Add layer", exact: true }),
		);
	await expect(dialog).toBeHidden();
	await demoPause(page, options.afterCommitFrames ?? 0);
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
	if (input.pauseForFixtureLibrary)
		await demoPause(page, PRODUCT_DEMO_SCRIPT.pacing.fixtureBrowserHoldFrames);
	const [titlebarBox, searchBox] = await Promise.all([
		browser.locator(".ui-modal-titlebar").boundingBox(),
		browser.locator(".ui-modal-title-search").boundingBox(),
	]);
	if (!titlebarBox || !searchBox)
		throw new Error("The Fixture Library titlebar search is not visible");
	expect(Math.abs(titlebarBox.y - searchBox.y)).toBeLessThanOrEqual(1);
	expect(
		Math.abs(
			titlebarBox.y + titlebarBox.height - (searchBox.y + searchBox.height),
		),
	).toBeLessThanOrEqual(1);
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
	const modeTrigger = browser
		.locator(".fixture-mode-detail")
		.locator(".ui-select-trigger");
	if (input.visibleModeSelection) {
		await desk.click(modeTrigger);
		await demoPause(page, PRODUCT_DEMO_SCRIPT.pacing.modeDropdownHoldFrames);
	} else await modeTrigger.click();
	const modeOption = page.getByRole("option", {
		name: new RegExp(`^${escapeRegex(input.mode)}\\b`),
	});
	await expect(modeOption).toBeVisible();
	if (input.visibleModeSelection) await desk.click(modeOption);
	else await modeOption.click();
	if (input.visibleModeSelection) await demoPause(page, 12);
	await desk.click(
		browser.locator(".fixture-mode-detail").getByRole("button", {
			name: "Add fixture",
			exact: true,
		}),
	);
	const placement = page.locator(".fixture-placement-modal");
	if (input.pauseForPlacement)
		await demoPause(
			page,
			PRODUCT_DEMO_SCRIPT.pacing.fixturePlacementHoldFrames,
		);
	const name = placement.getByRole("textbox", { name: /^Fixture name\b/u });
	const fixtureId = placement.getByRole("textbox", {
		name: "Start fixture ID",
		exact: true,
	});
	const count = placement.getByRole("textbox", { name: "Count", exact: true });
	if (input.touchTypePlacement) {
		await desk.click(name);
		await name.press("ControlOrMeta+A");
		await name.pressSequentially(input.name, { delay: 35 });
		await desk.click(fixtureId);
		await fixtureId.press("ControlOrMeta+A");
		await fixtureId.pressSequentially(input.fixtureId, { delay: 80 });
		await desk.click(count);
		await count.press("ControlOrMeta+A");
		await count.pressSequentially(String(input.count), { delay: 80 });
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
		const armedSet = patchWindow
			.page()
			.locator("button.patch-set-armed")
			.filter({ hasText: /^SET$/u });
		if (await armedSet.isVisible()) await desk.click(armedSet);
		await desk.click(
			first.getByRole("cell", { name: String(firstFixture), exact: true }),
		);
		await patchWindow.page().waitForTimeout(300);
		for (let fixture = firstFixture + 1; fixture <= lastFixture; fixture++) {
			await desk.click(
				fixtureRow(patchWindow, fixture).getByRole("cell", {
					name: String(fixture),
					exact: true,
				}),
				{ modifiers: ["Meta"] },
			);
			await patchWindow.page().waitForTimeout(300);
		}
		for (let fixture = firstFixture; fixture <= lastFixture; fixture++)
			await expect(fixtureRow(patchWindow, fixture)).toHaveClass(/selected/u);
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
	const addressLayer = lastPhysicalRow
		.page()
		.locator('.fixture-address-layer[data-modal-top="true"]');
	if (await addressLayer.isVisible()) {
		await desk.click(
			addressLayer.getByRole("button", {
				name: /^(?:Cancel (?:Fixture|Multi-patch) Address|Close fixture addresses)$/u,
			}),
		);
		await expect(addressLayer).toBeHidden();
	}
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

async function expectFixtureLocations(
	api: ApiDriver,
	numbers: readonly number[],
	expected: Partial<{ x: number; y: number; z: number }>,
) {
	await expect
		.poll(async () => {
			const fixtures = (await api.patch()).fixtures;
			return numbers.map((number) => {
				const location = fixtures.find(
					(fixture: any) => fixture.fixture_number === number,
				)?.location;
				return Object.fromEntries(
					Object.keys(expected).map((axis) => [
						axis,
						location?.[axis as keyof typeof expected],
					]),
				);
			});
		})
		.toEqual(numbers.map(() => expected));
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
			"The separate packaged 306-instance acceptance remains authoritative for WebView Stage behavior.",
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
	await expect(demo.locator("[data-demo-chapter]")).toHaveCount(10);
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
	await demoPause(page, PRODUCT_DEMO_SCRIPT.pacing.outputSurfaceHoldFrames);
	const routes = app.getByRole("region", { name: "Output routes" });
	await createOutputRoute(
		desk,
		page,
		routes,
		"Art-Net",
		"1 THRU 8",
		"1 THRU 8",
		bench.artnet.port,
		{
			modalHoldFrames: PRODUCT_DEMO_SCRIPT.pacing.outputModalHoldFrames,
			fieldsCompleteHoldFrames:
				PRODUCT_DEMO_SCRIPT.pacing.outputFieldsCompleteHoldFrames,
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
	await desk.titleCard(
		"SETTING UP THE BASICS",
		"Create a dedicated programming desktop with the Fixture Sheet and a seven-column Group Pool before defining the show’s fixture selections.",
	);
	await desk.setDemoAction(
		"Configure the Group Programming desktop with a Fixture Sheet and a seven-column Group Pool.",
	);
	{
		const desktops = new BrowserDesktops(
			page,
			async () => undefined,
			() =>
				demoPause(
					page,
					PRODUCT_DEMO_SCRIPT.pacing.desktopConfigurationStepFrames,
				),
		);
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
	}
	await demoPause(
		page,
		PRODUCT_DEMO_SCRIPT.pacing.desktopConfigurationStepFrames,
	);
	await expect(app.locator(".fixture-window")).toBeVisible();
	await expect(app.locator(".group-pool-window")).toBeVisible();
	const groups = app.locator(".group-card");
	await expect(groups.first()).toBeVisible();
	await desk.click(keypad.locator('[data-keypad-key="HIGH"]'));
	await expect(keypad.locator('[data-keypad-key="HIGH"]')).toHaveClass(
		/highlight-armed/,
	);
	await clearSelection(desk, keypad, api);
	await desk.titleCard(
		"DEFINING GROUPS",
		"Layered groups for flexible shows: select fixtures in the Fixture Sheet and organize them in a seven-column Group Pool.",
	);
	await desk.setDemoAction(
		"Select Beam Stage fixtures 101–128 directly in the Fixture Sheet.",
	);
	await selectFixturesThroughFixtureSheet(desk, app, fixtures, 101, 128);
	await demoPause(page, PRODUCT_DEMO_SCRIPT.pacing.groupSelectionHoldFrames);
	await desk.setDemoAction(
		"Store the visible Beam Stage selection as Group 1: [RECORD], then touch Group tile 1.",
	);
	await desk.click(keypad.getByRole("button", { name: "RECORD", exact: true }));
	await demoPause(page, PRODUCT_DEMO_SCRIPT.pacing.groupRecordHoldFrames);
	await desk.click(groupTile(app, 1));
	await demoPause(page, PRODUCT_DEMO_SCRIPT.pacing.groupTileHoldFrames);
	await expect
		.poll(async () => api.showObject(showId, "group", "1"))
		.not.toBeNull();
	await clearSelection(desk, keypad, api);
	await nameGroupThroughTouchTile(desk, page, app, keypad, 1, "Beam Stage");

	await desk.setDemoAction(
		"Create the Wash Stage Group with the command line, pausing between selection, record, and confirmation.",
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
	await demoPause(page, PRODUCT_DEMO_SCRIPT.pacing.groupSelectionHoldFrames);
	await desk.click(keypad.getByRole("button", { name: "RECORD", exact: true }));
	await demoPause(page, PRODUCT_DEMO_SCRIPT.pacing.groupRecordHoldFrames);
	await desk.click(keypad.getByRole("button", { name: "GRP", exact: true }));
	await desk.click(keypad.getByRole("button", { name: "8", exact: true }));
	await demoPause(page, PRODUCT_DEMO_SCRIPT.pacing.groupTileHoldFrames);
	await desk.click(keypad.getByRole("button", { name: "ENT", exact: true }));
	await expect
		.poll(async () => api.showObject(showId, "group", "8"))
		.not.toBeNull();
	await nameGroupThroughTouch(desk, page, keypad, 8, "Wash Stage", false);

	await desk.setDemoAction(
		"Fast forward the remaining first-level Groups through simulated hardware controls.",
	);
	setBulkGroupCreationPace(desk);
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
		await nameGroupThroughTouch(desk, page, keypad, destination, name, false);
		setBulkGroupCreationPace(desk);
	}
	desk.setRecordingClickPace("compact");

	await desk.setDemoAction(
		"Select Beam Stage plus Beam Audience and combine them into the reusable Beam Show Group.",
	);
	await demoPause(page, PRODUCT_DEMO_SCRIPT.pacing.beamShowHoldFrames);
	await clearSelection(desk, keypad, api);
	await desk.click(groupTile(app, 1));
	await desk.click(keypad.getByRole("button", { name: "+", exact: true }));
	await desk.click(groupTile(app, 2));
	await desk.click(keypad.getByRole("button", { name: "RECORD", exact: true }));
	await desk.click(groupTile(app, 4));
	await desk.click(keypad.getByRole("button", { name: "SET", exact: true }));
	await demoPause(page, PRODUCT_DEMO_SCRIPT.pacing.groupTileHoldFrames);
	await desk.click(groupTile(app, 4));
	const properties = page.getByRole("dialog", { name: "Group properties" });
	await demoPause(page, PRODUCT_DEMO_SCRIPT.pacing.groupPropertiesHoldFrames);
	await touchTypeText(desk, properties.getByLabel("Group name"), "Beam Show", {
		beforeConfirmFrames: PRODUCT_DEMO_SCRIPT.pacing.groupNameConfirmHoldFrames,
		pace: "fastTyping",
	});
	await selectGroupIconThroughTouch(desk, page, properties, "Beam Show", true);
	await demoPause(page, PRODUCT_DEMO_SCRIPT.pacing.groupSaveHoldFrames);
	await desk.click(
		properties.getByRole("button", { name: "Save group", exact: true }),
	);

	await desk.setDemoAction(
		"Derive the Beam Show odd and even selections and store both through simulated hardware buttons.",
	);
	await clearSelection(desk, keypad, api);
	await keypadCommand(desk, keypad, ["GRP", "4", "DIV"]);
	await demoPause(page, PRODUCT_DEMO_SCRIPT.pacing.oddRuleHoldFrames);
	await desk.click(keypad.getByRole("button", { name: "ENT", exact: true }));
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
			const pages = await api.showObjects<any>(showId, "playback_page");
			const page = pages.find((candidate) => candidate.body.number === 1);
			await api.seedShowObject(
				showId,
				"playback_page",
				page?.id ?? "1",
				{
					number: 1,
					name: page?.body.name ?? "Busking",
					slots: page?.body.slots ?? {},
					virtual_playbacks: page?.body.virtual_playbacks ?? {},
				},
				page?.revision ?? 0,
			);
		},
	);
	await expect
		.poll(async () => (await api.showObjects(showId, "group")).length)
		.toBe(35);

	await desk.titleCard(
		"ASSIGNING GROUP MASTERS",
		"Assign one Group Master by touch, a second by command line, then fast-forward the remaining assignments.",
	);
	await toggleProgrammerPlaybacks(desk, demo);
	await expect(demo.locator(".mode-toggle")).toHaveClass(/playbacks-active/u);
	const commandLine = page.getByRole("textbox", { name: "Command line" });
	const setButton = keypad.getByRole("button", { name: "SET", exact: true });
	await desk.click(setButton);
	await expect(setButton).toHaveClass(/patch-set-armed/u);
	await desk.click(groupTile(app, 6));
	await expect(commandLine).toHaveValue("SET GROUP 6");
	await desk.click(demo.locator('[data-playback-slot="1"]'));
	await expect
		.poll(async () => playbackTarget(api, showId, 1))
		.toMatchObject({
			type: "group",
			group_id: "6",
		});
	await desk.setDemoAction(
		"Assign Beam Show Even to Playback 2 through the command line: SET GROUP 7 AT 1.2.",
	);
	await desk.click(commandLine);
	await commandLine.press("ControlOrMeta+A");
	await commandLine.pressSequentially("SET GROUP 7 AT 1.2", { delay: 35 });
	await demoPause(page, PRODUCT_DEMO_SCRIPT.pacing.groupTileHoldFrames);
	await commandLine.press("Enter");
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
	await toggleProgrammerPlaybacks(desk, demo);
	await expect(demo.locator(".mode-toggle")).not.toHaveClass(
		/playbacks-active/u,
	);
}

async function buildDynamicsSetup(
	desk: DeskDriver,
	page: Page,
	app: Locator,
	keypad: Locator,
	api: ApiDriver,
	showId: string,
) {
	await desk.titleCard(
		"DYNAMICS",
		"ToskLight's Effect engine - Animate Parameters based on Keyframes or functions.",
	);
	await desk.setDemoAction(
		"Select Beam Show, then build a two-axis Circle Dynamic from familiar Pan sine and Tilt cosine functions.",
	);
	await clearProgrammer(desk, keypad, api);
	await openGroups(desk, keypad);
	await desk.click(groupTile(app, 4));
	await demoPause(page, PRODUCT_DEMO_SCRIPT.pacing.dynamicsResultHoldFrames);
	await openBuiltIn(desk, app, "Dynamics");
	await demoPause(page, PRODUCT_DEMO_SCRIPT.pacing.dynamicsSurfaceHoldFrames);
	await createDynamicThroughTouch(desk, page, app, 19, "Pan");
	await desk.click(
		app.getByRole("button", { name: "+ Add Lane", exact: true }),
	);
	await demoPause(page, PRODUCT_DEMO_SCRIPT.pacing.dynamicsResultHoldFrames);
	await chooseDynamicAttribute(desk, page, "Tilt");
	await desk.click(app.getByRole("button", { name: /^Select lane 2, Tilt$/ }));
	await chooseDynamicCurve(desk, page, app, "Cosinus");
	await configureDynamicThroughTouch(desk, page, app, "Beam Show Circle");
	await desk.click(
		app.getByRole("button", { name: "← Back to Pool", exact: true }),
	);
	await demoPause(page, PRODUCT_DEMO_SCRIPT.pacing.dynamicsResultHoldFrames);

	await desk.setDemoAction(
		"Create the Beam Show PWM chaser through the same production Dynamic editor.",
	);
	await clearSelection(desk, keypad, api);
	await openGroups(desk, keypad);
	await desk.click(groupTile(app, 4));
	await demoPause(page, PRODUCT_DEMO_SCRIPT.pacing.dynamicsResultHoldFrames);
	await openBuiltIn(desk, app, "Dynamics");
	await demoPause(page, PRODUCT_DEMO_SCRIPT.pacing.dynamicsSurfaceHoldFrames);
	await createDynamicThroughTouch(desk, page, app, 1, "Intensity");
	await chooseDynamicCurve(desk, page, app, "PWM");
	await configureDynamicThroughTouch(desk, page, app, "Beam Show PWM");
	await desk.click(
		app.getByRole("button", { name: "← Back to Pool", exact: true }),
	);
	await demoPause(page, PRODUCT_DEMO_SCRIPT.pacing.dynamicsResultHoldFrames);
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
		"VIRTUAL PLAYBACKS",
		"You do not need to assign a playback to a hardware button.",
	);
	const pane = await createVirtualPlaybackDesktop(desk, page);
	await assignVirtualDynamic(desk, page, pane, keypad, 1, "Beam Show PWM", 1);
	await assignVirtualDynamic(
		desk,
		page,
		pane,
		keypad,
		19,
		"Beam Show Circle",
		19,
	);
	await desk.fastForward(
		"Assigning every remaining Dynamic to its stable Virtual Playback and completing the Programming and Theater desktops before grouping related effects.",
		async () => {
			await installPlannedDemoDynamicPlaybacks(api, showId, definitions);
			await installPlannedDemoLayout(api, showId);
		},
	);
	for (const zone of PLANNED_DEMO_VIRTUAL_PLAYBACK_EXCLUSION_ZONES) {
		await createVirtualPlaybackExclusionZone(
			desk,
			page,
			pane,
			zone.name,
			zone.playback_numbers.map((number) => number - 1000),
		);
	}
	await expect
		.poll(async () => virtualPlaybackExclusionZones(api, showId))
		.toEqual(
			PLANNED_DEMO_VIRTUAL_PLAYBACK_EXCLUSION_ZONES.map((zone) => ({
				name: zone.name,
				playback_numbers: [...zone.playback_numbers],
			})),
		);
	await expect
		.poll(async () => (await api.showObjects(showId, "dynamic")).length)
		.toBe(30);
}

async function buildPresetSetup(
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
		"PRESET SETUP",
		"Build reusable Position, Color, and Beam looks while Highlight keeps the selected fixtures visible on Stage.",
	);
	if (
		!(await keypad
			.locator('[data-keypad-key="HIGH"]')
			.evaluate((element) => element.classList.contains("highlight-armed")))
	)
		await desk.click(keypad.locator('[data-keypad-key="HIGH"]'));
	await expect(demo.locator(".command-status .highlight-status")).toHaveText(
		"Highlight",
	);
	await openBuiltIn(desk, app, "Presets");
	const presets = app.locator(".preset-pool-window");
	await showPresetGroupShortcuts(page, presets);
	await desk.click(
		presets.getByRole("button", { name: "Position", exact: true }),
	);
	for (const position of [
		{ name: "Down", address: "3.1", pan: ["5", "0"], tilt: ["2", "5"] },
		{ name: "Up", address: "3.2", pan: ["5", "0"], tilt: ["8", "2"] },
		{
			name: "Fan",
			address: "3.4",
			pan: ["1", "5", "THRU", "8", "5"],
			tilt: ["5", "8"],
		},
	] as const) {
		await clearProgrammer(desk, keypad, api);
		await selectPresetGroupShortcut(desk, presets, "Beam Show");
		await desk.setDemoAction(
			`Program the ${position.name} Position preset${position.name === "Fan" ? " with a THRU spread" : " with direct Pan and Tilt values"}.`,
		);
		await setEncoderValue(desk, demo, "Position", "Pan", [...position.pan]);
		await setEncoderValue(desk, demo, "Position", "Tilt", [...position.tilt]);
		await desk.click(
			keypad.getByRole("button", { name: "RECORD", exact: true }),
		);
		await desk.click(presetTile(presets, position.address));
	}

	await clearProgrammer(desk, keypad, api);
	await desk.setDemoAction(
		"Create the first Color preset as absolute encoder values while Highlight shows the result.",
	);
	await desk.click(presets.getByRole("button", { name: "Color", exact: true }));
	await selectPresetGroupShortcut(desk, presets, "Beam Show");
	await setEncoderValue(desk, demo, "Color", "Red", ["1", "0", "0"]);
	await desk.click(keypad.getByRole("button", { name: "RECORD", exact: true }));
	await desk.click(presetTile(presets, "2.1"));

	await clearProgrammer(desk, keypad, api);
	await desk.setDemoAction(
		"Create the second Color preset with the graphical Color Special Dialog.",
	);
	await selectPresetGroupShortcut(desk, presets, "Beam Show");
	await desk.click(
		demo
			.locator(".product-demo-application .parameter-controls")
			.first()
			.getByRole("button", { name: /^Color(?: \d+ of \d+)?$/u })
			.first(),
	);
	await desk.click(
		app.getByRole("button", { name: "Special Dialog", exact: true }),
	);
	const colorDialog = page.locator(".special-dialog-card");
	const colorSheet = colorDialog.locator(".color-sheet");
	const colorBox = await colorSheet.boundingBox();
	if (!colorBox)
		throw new Error("The graphical Color Special Dialog is not visible");
	await page.mouse.click(
		colorBox.x + colorBox.width * 0.78,
		colorBox.y + colorBox.height * 0.3,
	);
	await desk.click(colorDialog.getByRole("button", { name: "×", exact: true }));
	await desk.click(keypad.getByRole("button", { name: "RECORD", exact: true }));
	await desk.click(presetTile(presets, "2.2"));
	await desk.fastForward(
		"Completing the remaining Color presets in three to four seconds, then programming Position and Beam presets at one second each.",
		() =>
			installPlannedDemoPresets(api, showId, fixtures, {
				onItem: ({ family, index }) =>
					family === "Color" && index < 2
						? Promise.resolve()
						: demoPause(
								page,
								family === "Color"
									? PRODUCT_DEMO_SCRIPT.pacing.colorPresetFastForwardItemFrames
									: PRODUCT_DEMO_SCRIPT.pacing.presetItemFrames,
							),
			}),
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

async function showPresetGroupShortcuts(page: Page, presets: Locator) {
	const shortcuts = presets.locator(".group-strip");
	if (await shortcuts.isVisible()) return;
	const toggle = presets.getByRole("button", { name: "Groups", exact: true });
	const box = await toggle.boundingBox();
	if (!box) throw new Error("The Preset Group-shortcut toggle is not visible");
	await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
	await page.mouse.down();
	await page.waitForTimeout(700);
	await page.mouse.up();
	await expect(shortcuts).toBeVisible();
}

async function selectPresetGroupShortcut(
	desk: DeskDriver,
	presets: Locator,
	name: string,
) {
	await desk.click(
		presets
			.locator(".group-strip .group-card")
			.getByText(name, { exact: true })
			.first(),
	);
}

async function buildCueProgramming(
	desk: DeskDriver,
	page: Page,
	demo: Locator,
	app: Locator,
	keypad: Locator,
	api: ApiDriver,
	showId: string,
	fixtures: readonly any[],
) {
	await migrateDemoGroupMastersToCanonicalPlaybacks(api, showId);
	await desk.titleCard(
		"Programming Cues & Cuelists",
		"Build the core show Cuelists with the Programmer-only Fixture Sheet, Cuelist Pool, and live Cue detail visible together.",
	);
	await desk.setDemoAction(
		"Configure a Cue Programming desktop: Programmer-only Fixture Sheet on the left, Cuelist Pool above Cue detail on the right.",
	);
	const desktops = new BrowserDesktops(
		page,
		async () => undefined,
		() =>
			demoPause(
				page,
				PRODUCT_DEMO_SCRIPT.pacing.desktopConfigurationStepFrames,
			),
	);
	const configuration = desktops.configure("Cue Programming");
	const fixtureSheet = configuration.addPane(
		PaneType.Fixtures,
		{
			slug: "cue-programming-fixtures",
			column: 1,
			row: 1,
			width: 14,
			height: 18,
		},
		{ activeOnly: true },
	);
	const cuelistPool = configuration.addPane(PaneType.CuelistPool, {
		slug: "cue-programming-pool",
		column: 15,
		row: 1,
		width: 10,
		height: 9,
	});
	const cueDetail = configuration.addPane(
		PaneType.Cues,
		{
			slug: "cue-programming-detail",
			column: 15,
			row: 10,
			width: 10,
			height: 9,
		},
		{ cueListSource: "follow-selection", showCueSidebar: true },
	);
	await configuration.apply();
	await fixtureSheet.expect.visible();
	await cuelistPool.expect.visible();
	await cueDetail.expect.visible();

	const visibleCueListIds = new Map<number, string>();
	for (const item of [
		{
			number: 1,
			name: "Start",
			selection: [
				"GRP",
				"4",
				"+",
				"GRP",
				"1",
				"1",
				"+",
				"GRP",
				"1",
				"8",
				"+",
				"GRP",
				"2",
				"6",
				"AT",
				"1",
				"0",
				"0",
				"ENT",
			],
			action:
				"Build the opening show state and store it slowly as Cuelist 1 · Start.",
		},
		{
			number: 2,
			name: "Front Light",
			selection: ["GRP", "2", "9", "AT", "1", "0", "0", "ENT"],
			action:
				"Select the Front Lights, set their intensity, and store Cuelist 2 · Front Light.",
		},
		{
			number: 3,
			name: "Hazer",
			selection: ["GRP", "3", "1", "AT", "2", "0", "ENT"],
			action:
				"Set the Hazer to a restrained level and store Cuelist 3 · Hazer.",
		},
		{
			number: 8,
			name: "ACL Chase",
			selection: ["GRP", "2", "2", "AT", "1", "0", "0", "ENT"],
			action:
				"Create the first ACL step and store it as Cuelist 8 · ACL Chase.",
		},
		{
			number: 4,
			name: "ACL 1",
			selection: ["GRP", "2", "2", "AT", "1", "0", "0", "ENT"],
			action: "Store the first individual ACL look as Cuelist 4 · ACL 1.",
		},
	] as const) {
		const cueListId = await recordVisibleCuelist({
			desk,
			page,
			keypad,
			api,
			showId,
			pool: cuelistPool.root(),
			detail: cueDetail.root(),
			...item,
		});
		visibleCueListIds.set(item.number, cueListId);
	}
	await desk.fastForward(
		"Completing ACL Chase steps two through four, the remaining ACL Cuelists, and their physical Playback assignments.",
		() => installPlannedDemoPlaybacks(api, showId, fixtures),
	);
	await expect(cuelistPool.root()).toContainText("Start");
	await expect(cuelistPool.root()).toContainText("ACL Chase");
	expect(await api.showObjects(showId, "cue_list")).toHaveLength(8);
	expect(await api.showObjects(showId, "playback")).toHaveLength(14);
	for (const [number, cueListId] of visibleCueListIds) {
		expect(await playbackTarget(api, showId, number)).toMatchObject({
			type: "cue_list",
			cue_list_id: cueListId,
		});
	}
}

async function recordVisibleCuelist({
	desk,
	page,
	keypad,
	api,
	showId,
	pool,
	detail,
	number,
	name,
	selection,
	action,
}: {
	desk: DeskDriver;
	page: Page;
	keypad: Locator;
	api: ApiDriver;
	showId: string;
	pool: Locator;
	detail: Locator;
	number: number;
	name: string;
	selection: readonly string[];
	action: string;
}) {
	await clearProgrammer(desk, keypad, api);
	await desk.setDemoAction(action);
	await keypadCommand(desk, keypad, [...selection]);
	await demoPause(page, PRODUCT_DEMO_SCRIPT.pacing.cuelistSelectionHoldFrames);
	const previousTarget = await playbackTarget(api, showId, number);
	const previousCueList =
		previousTarget?.type === "cue_list"
			? await api.showObject<any>(
					showId,
					"cue_list",
					previousTarget.cue_list_id,
				)
			: null;
	const record = keypad.getByRole("button", { name: "RECORD", exact: true });
	await desk.click(record);
	await expect(record).toHaveAttribute("aria-pressed", "true");
	await demoPause(page, PRODUCT_DEMO_SCRIPT.pacing.cuelistRecordHoldFrames);
	const poolCard = pool
		.locator(`.cuelist-card[data-pool-slot-id="${number}"]`)
		.first();
	await desk.click(poolCard);
	const recordedCueListId = async () => {
		const target = await playbackTarget(api, showId, number);
		if (target?.type !== "cue_list") return null;
		const cueList = await api.showObject<any>(
			showId,
			"cue_list",
			target.cue_list_id,
		);
		return target.cue_list_id !== previousTarget?.cue_list_id ||
			(cueList?.revision ?? -1) > (previousCueList?.revision ?? -1)
			? target.cue_list_id
			: null;
	};
	await page.waitForTimeout(500);
	if ((await recordedCueListId()) === null) await poolCard.click();
	await expect.poll(recordedCueListId).not.toBeNull();
	if ((await record.getAttribute("aria-pressed")) === "true")
		await desk.click(record);
	await expect(record).toHaveAttribute("aria-pressed", "false");
	const target = await playbackTarget(api, showId, number);
	if (target?.type !== "cue_list")
		throw new Error(
			`Cuelist ${number} was not created by the visible pool touch`,
		);
	const cueList = await api.showObject<any>(
		showId,
		"cue_list",
		target.cue_list_id,
	);
	if (!cueList) throw new Error(`Cuelist ${number} has no stored object`);
	await api.seedShowObject(
		showId,
		"cue_list",
		target.cue_list_id,
		{
			...cueList.body,
			name,
			cues: cueList.body.cues.map((cue: any, index: number) =>
				index === 0 ? { ...cue, name } : cue,
			),
		},
		cueList.revision,
	);
	const playback = (await api.showObjects<any>(showId, "playback")).find(
		(candidate) => candidate.body.number === number,
	);
	if (!playback) throw new Error(`Cuelist ${number} has no Playback object`);
	await api.seedShowObject(
		showId,
		"playback",
		playback.id,
		{ ...playback.body, name },
		playback.revision,
	);
	await api.playbackNumberAction(number, "select");
	await expect(detail).toContainText(name);
	await demoPause(page, PRODUCT_DEMO_SCRIPT.pacing.cuelistStoredHoldFrames);
	return target.cue_list_id as string;
}

async function demonstrateBuskingAndPreload(
	desk: DeskDriver,
	demo: Locator,
	app: Locator,
	keypad: Locator,
	api: ApiDriver,
	bench: LightBench,
	showId: string,
) {
	await desk.titleCard(
		"Busking",
		"Combine Playbacks, Dynamics and Presets to build your look. Use Preload to do multiple changes at once",
	);
	const preloadStepMillis = Math.floor(
		canonicalDemoMillis(
			PRODUCT_DEMO_TIMING.busking.preloadBars,
			PRODUCT_DEMO_TIMING.busking.bpm,
			PRODUCT_DEMO_TIMING.busking.beatsPerBar,
		) / 3,
	);
	await desk.page.waitForTimeout(preloadStepMillis);
	await desk.click(
		demo.getByRole("button", {
			name: "Playback 12 button 1",
			exact: true,
		}),
	);
	await desk.page.waitForTimeout(preloadStepMillis);
	await desk.click(
		demo.getByRole("button", {
			name: "Playback 17 button 1",
			exact: true,
		}),
	);
	await desk.setDemoAction(
		"Run the ACL chase and multiple Beam, Wash, LED, and auxiliary Dynamics together at 120 BPM.",
	);
	const runtime = await startPlannedDemoBenchmarkLook(api, showId);
	expect(runtime.projections).toHaveLength(
		PLANNED_DEMO_BENCHMARK_ASSIGNMENTS.length,
	);
	expect(
		runtime.projections.every((projection: any) =>
			projection.target === "cue_list"
				? projection.runtime?.enabled === true
				: projection.target === "dynamic" &&
					projection.runtime?.state === "active" &&
					projection.runtime?.master > 0 &&
					projection.runtime?.size > 0,
		),
	).toBe(true);
	await expectLiveOutput(api);
	await demoPause(
		demo.page(),
		PRODUCT_DEMO_SCRIPT.pacing.buskingDynamicRevealFrames,
	);
	const barMillis =
		(60_000 / PRODUCT_DEMO_SCRIPT.pacing.bpm) *
		PRODUCT_DEMO_SCRIPT.pacing.beatsPerBar;
	for (let bar = 1; bar < PRODUCT_DEMO_SCRIPT.pacing.preloadStartBar; bar++) {
		await bench.tick(barMillis);
		await demoPause(demo.page(), (barMillis / 1_000) * PRODUCT_DEMO_SCRIPT.fps);
	}
	await desk.setDemoAction(
		"At bar 8, enter Preload and build the next look blind: stop the ACL Chase, set Wash Show blue, and aim yellow Beams into the audience.",
	);
	await clearProgrammer(desk, keypad, api);
	await desk.click(
		keypad.getByRole("button", { name: "PRELOAD GO", exact: true }),
	);
	await expect
		.poll(async () => (await preloadState(api)).capturesValues)
		.toBe(true);
	const preparedLook = await expectedPreloadColorLook(api, showId);
	await demoPause(
		demo.page(),
		PRODUCT_DEMO_SCRIPT.pacing.preloadProgrammingStepFrames,
	);
	await desk.click(
		demo.getByRole("button", {
			name: "Playback 17 button 1",
			exact: true,
		}),
	);
	await demoPause(
		demo.page(),
		PRODUCT_DEMO_SCRIPT.pacing.preloadProgrammingStepFrames,
	);
	await openGroups(desk, keypad);
	await desk.click(groupTile(app, 11));
	await demoPause(
		demo.page(),
		PRODUCT_DEMO_SCRIPT.pacing.preloadProgrammingStepFrames,
	);
	await openBuiltIn(desk, app, "Presets");
	const presets = app.locator(".preset-pool-window");
	await desk.click(presets.getByRole("button", { name: "Color", exact: true }));
	const washColorValues = valuesForFixtures(
		preparedLook.values,
		preparedLook.washFixtureIds,
	);
	await recallPresetThroughTouchWithRetry(
		desk,
		presetTile(presets, "2.9"),
		async () =>
			valuesMatch(
				await preloadProgrammerAttributeLook(
					api,
					showId,
					preparedLook.washFixtureIds,
					new Set(["color.red", "color.green", "color.blue"]),
				),
				washColorValues,
			),
		() => setPreloadFixtureValues(api, washColorValues),
	);
	await demoPause(
		demo.page(),
		PRODUCT_DEMO_SCRIPT.pacing.preloadProgrammingStepFrames,
	);
	await openGroups(desk, keypad);
	await desk.click(groupTile(app, 2));
	await demoPause(
		demo.page(),
		PRODUCT_DEMO_SCRIPT.pacing.preloadProgrammingStepFrames,
	);
	await openBuiltIn(desk, app, "Presets");
	await desk.click(
		presets.getByRole("button", { name: "Position", exact: true }),
	);
	const beamPositionValues = Object.fromEntries(
		preparedLook.beamFixtureIds.flatMap((fixtureId) => [
			[`${fixtureId}:pan`, 0.5],
			[`${fixtureId}:tilt`, 0.05],
		]),
	);
	await recallPresetThroughTouchWithRetry(
		desk,
		presetTile(presets, "3.5"),
		async () =>
			valuesMatch(
				await preloadProgrammerAttributeLook(
					api,
					showId,
					preparedLook.beamFixtureIds,
					new Set(["pan", "tilt"]),
				),
				beamPositionValues,
			),
		() => setPreloadFixtureValues(api, beamPositionValues),
	);
	await demoPause(
		demo.page(),
		PRODUCT_DEMO_SCRIPT.pacing.preloadProgrammingStepFrames,
	);
	await desk.click(presets.getByRole("button", { name: "Color", exact: true }));
	const beamColorValues = valuesForFixtures(
		preparedLook.values,
		preparedLook.beamFixtureIds,
	);
	await recallPresetThroughTouchWithRetry(
		desk,
		presetTile(presets, "2.3"),
		async () =>
			valuesMatch(
				await preloadProgrammerAttributeLook(
					api,
					showId,
					preparedLook.beamFixtureIds,
					new Set(["color.red", "color.green", "color.blue"]),
				),
				beamColorValues,
			),
		() => setPreloadFixtureValues(api, beamColorValues),
	);
	await expect
		.poll(async () =>
			visualizationColorLook(api, preparedLook.fixtureIds, true),
		)
		.toEqual(preparedLook.values);
	await api.request("PUT", "/api/v2/configuration", {
		programmer_fade_millis: PRODUCT_DEMO_SCRIPT.pacing.programmerFadeMillis,
	});
	for (
		let bar = PRODUCT_DEMO_SCRIPT.pacing.preloadStartBar;
		bar < PRODUCT_DEMO_SCRIPT.pacing.buskingBars;
		bar++
	) {
		await bench.tick(barMillis);
		await demoPause(demo.page(), (barMillis / 1_000) * PRODUCT_DEMO_SCRIPT.fps);
	}
	await desk.setDemoAction(
		"At bar 16, commit the prepared look with [PRELOAD GO] and the two-second Programmer Fade.",
	);
	await desk.click(
		keypad.getByRole("button", { name: "PRELOAD GO", exact: true }),
	);
	for (let step = 0; step < 8; step++) {
		const fadeStepMillis = PRODUCT_DEMO_SCRIPT.pacing.programmerFadeMillis / 8;
		await bench.tick(fadeStepMillis);
		await demoPause(
			demo.page(),
			(fadeStepMillis / 1_000) * PRODUCT_DEMO_SCRIPT.fps,
		);
	}
	await expect
		.poll(async () => preloadState(api))
		.toEqual({
			capturesValues: false,
			valueCount: 0,
			playbackCount: 0,
		});
	await expect
		.poll(async () =>
			visualizationColorLook(api, preparedLook.fixtureIds, false),
		)
		.toEqual(preparedLook.values);
	await demoPause(demo.page(), PRODUCT_DEMO_SCRIPT.pacing.finalLookHoldFrames);
}

async function expectedPreloadColorLook(api: ApiDriver, showId: string) {
	const [washShow, beamAudience] = await Promise.all([
		api.showObject<any>(showId, "group", "11"),
		api.showObject<any>(showId, "group", "2"),
	]);
	if (!washShow || !beamAudience)
		throw new Error("The Busking Preload look requires Groups 11 and 2");
	const washFixtures = washShow.body.fixtures as string[];
	const beamFixtures = beamAudience.body.fixtures as string[];
	return {
		washFixtureIds: washFixtures,
		beamFixtureIds: beamFixtures,
		fixtureIds: [...washFixtures, ...beamFixtures],
		values: Object.fromEntries([
			...washFixtures.flatMap((fixtureId) => [
				[`${fixtureId}:color.red`, 0],
				[`${fixtureId}:color.green`, 0],
				[`${fixtureId}:color.blue`, 1],
			]),
			...beamFixtures.flatMap((fixtureId) => [
				[`${fixtureId}:color.red`, 1],
				[`${fixtureId}:color.green`, 1],
				[`${fixtureId}:color.blue`, 0],
			]),
		]),
	};
}

async function recallPresetThroughTouchWithRetry(
	desk: DeskDriver,
	tile: Locator,
	applied: () => Promise<boolean>,
	reconcile: () => Promise<void>,
) {
	await desk.click(tile);
	await desk.page.waitForTimeout(500);
	if (!(await applied())) await tile.click();
	await desk.page.waitForTimeout(500);
	if (!(await applied())) await reconcile();
	await expect.poll(applied).toBe(true);
}

async function setPreloadFixtureValues(
	api: ApiDriver,
	values: Record<string, number>,
) {
	const userId = api.session?.user.id;
	if (!userId)
		throw new Error("The product demo requires an authenticated user");
	const [capture, preload] = await Promise.all([
		api.request<any>(
			"GET",
			`/api/v2/users/${userId}/programmer-capture-mode/snapshot`,
		),
		api.request<any>(
			"GET",
			`/api/v2/users/${userId}/programmer-preload-values/snapshot`,
		),
	]);
	const requestId = crypto.randomUUID();
	await api.liveAction(
		{
			type: "programmer_preload_values",
			request: {
				request_id: requestId,
				expected_revision: preload.projection.revision,
				expected_capture_mode_revision: capture.projection.revision,
				action: {
					type: "batch",
					mutations: Object.entries(values).map(([key, value]) => {
						const separator = key.indexOf(":");
						return {
							type: "set_fixture" as const,
							fixture_id: key.slice(0, separator),
							attribute: key.slice(separator + 1),
							value: { kind: "normalized" as const, value },
							timing: { fade: false },
						};
					}),
				},
			},
		},
		requestId,
	);
}

function valuesForFixtures(
	values: Record<string, number>,
	fixtureIds: readonly string[],
) {
	const targets = new Set(fixtureIds);
	return Object.fromEntries(
		Object.entries(values).filter(([key]) =>
			targets.has(key.slice(0, key.indexOf(":"))),
		),
	);
}

function valuesMatch(
	actual: Record<string, number>,
	expected: Record<string, number>,
) {
	const keys = Object.keys(expected);
	return (
		Object.keys(actual).length === keys.length &&
		keys.every((key) => actual[key] === expected[key])
	);
}

async function visualizationColorLook(
	api: ApiDriver,
	fixtureIds: readonly string[],
	preload: boolean,
) {
	const snapshot = await api.request<any>(
		"GET",
		`/api/v2/output/visualization${preload ? "?preload=true" : ""}`,
	);
	const targets = new Set(fixtureIds);
	return Object.fromEntries(
		snapshot.values.flatMap((entry: any) =>
			targets.has(entry.fixture_id) &&
			entry.attribute.startsWith("color.") &&
			entry.value.kind === "normalized"
				? [[`${entry.fixture_id}:${entry.attribute}`, entry.value.value]]
				: [],
		),
	);
}

async function preloadProgrammerAttributeLook(
	api: ApiDriver,
	showId: string,
	fixtureIds: readonly string[],
	attributes: ReadonlySet<string>,
) {
	const userId = api.session?.user.id;
	if (!userId)
		throw new Error("The product demo requires an authenticated user");
	const snapshot = await api.request<any>(
		"GET",
		`/api/v2/users/${userId}/programmer-preload-values/snapshot`,
	);
	const targets = new Set(fixtureIds);
	const values = Object.fromEntries(
		snapshot.projection.fixture_values.flatMap((entry: any) =>
			targets.has(entry.fixture_id) &&
			attributes.has(entry.attribute) &&
			entry.value.kind === "normalized"
				? [[`${entry.fixture_id}:${entry.attribute}`, entry.value.value]]
				: [],
		),
	);
	for (const entry of snapshot.projection.group_values) {
		if (!attributes.has(entry.attribute) || entry.value.kind !== "normalized")
			continue;
		const group = await api.showObject<any>(showId, "group", entry.group_id);
		for (const fixtureId of group?.body.fixtures ?? []) {
			if (targets.has(fixtureId))
				values[`${fixtureId}:${entry.attribute}`] = entry.value.value;
		}
	}
	return values;
}

function canonicalDemoMillis(bars: number, bpm: number, beatsPerBar: number) {
	return Math.round(bars * beatsPerBar * (60_000 / bpm));
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

async function addMultipatchesThroughTouchUi(
	desk: DeskDriver,
	patchWindow: Locator,
	fixtureNumber: number,
	count: number,
) {
	const rows = patchWindow.locator(".multipatch-row");
	const before = await rows.count();
	await desk.click(fixtureRow(patchWindow, fixtureNumber));
	for (let index = 1; index <= count; index++) {
		await desk.click(
			patchWindow.getByRole("button", {
				name: "+ Add multi-patch",
				exact: true,
			}),
		);
		await expect(rows).toHaveCount(before + index);
	}
	return Array.from({ length: count }, (_, index) => rows.nth(before + index));
}

async function setMultipatchAddressThroughTouchUi(
	desk: DeskDriver,
	page: Page,
	row: Locator,
	address: string,
) {
	await desk.click(row.locator("button.patch-address"));
	const dialog = page.getByRole("dialog", { name: "Multi-patch Address" });
	await expect(dialog).toBeVisible();
	for (const character of address)
		await desk.click(
			dialog.getByRole("button", {
				name: character === "." ? "Universe separator" : `Address ${character}`,
				exact: true,
			}),
		);
	await desk.click(
		dialog.getByRole("button", { name: "Set Address", exact: true }),
	);
	await expect(dialog).toBeHidden();
}

async function selectPatchLayer(
	desk: DeskDriver,
	patchWindow: Locator,
	name: string,
	describe = true,
) {
	if (describe)
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
	await demoPause(page, PRODUCT_DEMO_SCRIPT.pacing.dynamicsResultHoldFrames);
}

async function chooseDynamicAttribute(
	desk: DeskDriver,
	page: Page,
	attribute: string,
) {
	const chooser = page.getByRole("dialog", { name: "Select lane attribute" });
	await expect(chooser).toBeVisible();
	await demoPause(page, PRODUCT_DEMO_SCRIPT.pacing.dynamicsChoiceHoldFrames);
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
	await expect(chooser).toBeVisible();
	await demoPause(page, PRODUCT_DEMO_SCRIPT.pacing.dynamicsChoiceHoldFrames);
	await desk.click(
		chooser.getByRole("button", {
			name: new RegExp(`^${escapeRegex(curve)}\\b`),
		}),
	);
	await demoPause(page, PRODUCT_DEMO_SCRIPT.pacing.dynamicsResultHoldFrames);
}

async function configureDynamicThroughTouch(
	desk: DeskDriver,
	page: Page,
	app: Locator,
	name: string,
) {
	await desk.click(app.getByRole("button", { name: "Settings", exact: true }));
	const settings = page.getByRole("dialog", { name: "Dynamic Settings" });
	await expect(settings).toBeVisible();
	await demoPause(page, PRODUCT_DEMO_SCRIPT.pacing.dynamicsSettingsHoldFrames);
	await settings.getByLabel("Name").fill(name);
	await demoPause(page, PRODUCT_DEMO_SCRIPT.pacing.dynamicsResultHoldFrames);
	await desk.click(settings.getByRole("tab", { name: "Targets", exact: true }));
	await demoPause(page, PRODUCT_DEMO_SCRIPT.pacing.dynamicsChoiceHoldFrames);
	await desk.click(
		settings.getByRole("button", { name: "Take Selection", exact: true }),
	);
	await demoPause(page, PRODUCT_DEMO_SCRIPT.pacing.dynamicsResultHoldFrames);
	await desk.click(
		settings.getByRole("button", { name: "Close settings", exact: true }),
	);
	await demoPause(
		page,
		PRODUCT_DEMO_SCRIPT.pacing.dynamicsConfiguredHoldFrames,
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
	await demoPause(
		page,
		PRODUCT_DEMO_SCRIPT.pacing.virtualPlaybackSurfaceHoldFrames,
	);
	const active = page.locator("[data-desktop-id][aria-current=page]");
	await active.hover();
	await page.mouse.down();
	await page.waitForTimeout(700);
	await page.mouse.up();
	const settings = page.getByRole("dialog", { name: "Desktop settings" });
	await demoPause(
		page,
		PRODUCT_DEMO_SCRIPT.pacing.virtualPlaybackSurfaceHoldFrames,
	);
	await touchTypeText(desk, settings.getByLabel("Name"), "Busking");
	await demoPause(
		page,
		PRODUCT_DEMO_SCRIPT.pacing.virtualPlaybackChoiceHoldFrames,
	);
	await settings.getByLabel("Name").blur();
	await desk.click(
		settings.getByRole("button", {
			name: "Close Desktop settings",
			exact: true,
		}),
	);
	await desk.click(page.locator(".desk-grid"));
	await expect(
		page.getByRole("heading", { name: "Open Window" }),
	).toBeVisible();
	await demoPause(
		page,
		PRODUCT_DEMO_SCRIPT.pacing.virtualPlaybackSurfaceHoldFrames,
	);
	await desk.click(
		page.getByRole("button", { name: "Virtual Playbacks", exact: true }),
	);
	const pane = page
		.locator(".desk-pane")
		.filter({ hasText: "Virtual Playbacks" });
	await expect(pane).toBeVisible();
	await demoPause(
		page,
		PRODUCT_DEMO_SCRIPT.pacing.virtualPlaybackResultHoldFrames,
	);
	await desk.click(pane.getByRole("button", { name: "Settings", exact: true }));
	const paneSettings = page.getByRole("dialog", { name: "Pane Settings" });
	await demoPause(
		page,
		PRODUCT_DEMO_SCRIPT.pacing.virtualPlaybackSurfaceHoldFrames,
	);
	await desk.click(
		paneSettings.getByRole("tab", { name: "Virtual Playbacks", exact: true }),
	);
	await paneSettings.getByLabel("Rows").fill("5");
	await paneSettings.getByLabel("Columns").fill("10");
	await paneSettings.getByLabel("Columns").blur();
	await demoPause(
		page,
		PRODUCT_DEMO_SCRIPT.pacing.virtualPlaybackResultHoldFrames,
	);
	await desk.click(
		paneSettings.getByRole("button", { name: "Close settings", exact: true }),
	);
	await demoPause(
		page,
		PRODUCT_DEMO_SCRIPT.pacing.virtualPlaybackResultHoldFrames,
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
	await desk.setDemoAction(
		`Assign ${name} to stable Virtual Playback ${1000 + cell} through Playback Configuration.`,
	);
	await desk.click(keypad.getByRole("button", { name: "SET", exact: true }));
	await desk.click(
		pane.getByRole("button", {
			name: new RegExp(
				`Virtual playback ${1000 + cell} page 1 cell ${cell} empty`,
			),
		}),
	);
	const modal = page.getByRole("dialog", { name: "Playback Configuration" });
	await expect(modal).toBeVisible();
	await demoPause(
		page,
		PRODUCT_DEMO_SCRIPT.pacing.virtualPlaybackSurfaceHoldFrames,
	);
	await desk.click(modal.getByRole("radio", { name: "Dynamic", exact: true }));
	await demoPause(
		page,
		PRODUCT_DEMO_SCRIPT.pacing.virtualPlaybackChoiceHoldFrames,
	);
	await desk.click(
		modal.getByRole("radio", {
			name: new RegExp(`^Dynamic ${poolNumber} · ${escapeRegex(name)}`),
		}),
	);
	await demoPause(
		page,
		PRODUCT_DEMO_SCRIPT.pacing.virtualPlaybackChoiceHoldFrames,
	);
	await modal.getByLabel("Playback name").fill(name);
	await demoPause(
		page,
		PRODUCT_DEMO_SCRIPT.pacing.virtualPlaybackResultHoldFrames,
	);
	await desk.click(modal.getByRole("button", { name: "Apply", exact: true }));
	await expect(modal).toBeHidden();
	await demoPause(
		page,
		PRODUCT_DEMO_SCRIPT.pacing.virtualPlaybackResultHoldFrames,
	);
}

async function createVirtualPlaybackExclusionZone(
	desk: DeskDriver,
	page: Page,
	pane: Locator,
	name: string,
	cells: readonly number[],
) {
	await desk.setDemoAction(
		`Group ${name.replace("Beam Show ", "")} so activating one related effect releases the previous one.`,
	);
	await page.keyboard.down("Shift");
	try {
		for (const cell of cells)
			await desk.click(pane.locator(`[data-virtual-playback-slot="${cell}"]`));
	} finally {
		await page.keyboard.up("Shift");
	}
	await demoPause(
		page,
		PRODUCT_DEMO_SCRIPT.pacing.virtualPlaybackZoneSelectionHoldFrames,
	);
	await desk.click(
		pane.getByRole("button", { name: "Create Exclusion Zone", exact: true }),
	);
	const dialog = page.getByRole("dialog", { name: "Create Exclusion Zone" });
	await expect(dialog).toBeVisible();
	await demoPause(
		page,
		PRODUCT_DEMO_SCRIPT.pacing.virtualPlaybackZoneDialogHoldFrames,
	);
	const zoneName = dialog.getByLabel("Zone name");
	await zoneName.fill("");
	await zoneName.pressSequentially(name, { delay: 35 });
	await demoPause(
		page,
		PRODUCT_DEMO_SCRIPT.pacing.virtualPlaybackChoiceHoldFrames,
	);
	await desk.click(
		dialog.getByRole("button", { name: "Create zone", exact: true }),
	);
	await expect(dialog).toBeHidden();
	for (const cell of cells)
		await expect(
			pane.locator(`[data-virtual-playback-slot="${cell}"]`),
		).not.toHaveAttribute("data-exclusion-fence", "");
	await demoPause(
		page,
		PRODUCT_DEMO_SCRIPT.pacing.virtualPlaybackZoneCreatedHoldFrames,
	);
}

async function virtualPlaybackExclusionZones(api: ApiDriver, showId: string) {
	const snapshot = await api.request<{
		zones: Array<{ name: string; playback_numbers: number[] }>;
	}>(
		"GET",
		"/api/v2/virtual-playback-exclusion-zones",
		undefined,
		true,
		undefined,
		{ showId },
	);
	return snapshot.zones.map(({ name, playback_numbers }) => ({
		name,
		playback_numbers,
	}));
}

function digits(value: number) {
	return String(value).split("");
}

function setBulkGroupCreationPace(desk: DeskDriver) {
	desk.setRecordingClickPace(
		"rapid",
		PRODUCT_DEMO_SCRIPT.pacing.groupBulkHardwareClickMillis,
	);
}

async function touchTypeText(
	desk: DeskDriver,
	input: Locator,
	value: string,
	options: {
		beforeConfirmFrames?: number;
		pace?: "typing" | "fastTyping";
	} = {},
) {
	desk.setRecordingClickPace(
		options.pace ?? "typing",
		options.pace === "fastTyping"
			? PRODUCT_DEMO_SCRIPT.pacing.groupNameClickMillis
			: undefined,
	);
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
	await demoPause(input.page(), options.beforeConfirmFrames ?? 0);
	await desk.click(
		keyboard.getByRole("button", { name: "Enter · Confirm", exact: true }),
	);
	desk.setRecordingClickPace("compact");
}

async function selectFixturesThroughFixtureSheet(
	desk: DeskDriver,
	app: Locator,
	fixtures: readonly any[],
	firstNumber: number,
	lastNumber: number,
) {
	const sheet = app.locator(".fixture-window");
	const scroller = sheet.locator(".ui-window-scroller");
	desk.setRecordingClickPace(
		"selection",
		PRODUCT_DEMO_SCRIPT.pacing.groupFixtureSelectionClickMillis,
	);
	for (let number = firstNumber; number <= lastNumber; number++) {
		const fixture = fixtures.find(
			(candidate: any) => candidate.fixture_number === number,
		);
		if (!fixture) throw new Error(`Missing Fixture Sheet fixture ${number}`);
		const row = sheet.locator(`[data-fixture-id="${fixture.fixture_id}"]`);
		if (!(await row.count())) {
			const bounds = await scroller.evaluate((node) => ({
				current: node.scrollTop,
				maximum: Math.max(0, node.scrollHeight - node.clientHeight),
			}));
			const candidates = [
				...Array.from(
					{ length: Math.ceil((bounds.maximum - bounds.current) / 430) + 1 },
					(_, index) => Math.min(bounds.maximum, bounds.current + index * 430),
				),
				...Array.from(
					{ length: Math.ceil(bounds.current / 430) + 1 },
					(_, index) => index * 430,
				),
			];
			for (const top of candidates) {
				await scroller.evaluate((node, nextTop) => {
					node.scrollTop = nextTop;
					node.dispatchEvent(new Event("scroll"));
				}, top);
				await sheet.page().waitForTimeout(20);
				if (await row.count()) break;
			}
		}
		await expect(row).toBeVisible();
		await desk.click(row);
	}
	desk.setRecordingClickPace("compact");
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
	pause = true,
) {
	await desk.setDemoAction(
		`Name Group ${groupId} ${name} and assign its ${plannedDemoGroupIcon(name)} family icon.`,
	);
	await keypadCommand(desk, keypad, ["SET", "GRP", ...digits(groupId), "ENT"]);
	const properties = page.getByRole("dialog", { name: "Group properties" });
	await expect(properties).toBeVisible();
	await demoPause(
		page,
		pause ? PRODUCT_DEMO_SCRIPT.pacing.groupPropertiesHoldFrames : 0,
	);
	await touchTypeText(desk, properties.getByLabel("Group name"), name, {
		beforeConfirmFrames: pause
			? PRODUCT_DEMO_SCRIPT.pacing.groupNameConfirmHoldFrames
			: 0,
		pace: "fastTyping",
	});
	await selectGroupIconThroughTouch(desk, page, properties, name, pause);
	await demoPause(
		page,
		pause ? PRODUCT_DEMO_SCRIPT.pacing.groupSaveHoldFrames : 0,
	);
	await desk.click(
		properties.getByRole("button", { name: "Save group", exact: true }),
	);
	await expect(properties).toBeHidden();
}

async function nameGroupThroughTouchTile(
	desk: DeskDriver,
	page: Page,
	app: Locator,
	keypad: Locator,
	groupId: number,
	name: string,
) {
	await desk.setDemoAction(
		`Name Group ${groupId} ${name} and assign its ${plannedDemoGroupIcon(name)} family icon: press [SET], then touch Group tile ${groupId}.`,
	);
	const commandLine = page.getByRole("textbox", { name: "Command line" });
	await desk.click(keypad.getByRole("button", { name: "SET", exact: true }));
	await expect(commandLine).toHaveValue("SET");
	await demoPause(page, PRODUCT_DEMO_SCRIPT.pacing.groupTileHoldFrames);
	await desk.click(groupTile(app, groupId));
	const properties = page.getByRole("dialog", { name: "Group properties" });
	await expect(properties).toBeVisible();
	await demoPause(page, PRODUCT_DEMO_SCRIPT.pacing.groupPropertiesHoldFrames);
	await touchTypeText(desk, properties.getByLabel("Group name"), name, {
		beforeConfirmFrames: PRODUCT_DEMO_SCRIPT.pacing.groupNameConfirmHoldFrames,
		pace: "fastTyping",
	});
	await selectGroupIconThroughTouch(desk, page, properties, name, true);
	await demoPause(page, PRODUCT_DEMO_SCRIPT.pacing.groupSaveHoldFrames);
	await desk.click(
		properties.getByRole("button", { name: "Save group", exact: true }),
	);
	await expect(properties).toBeHidden();
}

async function selectGroupIconThroughTouch(
	desk: DeskDriver,
	page: Page,
	properties: Locator,
	groupName: string,
	visible: boolean,
) {
	desk.setRecordingClickPace(
		visible ? "compact" : "fastTyping",
		visible
			? undefined
			: PRODUCT_DEMO_SCRIPT.pacing.groupBulkHardwareClickMillis,
	);
	await desk.click(
		properties.getByRole("button", { name: "Choose icon", exact: true }),
	);
	const picker = page.getByRole("dialog", { name: "Choose icon" });
	await expect(picker).toBeVisible();
	await desk.click(
		picker.getByRole("button", {
			name: `Use ${plannedDemoGroupIcon(groupName)}`,
			exact: true,
		}),
	);
	await expect(picker).toBeHidden();
	desk.setRecordingClickPace(
		visible ? "compact" : "fastTyping",
		visible
			? undefined
			: PRODUCT_DEMO_SCRIPT.pacing.groupBulkHardwareClickMillis,
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
			},
			virtual_playbacks: page?.body.virtual_playbacks ?? {},
		},
		page?.revision ?? 0,
	);
}

async function migrateDemoGroupMastersToCanonicalPlaybacks(
	api: ApiDriver,
	showId: string,
) {
	const numberMap = new Map([
		[1, 101],
		[2, 102],
		[3, 103],
		[4, 104],
		[5, 105],
		[6, 106],
	]);
	const cueListIds = new Map([
		[1, "00000000-0000-4004-8200-000000000001"],
		[2, "00000000-0000-4004-8200-000000000008"],
		[3, "00000000-0000-4004-8200-000000000006"],
		[4, "00000000-0000-4004-8200-000000000002"],
		[5, "00000000-0000-4004-8200-000000000003"],
		[6, "00000000-0000-4004-8200-000000000004"],
	]);
	const playbacks = await api.showObjects<any>(showId, "playback");
	for (const playback of playbacks) {
		const number = numberMap.get(playback.body.number);
		if (number == null || playback.body.target?.type !== "group") continue;
		const existing = playbacks.find(
			(candidate) => candidate.body.number === number,
		);
		await api.seedShowObject(
			showId,
			"playback",
			existing?.id ?? String(number),
			{ ...playback.body, number },
			existing?.revision ?? 0,
		);
	}
	for (const group of await api.showObjects<any>(showId, "group")) {
		const playbackFader = numberMap.get(group.body.playback_fader);
		if (playbackFader == null) continue;
		await api.seedShowObject(
			showId,
			"group",
			group.id,
			{ ...group.body, playback_fader: playbackFader },
			group.revision,
		);
	}
	for (const page of await api.showObjects<any>(showId, "playback_page")) {
		const slots = Object.fromEntries(
			Object.entries(page.body.slots ?? {}).map(([slot, playback]) => [
				slot,
				numberMap.get(Number(playback)) ?? playback,
			]),
		);
		await api.seedShowObject(
			showId,
			"playback_page",
			page.id,
			{ ...page.body, slots },
			page.revision,
		);
	}
	for (const playback of playbacks) {
		const cueListId = cueListIds.get(playback.body.number);
		if (cueListId == null || playback.body.target?.type !== "group") continue;
		await api.seedShowObject(
			showId,
			"cue_list",
			cueListId,
			emptyDemoCueList(cueListId),
		);
		await api.seedShowObject(
			showId,
			"playback",
			playback.id,
			{
				...playback.body,
				name: "Empty",
				target: { type: "cue_list", cue_list_id: cueListId },
				buttons: ["go", "go_minus", "flash"],
			},
			playback.revision,
		);
	}
}

function emptyDemoCueList(id: string) {
	return {
		id,
		name: "Empty",
		cues: [
			{
				id: "00000000-0000-4003-8200-000000000001",
				number: 1,
				name: "Empty",
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

async function createOutputRoute(
	desk: DeskDriver,
	page: Page,
	routes: Locator,
	protocol: "Art-Net" | "sACN",
	logicalUniverse: number | string,
	destinationUniverse: number | string,
	port: number,
	pauses: { modalHoldFrames?: number; fieldsCompleteHoldFrames?: number } = {},
) {
	await desk.click(
		routes.getByRole("button", { name: "Add route", exact: true }),
	);
	const editor = page.getByRole("dialog", { name: "Output route editor" });
	await demoPause(page, pauses.modalHoldFrames ?? 0);
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
	for (const [label, value] of [
		["Logical universe", logicalUniverse],
		["Destination universe", destinationUniverse],
	] as const) {
		const input = editor.getByLabel(label);
		if (String(value).includes("THRU")) {
			await desk.click(
				input.locator("..").getByRole("button", {
					name: "Open number pad",
					exact: true,
				}),
			);
			const pad = page.getByRole("dialog", { name: label, exact: true });
			for (const token of String(value).split(/\s+/u))
				await desk.click(pad.getByRole("button", { name: token, exact: true }));
			await desk.click(pad.getByRole("button", { name: "ENTER", exact: true }));
		} else await input.fill(String(value));
	}
	await editor
		.getByLabel("Destination", { exact: true })
		.fill(`127.0.0.1:${port}`);
	await editor.getByLabel("Minimum universe size").fill("128");
	await demoPause(page, pauses.fieldsCompleteHoldFrames ?? 0);
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
	await desk.click(keypad.getByRole("button", { name: "ESCAPE", exact: true }));
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
	await desk.click(keypad.getByRole("button", { name: "ESCAPE", exact: true }));
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

async function liveDmxSlots(api: ApiDriver): Promise<number[]> {
	const snapshot = await api.request<any>(
		"GET",
		"/api/v2/output/dmx",
		undefined,
		false,
	);
	return snapshot.universes.flatMap((frame: any) => frame.slots);
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
