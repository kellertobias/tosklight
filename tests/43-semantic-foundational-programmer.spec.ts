// @bench-semantic-world

import { StoreMode } from "./bench/groups-presets/groupScenario";
import { PaneType } from "./bench/window-system/paneTypes";
import { scenario } from "./bench/core/scenario";
import {
	fixture,
	fixtureRange,
} from "./bench/command-selection/selectionContract";
import { Show } from "./bench/show/showScenario";

scenario(
	"PROG-001",
	"values retain selection until replacement while leading Plus continues it",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();

		for (const command of ["1 + 2 AT 50", "AT 25", "3 AT 75", "+ 4 AT 100"])
			await t.command.execute(command);

		await t.expect.selection(fixture(3), fixture(4));
		await t.clock.advanceBy("3s");
		await t.expectFixtureDMX(fixtureRange(1, 2), { Intensity: 64 });
		await t.expectFixtureDMX(fixtureRange(3, 4), { Intensity: 255 });
		await t.expectFixtureDMX(fixtureRange(5, 12), { Intensity: 0 });
	},
);

scenario(
	"PROG-002",
	"relative values spread across the live ordered Group",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();
		await openProgrammerPools(t);

		await t.selection.fixtures.via.fixtureSheet.range(1, 10);
		await t.group.via.pool.store(1, { mode: StoreMode.Overwrite });
		await t.command.execute("GROUP 1 AT 0 THRU 100");
		await t.clock.advanceBy("3s");
		await expectIntensities(
			t,
			[0, 28, 57, 85, 113, 142, 170, 198, 227, 255, 0, 0],
		);

		const sourceOrder = [12, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
		await t.selection.fixtures.via.fixtureSheet.items(...sourceOrder);
		await t.group.via.pool.store(1, { mode: StoreMode.Overwrite });
		await t.group.expect(1).fixtures(...sourceOrder);
		await t.clock.advanceBy("0ms");
		await expectIntensities(
			t,
			[26, 51, 77, 102, 128, 153, 179, 204, 230, 255, 0, 0],
		);
	},
);

scenario(
	"PROG-002",
	"fixture ranges and retained selections spread through the desk command line",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();

		await t.command.execute("1 THRU 5 AT 20 THRU 50");
		await t.clock.advanceBy("3s");
		await expectIntensities(t, [51, 70, 89, 108, 128]);

		await t.command.execute("1 THRU 5");
		await t.command.execute("AT 0 THRU 50");
		await t.clock.advanceBy("3s");
		await expectIntensities(t, [0, 32, 64, 96, 128]);

		await t.command.execute("1 THRU 5");
		await t.command.execute("AT 100 THRU 0 THRU 100");
		await t.clock.advanceBy("3s");
		await expectIntensities(t, [255, 128, 0, 128, 255]);
	},
);

scenario(
	"PROG-003",
	"newer fixture intensity wins LTP and releases back to its Group value",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();

		await t.command.execute("GROUP 1 AT 50");
		await t.command.execute("1 AT 25");
		await t.clock.advanceBy("3s");
		await t.expectFixtureDMX(fixture(1), { Intensity: 64 });
		await t.expectFixtureDMX(fixtureRange(2, 12), { Intensity: 128 });

		await t.encoder.intensity.dimmer.release();
		await t.clock.advanceBy("0ms");
		await t.expectFixtureDMX(fixtureRange(1, 12), { Intensity: 128 });
	},
);

scenario(
	"PROG-004",
	"Clear removes selection first and programmer values second",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();

		await t.command.execute("1 + 2 AT 50");
		await t.clock.advanceBy("3s");
		await t.keypad.press(["CLR"]);
		await t.expect.selection();
		await t.clock.advanceBy("0ms");
		await t.expectFixtureDMX(fixtureRange(1, 2), { Intensity: 128 });

		await t.keypad.press(["CLR"]);
		await t.clock.advanceBy("0ms");
		await t.expectFixtureDMX(fixtureRange(1, 12), { Intensity: 0 });
	},
);

async function expectIntensities(
	t: Parameters<Parameters<typeof scenario>[2]>[0],
	values: readonly number[],
) {
	for (const [index, intensity] of values.entries())
		await t.expectFixtureDMX(fixture(index + 1), { Intensity: intensity });
}

async function openProgrammerPools(
	t: Parameters<Parameters<typeof scenario>[2]>[0],
) {
	const desktop = t.desktop.configure("Foundational Programmer");
	desktop.addPane(PaneType.Fixtures, {
		slug: "fixtures",
		column: 1,
		row: 1,
		width: 12,
		height: 18,
	});
	desktop.addPane(PaneType.Groups, {
		slug: "groups",
		column: 13,
		row: 1,
		width: 12,
		height: 18,
	});
	await desktop.apply();
}
