import { expect } from "../../apps/control-ui/e2e/bench/fixtures";
import { scenario } from "../../apps/control-ui/e2e/bench/scenario";
import {
	fixture,
	groupRange,
} from "../../apps/control-ui/e2e/bench/selectionContract";

scenario(
	"BENCH-COMMAND-SELECTION-001",
	"enters a fixture range through real desk keys and visible ENT",
	async (t) => {
		await t.app.open();
		await t.app.expect.ready();

		await t.command.execute("FIXTURE 1 THRU 3");
		await t.expect.selection(fixture(1), fixture(2), fixture(3));
		await t.command.expect(/^(?:FIXTURE|GROUP)$/);
	},
);

scenario(
	"BENCH-COMMAND-SELECTION-002",
	"preserves ordered typed sources while Highlight power stays independent",
	async (t) => {
		await t.app.open();
		await t.app.expect.ready();

		await t.selection.targets(fixture(3), fixture(1));
		await t.expect.selection(fixture(3), fixture(1));

		await t.selection.groups.range(1, 4);
		await t.expect.selection(groupRange(1, 4));
		const before = await t.selection.observe();

		await t.highlight.via.api.on();
		await t.highlight.via.api.off();
		const after = await t.selection.observe();
		expect(after.selected).toEqual(before.selected);
		expect(after.expression).toEqual(before.expression);

		await t.hardware.connect();
		try {
			await t.highlight.via.osc.toggle();
			await t.highlight.via.osc.toggle();
		} finally {
			await t.hardware.disconnect();
		}
	},
);
