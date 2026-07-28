import { BrowserScenarioWorld } from "./bench/core/browserScenario";
import { expect, test } from "./bench/core/fixtures";
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
		testInfo.setTimeout(180_000);
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
				exercise: () => exerciseStage(world, profile),
			});

			expect(evidence.frontend.sceneBuilds.length).toBeGreaterThan(0);
			expect(evidence.frontend.renders.length).toBeGreaterThan(0);
			expect(evidence.frontend.frames.length).toBeGreaterThan(0);
			expect(evidence.server.outputDelta.frames_sent).toBeGreaterThan(0);
			expect(evidence.server.visualizationWindow.projections).toBeGreaterThan(0);
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

async function exerciseStage(
	world: BrowserScenarioWorld,
	profile: StageMeasurementProfile,
): Promise<void> {
	const desktop = world.desktop.configure(`Stage performance · ${profile}`);
	const stage = desktop.addPane(PaneType.Stage, {
		slug: "performance-stage",
		column: 1,
		row: 1,
		width: 24,
		height: 18,
	});
	await stage.configure({
		view: StageView.ThreeDimensional,
		followPreload: false,
		renderQuality: StageRenderQuality.LinesAndBeams,
	});
	await desktop.apply();
	await world.clock.advanceBy("1ms");
	await world.stage.expectLane(stage, "live");

	await world.selection.clear();
	await world.selection.fixtures.via.api.item(201);
	await world.timing.programmerFade.via.api.set("1s");
	await world.encoder.position.pan.via.api.set(75);
	await world.command.execute("AT 80");
	await world.clock.advanceBy("500ms");
	await world.stage.waitForChangingFrame();
	await world.clock.advanceBy("500ms");
	await world.stage.waitForChangingFrame();
}
