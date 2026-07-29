import { startSlowVisualizationClient } from "../tools/slow-visualization-client.mjs";
import type { ApiDriver } from "./bench/core/api";
import { BrowserScenarioWorld } from "./bench/core/browserScenario";
import { expect, test } from "./bench/core/fixtures";
import { fixture } from "./bench/output/fixtureDmxContract";
import {
	countFixtureInstances,
	installDeterministicLargeStage,
} from "./bench/performance/stageLargeScene";
import {
	measureStagePerformance,
	type StageMeasurementProfile,
} from "./bench/performance/stageMeasurement";
import { Show } from "./bench/show/showScenario";
import {
	PaneType,
	StageRenderQuality,
	StageView,
} from "./bench/window-system/paneTypes";
import { readPatchSnapshot } from "./support/operator/patch";

for (const profile of ["default-stage", "large-stage"] as const) {
	test(`STAGE-PERF-${profile === "default-stage" ? "001" : "002"} @ui › collect informational ${profile} frontend and output evidence`, async ({
		api,
		bench,
		desk,
		page,
		show,
	}, testInfo) => {
		testInfo.setTimeout(240_000);
		page.setDefaultTimeout(20_000);
		const world = new BrowserScenarioWorld(
			page,
			desk,
			bench,
			api,
			show,
			testInfo,
		);
		let failure: unknown;
		try {
			await world.show.use(Show.DefaultStage);
			await world.stage.prepareSocketRecoveryProof();
			const scene =
				profile === "large-stage"
					? await installDeterministicLargeStage(api)
					: await defaultStageScene(api);
			await world.app.open();
			await world.app.expect.ready();

			const evidence = await measureStagePerformance({
				page,
				api,
				testInfo,
				profile,
				fixtureRecords: scene.fixtureRecords,
				fixtureInstances: scene.fixtureInstances,
				noStageExercise: () => exerciseWithoutStage(world, profile),
				exercise: () => exerciseStage(world, api, profile),
			});

			expect(evidence.frontend.sceneBuilds.length).toBeGreaterThan(0);
			expect(evidence.frontend.renders.length).toBeGreaterThan(0);
			expect(evidence.frontend.frames.length).toBeGreaterThan(0);
			expect(evidence.server.outputDelta.frames_sent).toBeGreaterThan(0);
			expect(evidence.server.noStageOutputDelta.frames_sent).toBeGreaterThan(0);
			expect(evidence.server.outputDelta.deadline_misses).toBe(0);
			expect(evidence.server.noStageOutputDelta.deadline_misses).toBe(0);
			expect(evidence.server.outputComparison.boundedWindowGatePassed).toBe(
				true,
			);
			expect(evidence.server.visualizationWindow.projections).toBeGreaterThan(
				0,
			);
			expect(evidence.server.visualizationWindow.finalStreamQueueDepth).toBe(0);
			if (profile === "large-stage") {
				expect(scene).toMatchObject({
					fixtureRecords: 970,
					fixtureInstances: 1_000,
					dynamicInstances: 20,
					staticControlInstances: 440,
					occupiedSlots: 18_840,
					universes: 37,
				});
				expect(
					evidence.server.visualizationWindow.streamQueueDrops +
						evidence.server.visualizationWindow.streamSendFailures,
				).toBeGreaterThan(0);
			}
			expect(evidence.packagedWebView).toMatchObject({
				controlled: false,
				measured: false,
			});
		} catch (reason) {
			failure = reason;
			throw reason;
		} finally {
			await world.finish(failure);
		}
	});
}

async function defaultStageScene(api: Parameters<typeof readPatchSnapshot>[0]) {
	const patch = await readPatchSnapshot(api);
	return {
		fixtureRecords: patch.fixtures.length,
		fixtureInstances: countFixtureInstances(patch.fixtures),
	};
}

async function exerciseWithoutStage(
	world: BrowserScenarioWorld,
	profile: StageMeasurementProfile,
): Promise<void> {
	const desktop = world.desktop.configure(`No Stage comparison · ${profile}`);
	desktop.addPane(PaneType.Fixtures, {
		slug: "comparison-fixtures",
		column: 1,
		row: 1,
		width: 24,
		height: 18,
	});
	await desktop.apply();
	await world.selection.clear();
	await world.selection.fixtures.via.api.item(101);
	await world.timing.programmerFade.via.api.set("1s");
	await world.command.execute("AT 20");
	await world.clock.freeRunFor("1s");
	await world.expectFixtureDMX(fixture(101), { Intensity: 51 });
}

