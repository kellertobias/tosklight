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
		await t.stage.expectLane(live, "live");
		await t.stage.expectLane(preload, "preload");

		await t.selection.clear();
		await t.selection.fixtures.via.click.item(201);
		await live.configure({
			view: StageView.ThreeDimensional,
			followPreload: false,
			renderQuality: StageRenderQuality.LinesAndBeams,
		});
		const retainedScene = await t.stage.captureRetainedSceneState();
		const changingFrames = await t.stage.beginChangingFrameMeasurement();
		await t.timing.programmerFade.via.api.set("1s");
		await t.encoder.position.pan.via.api.set(75);
		await t.command.execute("AT 80");
		await t.clock.advanceBy("500ms");
		await t.stage.waitForChangingFrame();
		await t.expectFixtureDMX(fixture(201), { Intensity: 102 });
		await t.clock.advanceBy("500ms");
		await t.stage.waitForChangingFrame();
		await t.expectFixtureDMX(fixture(201), { Intensity: 204 });
		await t.stage.assertChangingFrameBudget(changingFrames);

		for (const quality of Object.values(StageRenderQuality)) {
			await live.configure({
				view: StageView.ThreeDimensional,
				followPreload: false,
				renderQuality: quality,
			});
			await t.stage.expectQuality(live, quality);
		}
		await t.stage.expectNoStructuralRebuildSince(retainedScene);
		await t.stage.expectSettledRendererIdle();

		await t.stage.interruptVisualization();
		await t.stage.expectStale(live);
		await t.encoder.intensity.dimmer.via.api.set(40);
		await t.clock.advanceBy("1s");
		await t.expectFixtureDMX(fixture(201), { Intensity: 102 });
		await t.stage.resumeVisualization();
		await t.stage.expectRecovered(live);
		await t.stage.expectRecovered(preload);

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
	},
);
