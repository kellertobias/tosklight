import type { StageShiftSelectionResult } from "../../apps/control-ui/e2e/bench/command-selection/selectionVisibleStageScenario";
import { expect } from "../../apps/control-ui/e2e/bench/core/fixtures";
import { PaneType } from "../../apps/control-ui/e2e/bench/window-system/paneTypes";
import { scenario } from "../../apps/control-ui/e2e/bench/core/scenario";
import {
	fixture,
	fixtureRange,
} from "../../apps/control-ui/e2e/bench/command-selection/selectionContract";

scenario(
	"BENCH-SELECTION-ROUTES-004",
	"OSC multi-target and range input waits for each authoritative command revision",
	async (t) => {
		await t.app.open();
		await t.app.expect.ready();
		await t.selection.clear();
		await t.hardware.connect();
		try {
			await t.selection.fixtures.via.osc.range(6, 8);
			await t.expect.selection(fixtureRange(6, 8));

			await t.selection.fixtures.via.osc.items(8, 6, 7);
			await t.expect.selection(fixture(8), fixture(6), fixture(7));
		} finally {
			await t.hardware.disconnect();
		}
	},
);

scenario(
	"BENCH-SELECTION-ROUTES-005",
	"Stage Shift-click reports and preserves the actual visible Stage order",
	async (t) => {
		await t.app.open();
		await t.app.expect.ready();
		const desktop = t.desktop.configure("Stage visible order proof");
		desktop.addPane(PaneType.Stage, {
			slug: "stage",
			column: 1,
			row: 1,
			width: 24,
			height: 18,
		});
		await desktop.apply();

		await t.selection.clear();
		await t.selection.fixtures.via.click.item(1);
		const result = (await t.selection.fixtures.via.shiftClick.item(
			5,
		)) as StageShiftSelectionResult;

		expect(result).toMatchObject({
			order: "stage-visible",
			anchor: 1,
			target: 5,
		});
		expect(result.selection.length).toBeGreaterThan(1);
		const observation = await t.selection.observe();
		expect(observation.targets.map((target) => target.number)).toEqual(
			result.selection,
		);
		expect(observation.expression).toEqual(result.expression);
	},
);
