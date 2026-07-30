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
		testInfo.setTimeout(profile === "large-stage" ? 600_000 : 240_000);
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
				noStageExercise: () =>
					exerciseWithoutStage(
						world,
						api,
						profile,
						scene.staticControlFixtureNumber,
						scene.fixtureIdsByNumber[scene.staticControlFixtureNumber],
					),
				exercise: () =>
					exerciseStage(
						world,
						api,
						profile,
						scene.fixtureIdsByNumber,
						scene.staticControlFixtureNumber,
					),
			});

			expect(evidence.frontend.sceneBuilds.length).toBeGreaterThan(0);
			expect(evidence.frontend.renders.length).toBeGreaterThan(0);
			expect(evidence.frontend.frames.length).toBeGreaterThan(0);
			expect(evidence.server.outputDelta.frames_sent).toBeGreaterThan(0);
			expect(evidence.server.noStageOutputDelta.frames_sent).toBeGreaterThan(0);
			if (profile === "default-stage") {
				expect(evidence.server.outputDelta.deadline_misses).toBe(0);
				expect(evidence.server.noStageOutputDelta.deadline_misses).toBe(0);
				expect(evidence.server.outputComparison.boundedWindowGatePassed).toBe(
					true,
				);
			}
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
			if (failure === undefined && profile === "large-stage")
				await page.goto("about:blank", { waitUntil: "commit" });
		}
	});
}

async function defaultStageScene(api: Parameters<typeof readPatchSnapshot>[0]) {
	const patch = await readPatchSnapshot(api);
	return {
		fixtureRecords: patch.fixtures.length,
		fixtureInstances: countFixtureInstances(patch.fixtures),
		fixtureIdsByNumber: Object.fromEntries(
			patch.fixtures.flatMap((fixture) =>
				fixture.fixture_number == null
					? []
					: [[fixture.fixture_number, fixture.fixture_id]],
			),
		),
		staticControlFixtureNumber: 101,
	};
}

async function exerciseWithoutStage(
	world: BrowserScenarioWorld,
	api: ApiDriver,
	profile: StageMeasurementProfile,
	staticControlFixtureNumber: number,
	staticControlFixtureId: string,
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
	await world.selection.fixtures.via.api.item(staticControlFixtureNumber);
	await setLiveFixtureIntensity(api, staticControlFixtureId, 0.2);
	await world.clock.freeRunFor("1s");
	await world.expectFixtureDMX(fixture(staticControlFixtureNumber), {
		Intensity: 51,
	});
}

async function exerciseStage(
	world: BrowserScenarioWorld,
	api: ApiDriver,
	profile: StageMeasurementProfile,
	fixtureIdsByNumber: Record<number, string>,
	staticControlFixtureNumber: number,
): Promise<void> {
	if (profile === "large-stage")
		return exerciseLargeStage(
			world,
			api,
			fixtureIdsByNumber,
			staticControlFixtureNumber,
		);
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
	await world.preload.setFixtureValueById({
		fixtureId: fixtureIdsByNumber[101],
		attribute: "intensity",
		value: { kind: "normalized", value: 0.35 },
	});
	for (let frame = 0; frame < 5; frame++) {
		await world.clock.advanceBy("100ms");
		await world.stage.waitForChangingFrame();
	}
	await world.preload.release();
	await world.selection.clear();
	await world.selection.fixtures.via.api.item(staticControlFixtureNumber);
	await setLiveFixtureIntensity(
		api,
		fixtureIdsByNumber[staticControlFixtureNumber],
		0.8,
	);
	await world.clock.freeRunFor("1s");
	await world.expectFixtureDMX(fixture(staticControlFixtureNumber), {
		Intensity: 204,
	});
	await world.selection.clear();
	await world.selection.fixtures.via.api.item(101);
	await world.encoder.position.pan.via.api.set(75);
	await world.clock.advanceBy("100ms");
	await world.stage.waitForChangingFrame();

	await world.stage.verifyRenderQualities(live, async (_quality, index) => {
		await world.encoder.position.pan.via.api.set(76 + index);
		await world.clock.advanceBy("100ms");
		await world.stage.waitForChangingFrame();
	});

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

async function exerciseLargeStage(
	world: BrowserScenarioWorld,
	api: ApiDriver,
	fixtureIdsByNumber: Record<number, string>,
	staticControlFixtureNumber: number,
): Promise<void> {
	const desktop = world.desktop.configure(
		"Large-show desk responsiveness · Fixture Sheet + Stage",
	);
	desktop.addPane(PaneType.Fixtures, {
		slug: "performance-fixture-sheet",
		column: 1,
		row: 1,
		width: 16,
		height: 18,
	});
	const live = desktop.addPane(PaneType.Stage, {
		slug: "performance-live",
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
	await desktop.apply();
	await world.clock.advanceBy("1ms");
	await world.stage.expectLane(live, "live");
	await world.stage.expectFixedCamera(live);
	await world.stage.expectLaneSubscribers({ normal: 1, preload: 0 });

	await world.selection.clear();
	await world.selection.fixtures.via.api.item(staticControlFixtureNumber);
	await setLiveFixtureIntensity(
		api,
		fixtureIdsByNumber[staticControlFixtureNumber],
		0.8,
	);
	await world.clock.freeRunFor("1s");
	await world.expectFixtureDMX(fixture(staticControlFixtureNumber), {
		Intensity: 204,
	});

	await world.selection.clear();
	await world.selection.fixtures.via.api.item(101);
	for (let frame = 0; frame < 5; frame++) {
		await world.encoder.position.pan.via.api.set(75 + frame);
		await world.clock.advanceBy("100ms");
		await world.stage.waitForChangingFrame();
	}
	await live.remove();
}

async function setLiveFixtureIntensity(
	api: ApiDriver,
	fixtureId: string,
	value: number,
): Promise<void> {
	if (!fixtureId) throw new Error("Stage control fixture identity is unavailable");
	if (!api.session) throw new Error("Stage performance session is unavailable");
	const userId = api.session.user.id;
	const [values, capture] = await Promise.all([
		api.request<{ projection: { revision: number } }>(
			"GET",
			`/api/v2/users/${encodeURIComponent(userId)}/programmer-values/snapshot`,
		),
		api.request<{ projection: { revision: number } }>(
			"GET",
			`/api/v2/users/${encodeURIComponent(userId)}/programmer-capture-mode/snapshot`,
		),
	]);
	await api.request(
		"POST",
		`/api/v2/users/${encodeURIComponent(userId)}/programmer-values/actions`,
		{
			request_id: crypto.randomUUID(),
			expected_revision: values.projection.revision,
			expected_capture_mode_revision: capture.projection.revision,
			action: {
				type: "batch",
				mutations: [
					{
						type: "set_fixture",
						fixture_id: fixtureId,
						attribute: "intensity",
						value: { kind: "normalized", value },
						timing: {
							fade: false,
							fade_millis: null,
							delay_millis: null,
						},
					},
				],
			},
		},
		true,
	);
}
