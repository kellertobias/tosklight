import { fixtureSheetIncludesFixture } from "../apps/light-desktop/src/windows/fixtureSheetFilters";
import { fixtureSheetTargets } from "../apps/light-desktop/src/windows/fixtureSheetTargets";
import { BrowserScenarioWorld } from "./bench/core/browserScenario";
import { expect, test } from "./bench/core/fixtures";
import { installDeterministicLargeStage } from "./bench/performance/stageLargeScene";
import { Show } from "./bench/show/showScenario";
import {
	PaneType,
	StageRenderQuality,
	StageView,
} from "./bench/window-system/paneTypes";

interface LargeTierPerformanceDiagnostics {
	output: {
		frames_sent: number;
		deadline_misses: number;
	};
}

test("PLAN76-LARGE-001 @ui @benchmark › keeps Fixture Sheet, Programmer, and output live beside the exact 1,000-instance Stage tier", async ({
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
		const patch = await api.patch();
		const excludedScenery = patch.fixtures.filter(
			(fixture) => !fixtureSheetIncludesFixture(fixture),
		);
		expect(excludedScenery).toHaveLength(20);
		const excludedSceneryRows = excludedScenery.reduce(
			(count, fixture) => count + Math.max(1, fixture.logical_heads.length + 1),
			0,
		);
		expect(excludedSceneryRows).toBe(20);
		const programmableRows = patch.fixtures
			.filter(fixtureSheetIncludesFixture)
			.reduce(
				(count, fixture) => count + fixtureSheetTargets(fixture).length,
				0,
			);
		expect(programmableRows).toBe(1_890);
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
		await expect(
			fixtures.root().getByText("0 selected", { exact: true }),
		).toBeVisible();
		const fixtureTable = fixtures.root().getByRole("table");
		await expect(fixtureTable).toHaveAttribute(
			"aria-rowcount",
			String(programmableRows + 1),
		);
		expect(
			await fixtureTable.locator(".ui-data-table-row:not(.header)").count(),
		).toBeLessThan(80);
		const fixtureScroller = fixtures
			.root()
			.locator(".fixture-table > .ui-window-scroller");
		const initialLayout = await fixtureTable
			.locator(".ui-data-table-row.header")
			.evaluate((header) => ({
				top: header.getBoundingClientRect().top,
				columnWidths: Array.from(header.children, (column) =>
					Number(column.getBoundingClientRect().width.toFixed(2)),
				),
			}));
		await fixtureScroller.evaluate((scroller) => {
			scroller.scrollTop = 12_000;
			scroller.dispatchEvent(new Event("scroll"));
		});
		await expect
			.poll(async () =>
				Number(
					(await fixtureTable
						.locator(".ui-data-table-row:not(.header)")
						.first()
						.getAttribute("data-table-index")) ?? 0,
				),
			)
			.toBeGreaterThan(100);
		const scrolledLayout = await fixtureTable
			.locator(".ui-data-table-row.header")
			.evaluate((header) => ({
				top: header.getBoundingClientRect().top,
				columnWidths: Array.from(header.children, (column) =>
					Number(column.getBoundingClientRect().width.toFixed(2)),
				),
			}));
		expect(Math.abs(scrolledLayout.top - initialLayout.top)).toBeLessThan(1);
		expect(scrolledLayout.columnWidths).toEqual(initialLayout.columnWidths);
		await fixtureScroller.evaluate((scroller) => {
			scroller.scrollTop = 0;
			scroller.dispatchEvent(new Event("scroll"));
		});
		await expect
			.poll(async () =>
				Number(
					(await fixtureTable
						.locator(".ui-data-table-row:not(.header)")
						.first()
						.getAttribute("data-table-index")) ?? -1,
				),
			)
			.toBe(0);

		const outputBefore = await api.request<LargeTierPerformanceDiagnostics>(
			"GET",
			"/api/v2/diagnostics/performance",
		);
		await world.clock.freeRunFor("1s");

		const fixtureSelection = await world.programmerActionTiming.expectAction(
			{
				source: "websocket",
				route: "software",
				action: "selection",
				requiresOutputFrame: false,
			},
			() => world.selection.fixtures.via.fixtureSheet.item(101),
		);
		await expect(
			fixtures.root().getByText("1 selected", { exact: true }),
		).toBeVisible();

		const command = await world.programmerActionTiming
			.expectKeyboardCommand("FIXTURE 101 AT 35")
			.catch((error: unknown) => {
				console.error(bench.recentLog());
				throw error;
			});
		await expect(
			page.getByRole("textbox", { name: "Command line", exact: true }),
		).toHaveValue("FIXTURE");

		const outputAfter = await api.request<LargeTierPerformanceDiagnostics>(
			"GET",
			"/api/v2/diagnostics/performance",
		);
		expect(outputAfter.output.frames_sent).toBeGreaterThan(
			outputBefore.output.frames_sent,
		);
		// This debug E2E path proves control and UI isolation. Release-mode throughput and zero
		// output deadline misses remain enforced by the packaged performance benchmark, because
		// hosted debug runners are not a representative output-rate target.

		// Exact 1,000-instance Stage rendering may stutter. The capacity contract
		// requires liveness and isolation instead of a real-time canvas cadence.
		await world.stage.expectLane(stage, "live");
		await expect(stage.root().locator(".stage-3d-canvas canvas")).toBeVisible();
		expect(page.isClosed()).toBe(false);

		await testInfo.attach("plan76-interactive-large-isolation.json", {
			body: Buffer.from(
				JSON.stringify(
					{
						scene: {
							fixtureRecords: scene.fixtureRecords,
							fixtureInstances: scene.fixtureInstances,
						},
						fixtureSheet: {
							open: true,
							selectedFixture: 101,
						},
						programmerTiming: {
							fixtureSelection,
							command,
						},
						output: {
							before: outputBefore.output,
							after: outputAfter.output,
						},
						stage: {
							realTimeCadenceRequired: false,
							liveAndRecoverable: true,
						},
					},
					null,
					2,
				),
			),
			contentType: "application/json",
		});
		await stage.screenshot("plan76-interactive-large-stage-and-fixture-sheet");
	} catch (reason) {
		failure = reason;
		throw reason;
	} finally {
		await world.finish(failure);
	}
});
