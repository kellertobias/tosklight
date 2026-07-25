import { scenario } from "../../apps/control-ui/e2e/bench/scenario";
import {
	fixture,
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
