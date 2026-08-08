import { expect } from "../bench/core/fixtures";
import { scenario } from "../bench/core/scenario";
import {
	fixture,
	fixtureRange,
} from "../bench/command-selection/selectionContract";

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
