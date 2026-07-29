// @bench-semantic-world

import { scenario } from "./bench/core/scenario";
import { fixture } from "./bench/output/fixtureDmxContract";
import { Show } from "./bench/show/showScenario";
import {
	PaneType,
	StageRenderQuality,
	StageView,
} from "./bench/window-system/paneTypes";

scenario(
	"STAGE-001",
	"authoritative Live and Preload Stage surfaces stay current, recover independently, and preserve output",
	async (t) => {
		await t.show.use(Show.DefaultStage);
		await t.stage.prepareSocketRecoveryProof();
		await t.app.open();
		await t.app.expect.ready();

		const desktop = t.desktop.configure("Stage runtime proof");
		const live = desktop.addPane(PaneType.Stage, {
			slug: "live-stage",
			column: 1,
			row: 1,
			width: 12,
			height: 18,
		});
		const preload = desktop.addPane(PaneType.Stage, {
			slug: "preload-stage",
			column: 13,
			row: 1,
			width: 12,
			height: 18,
		});
		await live.configure({
			view: StageView.TwoDimensional,
			followPreload: false,
			renderQuality: StageRenderQuality.LinesAndBeams,
		});
		await preload.configure({
			view: StageView.ThreeDimensional,
			followPreload: true,
			renderQuality: StageRenderQuality.LinesAndBeams,
		});
		await desktop.apply();
		await t.clock.advanceBy("1ms");
		await t.stage.expectLane(live, "live", {
			sharedFeed: {
				normalSubscribers: 1,
				preloadSubscribers: 1,
			},
		});
		await t.stage.expectLane(preload, "preload");

		await t.selection.clear();
		await t.stage.selectFixture(live, 101);
		await preload.configure({
			view: StageView.TwoDimensional,
			followPreload: true,
			renderQuality: StageRenderQuality.LinesAndBeams,
		});
		await t.preload.start();
		await t.preload.setFixtureValue({
			fixture: 101,
			attribute: "intensity",
			value: { kind: "normalized", value: 0.35 },
		});
		await t.clock.advanceBy("100ms");
		await t.stage.waitForChangingFrame();
		await t.stage.expectLane(live, "live", {
			fixtureNumber: 101,
			percent: 0,
		});
		await t.stage.expectLane(preload, "preload", {
			fixtureNumber: 101,
			percent: 35,
		});
		await t.preload.release();

		await live.configure({
			view: StageView.ThreeDimensional,
			followPreload: false,
			renderQuality: StageRenderQuality.LinesAndBeams,
		});
		await preload.configure({
			view: StageView.ThreeDimensional,
			followPreload: true,
			renderQuality: StageRenderQuality.LinesAndBeams,
		});
		await t.stage.expectQuality(live, StageRenderQuality.LinesAndBeams);
		await t.stage.expectLane(preload, "preload", { fixedCamera: true });
		const retainedScene = await t.stage.captureRetainedSceneState();
		const changingFrames = await t.stage.beginChangingFrameMeasurement();
		await t.timing.programmerFade.via.api.set("1s");
		await t.encoder.position.pan.via.api.set(75);
		await t.command.execute("AT 80");
		for (let frame = 1; frame <= 10; frame++) {
			await t.clock.advanceBy("100ms");
			await t.stage.waitForChangingFrame();
			if (frame === 5)
				await t.expectFixtureDMX(fixture(101), { Intensity: 102 });
		}
		await t.expectFixtureDMX(fixture(101), { Intensity: 204 });
		await t.stage.assertChangingFrameBudget(changingFrames);

		for (const [qualityIndex, quality] of Object.values(
			StageRenderQuality,
		).entries()) {
			await live.configure({
				view: StageView.ThreeDimensional,
				followPreload: false,
				renderQuality: quality,
			});
			await t.encoder.position.pan.via.api.set(76 + qualityIndex);
			await t.clock.advanceBy("100ms");
			await t.stage.waitForChangingFrame();
			await t.stage.expectQuality(live, quality);
		}
		await t.stage.expectNoStructuralRebuildSince(retainedScene);
		await t.stage.expectSettledRendererIdle();
		await t.stage.expectCanvasCapture(live);

		await t.stage.interruptVisualization();
		await t.stage.expectStale(live);
		await t.encoder.intensity.dimmer.via.api.set(40);
		await t.clock.advanceBy("1s");
		await t.expectFixtureDMX(fixture(101), { Intensity: 102 });
		await t.stage.resumeVisualization();
		await t.stage.expectRecovered(live);
		await t.stage.expectRecovered(preload);
		await t.stage.expectLane(live, "live", {
			sharedFeed: {
				normalSubscribers: 1,
				preloadSubscribers: 1,
			},
		});

		await t.stage.expectContextRecovery(live);
		const beforeViewSwitch = await t.stage.captureRetainedSceneState();
		await live.configure({
			view: StageView.TwoDimensional,
			followPreload: false,
			renderQuality: StageRenderQuality.ImprovedBeams,
		});
		await t.stage.expectRendererReleasedSince(beforeViewSwitch);
		const beforeRestore = await t.stage.captureRetainedSceneState();
		await live.configure({
			view: StageView.ThreeDimensional,
			followPreload: false,
			renderQuality: StageRenderQuality.ImprovedBeams,
		});
		await t.stage.expectRendererCreatedSince(beforeRestore);
		await t.stage.expectQuality(live, StageRenderQuality.ImprovedBeams);

		await live.configure({
			view: StageView.ThreeDimensional,
			followPreload: false,
			renderQuality: StageRenderQuality.LinesOnly,
		});
		await t.clock.advanceBy("100ms");
		await t.stage.waitForChangingFrame();
		await t.stage.expectQuality(live, StageRenderQuality.LinesOnly);
		await t.stage.resumeVisualization({
			reload: true,
			waitBeforeReloadMillis: 700,
		});
		await t.stage.expectLane(live, "live");
		await t.stage.expectLane(preload, "preload");
		await t.stage.expectQuality(live, StageRenderQuality.LinesOnly);
	},
);
