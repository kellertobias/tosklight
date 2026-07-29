import { BrowserScenarioWorld } from "./bench/core/browserScenario";
import { expect, test } from "./bench/core/fixtures";
import { installDeterministicLargeStage } from "./bench/performance/stageLargeScene";
import { Show } from "./bench/show/showScenario";
import {
	PaneType,
	StageRenderQuality,
	StageView,
} from "./bench/window-system/paneTypes";

test("PLAN76-LARGE-001 @ui @benchmark › opens the exact 1,000-instance interactive tier with Stage and Fixture Sheet", async ({
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
		const scene = await installDeterministicLargeStage(api);
		expect(scene).toMatchObject({
			fixtureRecords: 970,
			fixtureInstances: 1_000,
			dynamicInstances: 20,
			staticControlInstances: 440,
			occupiedSlots: 18_840,
			universes: 37,
		});

		await world.app.open();
		await world.app.expect.ready();
		const desktop = world.desktop.configure("Plan 76 · Interactive large tier");
		const stage = desktop.addPane(PaneType.Stage, {
			slug: "interactive-large-stage",
			column: 1,
			row: 1,
			width: 12,
			height: 18,
		});
		const fixtures = desktop.addPane(PaneType.Fixtures, {
			slug: "interactive-large-fixtures",
			column: 13,
			row: 1,
			width: 12,
			height: 18,
		});
		await stage.configure({
			view: StageView.ThreeDimensional,
			followPreload: false,
			renderQuality: StageRenderQuality.LinesAndBeams,
		});
		await desktop.apply();
		await stage.expect.visible();
		await fixtures.expect.visible();
		await expect(fixtures.root().locator(".fixture-window")).toBeVisible();
		await expect(stage.root().locator(".stage-3d-canvas canvas")).toBeVisible();

		await world.clock.freeRunFor("1s");
		await world.stage.waitForChangingFrame();
		await stage.screenshot("plan76-interactive-large-stage-and-fixture-sheet");
	} catch (reason) {
		failure = reason;
		throw reason;
	} finally {
		await world.finish(failure);
	}
});