async function exerciseStage(
	world: BrowserScenarioWorld,
	api: ApiDriver,
	profile: StageMeasurementProfile,
): Promise<void> {
	const desktop = world.desktop.configure(`Stage performance · ${profile}`);
	const live = desktop.addPane(PaneType.Stage, {
		slug: "performance-live",
		column: 1,
		row: 1,
		width: 8,
		height: 18,
	});
	const duplicate = desktop.addPane(PaneType.Stage, {
		slug: "performance-live-duplicate",
		column: 9,
		row: 1,
		width: 8,
		height: 18,
	});
	const preload = desktop.addPane(PaneType.Stage, {
		slug: "performance-preload",
		column: 17,
		row: 1,
		width: 8,
		height: 18,
	});
	await live.configure({
		view: StageView.ThreeDimensional,
		followPreload: false,
		renderQuality: StageRenderQuality.LinesAndBeams,
	});
	await duplicate.configure({
		view: StageView.ThreeDimensional,
		followPreload: false,
		renderQuality: StageRenderQuality.LinesAndBeams,
	});
	await preload.configure({
		view: StageView.ThreeDimensional,
		followPreload: true,
		renderQuality: StageRenderQuality.LinesAndBeams,
	});
	await desktop.apply();
	await world.clock.advanceBy("1ms");
	await world.stage.expectLane(live, "live");
	await world.stage.expectLane(duplicate, "live");
	await world.stage.expectLane(preload, "preload");
	await world.stage.expectFixedCamera(live);
	await world.stage.expectFixedCamera(preload);
	await world.stage.expectSharedLaneFeed({
		normalSubscribers: 1,
		preloadSubscribers: 1,
	});

	await world.selection.clear();
	await world.selection.fixtures.via.api.item(101);
	await world.preload.via.api.start();
	await world.preload.setFixtureValue({
		fixture: 101,
		attribute: "intensity",
		value: { kind: "normalized", value: 0.35 },
	});
	for (let frame = 0; frame < 5; frame++) {
		await world.clock.advanceBy("100ms");
		await world.stage.waitForChangingFrame();
	}
	await world.preload.release();
	await world.timing.programmerFade.via.api.set("1s");
	await world.encoder.position.pan.via.api.set(75);
	await world.command.execute("AT 80");
	await world.clock.freeRunFor("1s");
	await world.stage.waitForChangingFrame();
	await world.expectFixtureDMX(fixture(101), { Intensity: 204 });

	await world.stage.verifyRenderQualities(live, async (_quality, index) => {
		await world.encoder.position.pan.via.api.set(76 + index);
		await world.clock.advanceBy("100ms");
		await world.stage.waitForChangingFrame();
	});

	if (profile === "large-stage")
		await exerciseSlowTcpVisualizationClient(api, world.clock, world.stage);

	await world.stage.stallVisualizationDelivery();
	await world.encoder.intensity.dimmer.via.api.set(40);
	for (let frame = 0; frame < 5; frame++) await world.clock.advanceBy("100ms");
	await world.stage.resumeVisualizationDelivery();
	await world.clock.advanceBy("100ms");
	await world.stage.waitForChangingFrame();
	await world.stage.expectLane(live, "live");
	await world.stage.expectLane(preload, "preload");

	await world.stage.expectContextRecovery(live);
	const before2d = await world.stage.captureRetainedSceneState();
	await live.configure({
		view: StageView.TwoDimensional,
		followPreload: false,
		renderQuality: StageRenderQuality.ImprovedBeams,
	});
	await world.stage.expectRendererReleasedSince(before2d);
	const before3d = await world.stage.captureRetainedSceneState();
	await live.configure({
		view: StageView.ThreeDimensional,
		followPreload: false,
		renderQuality: StageRenderQuality.ImprovedBeams,
	});
	await world.stage.expectRendererCreatedSince(before3d);

	await world.selection.clear();
	await duplicate.remove();
	await world.stage.expectLaneSubscribers({ normal: 1, preload: 1 });
	await live.remove();
	await world.stage.expectLaneSubscribers({ normal: 0, preload: 1 });
}

async function exerciseSlowTcpVisualizationClient(
	api: ApiDriver,
	clock: BrowserScenarioWorld["clock"],
	stage: BrowserScenarioWorld["stage"],
): Promise<void> {
	const token = api.session?.token;
	if (!token)
		throw new Error("Slow visualization client requires a session token");
	const client = await startSlowVisualizationClient(api.baseUrl, token);
	try {
		await clock.freeRunFor("10s");
	} finally {
		client.close();
	}
	await stage.expectLaneSubscribers({ normal: 1, preload: 1 });
}
