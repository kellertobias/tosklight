import { BrowserScenarioWorld } from "../../apps/control-ui/e2e/bench/browserScenario";
import { DeskDriver } from "../../apps/control-ui/e2e/bench/desk";
import { expect, test } from "../../apps/control-ui/e2e/bench/fixtures";
import { PaneType } from "../../apps/control-ui/e2e/bench/paneTypes";
import { scenario } from "../../apps/control-ui/e2e/bench/scenario";
import {
	fixture,
	fixtureRange,
	groupRange,
} from "../../apps/control-ui/e2e/bench/selectionContract";

scenario(
	"BENCH-SELECTION-ROUTES-001",
	"visible panes, keypad, API, and OSC converge on the ordered selection oracle",
	async (t) => {
		await t.app.open();
		await t.app.expect.ready();
		await openSelectionDesktop(t);

		await t.selection.clear();
		await t.selection.fixtures.via.fixtureSheet.items(1, 3, 2);
		await t.expect.selection(fixture(1), fixture(3), fixture(2));

		await t.selection.clear();
		await t.selection.fixtures.via.stage.range(1, 4);
		await t.expect.selection(fixtureRange(1, 4));

		await t.selection.clear();
		await t.selection.groups.via.pool.range(1, 4);
		await t.expect.selection(groupRange(1, 4));

		await t.selection.fixtures.via.keypad.range(2, 5);
		await t.expect.selection(fixtureRange(2, 5));

		await t.selection.fixtures.via.api.items(5, 2, 4);
		await t.expect.selection(fixture(5), fixture(2), fixture(4));

		await t.hardware.connect();
		try {
			await t.selection.fixtures.via.osc.item(6);
			await t.expect.selection(fixture(6));
		} finally {
			await t.hardware.disconnect();
		}
	},
);

scenario(
	"BENCH-SELECTION-ROUTES-002",
	"unqualified selection records and replays its seeded eligible route",
	async (t) => {
		await t.app.open();
		await t.app.expect.ready();
		const request = {
			action: "replace" as const,
			targets: [fixture(4), fixture(1)],
		};

		await t.selection.targets(...request.targets);
		await t.expect.selection(...request.targets);
		const report = t.selection.routeReports.at(-1);
		expect(report).toMatchObject({
			seed: expect.any(String),
			actionIndex: 0,
			action: "replace",
			targetKinds: ["fixture", "fixture"],
			candidates: ["api", "keypad"],
			selected: expect.stringMatching(/^(api|keypad)$/),
		});

		await t.selection.clear();
		if (!report)
			throw new Error("Unqualified selection did not report its route");
		await t.selection.routeChoice.replay(report, request);
		await t.expect.selection(...request.targets);
	},
);

test("BENCH-SELECTION-ROUTES-003 @bench @touch › touch route uses a real touch-enabled browser context", async ({
	api,
	bench,
	browser,
	show,
}, testInfo) => {
	const context = await browser.newContext({
		baseURL: bench.baseUrl,
		hasTouch: true,
		viewport: { width: 1280, height: 720 },
	});
	const page = await context.newPage();
	const desk = new DeskDriver(
		page,
		testInfo.title,
		api.session?.desk.id ?? null,
	);
	try {
		const t = new BrowserScenarioWorld(page, desk, bench, api, show, testInfo);
		await t.app.open();
		await t.app.expect.ready();
		await openSelectionDesktop(t);
		await t.selection.clear();
		await t.selection.fixtures.via.touch.range(9, 11);
		await t.expect.selection(fixtureRange(9, 11));
	} finally {
		await desk.dispose();
		await context.close();
	}
});

async function openSelectionDesktop(t: BrowserScenarioWorld) {
	const desktop = t.desktop.configure("Selection route proof");
	desktop.addPane(PaneType.Fixtures, {
		slug: "fixtures",
		column: 1,
		row: 1,
		width: 8,
		height: 18,
	});
	desktop.addPane(PaneType.Stage, {
		slug: "stage",
		column: 9,
		row: 1,
		width: 8,
		height: 18,
	});
	desktop.addPane(PaneType.Groups, {
		slug: "groups",
		column: 17,
		row: 1,
		width: 8,
		height: 18,
	});
	await desktop.apply();
}
