// @bench-semantic-world

import {
	fixture,
	fixtureRange,
	group,
} from "./bench/command-selection/selectionContract";
import { scenario } from "./bench/core/scenario";
import { Show } from "./bench/show/showScenario";

scenario(
	"DIM-001",
	"ordered Group edits retain their live value and append re-added fixtures",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();

		for (const command of [
			"GROUP 3 AT 50",
			"GROUP 3 + 5 + 6",
			"RECORD GROUP 3",
			"GROUP 3 - 2 + 2",
			"RECORD GROUP 3",
		])
			await t.command.execute(command);

		await t.group.expect(3).fixtures(1, 3, 4, 5, 6, 2);
		await t.clock.advanceBy("3s");
		await t.expectFixtureDMX(fixtureRange(1, 6), { Intensity: 128 });
	},
);

scenario(
	"DIM-002",
	"Lightning Desk command reaches the exact rendered output boundary",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();

		await t.timing.programmerFade.setCommandLineAtEnabled(true);
		await t.command.execute("GROUP 1 AT 50");
		await t.clock.advanceBy("1.5s");
		await t.expectFixtureDMX(fixtureRange(1, 12), { Intensity: 64 });
		await t.clock.advanceBy("1.5s");
		await t.expectFixtureDMX(fixtureRange(1, 12), { Intensity: 128 });
		await t.clock.advanceBy("1ms");
		await t.expectFixtureDMX(fixtureRange(1, 12), { Intensity: 128 });
	},
);

scenario(
	"CMD-001",
	"Fixture and Group default modes toggle while explicit prefixes stay scoped",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();

		await t.keypad.press(["GRP"]);
		await t.command.expect("GROUP");
		await t.keypad.press(["ENT"]);
		await t.command.expect("GROUP");
		await t.keypad.press(["SHIFT", "GRP", "SHIFT"]);
		await t.command.expect("FIXTURE");
		await t.keypad.press(["ENT"]);
		await t.command.expect("FIXTURE");

		await t.command.execute("GROUP 1 + 2");
		await t.expect.selection(group(1), fixture(2));
	},
);
