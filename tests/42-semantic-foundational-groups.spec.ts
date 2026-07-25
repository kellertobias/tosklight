// @bench-semantic-world

import {
	StoreMode,
} from "../apps/control-ui/e2e/bench/groupScenario";
import { PaneType } from "../apps/control-ui/e2e/bench/paneTypes";
import { scenario } from "../apps/control-ui/e2e/bench/scenario";
import {
	fixture,
	fixtureRange,
	group,
	groupRange,
} from "../apps/control-ui/e2e/bench/selectionContract";
import { Show } from "../apps/control-ui/e2e/bench/showScenario";

scenario(
	"GROUP-003",
	"derived Group follows source ordering",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();
		await openFoundationPools(t);

		await t.command.execute("GROUP 1 DIV 2");
		await t.group.via.pool.store(5, { mode: StoreMode.Overwrite });

		const sourceOrder = [12, 1, 2, 8, 4, 5, 6, 7, 9, 10, 11];
		await t.selection.fixtures.via.api.items(...sourceOrder);
		await t.group.via.pool.store(1, { mode: StoreMode.Overwrite });

		await t.group.expect(1).fixtures(...sourceOrder);
		await t.group.expect(5).fixtures(12, 2, 4, 6, 9, 11);
		await t.group.expect(4).empty();
	},
);

scenario(
	"GROUP-004",
	"frozen Group survives source edits and keeps unpatched fixtures programmable",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();
		await openFoundationPools(t);

		await t.command.execute("DEGRP 1");
		await t.group.via.pool.store(5, { mode: StoreMode.Overwrite });

		const sourceOrder = [12, 1, 2, 4, 5, 6, 7, 8, 9, 10, 11];
		await t.selection.fixtures.via.api.items(...sourceOrder);
		await t.group.via.pool.store(1, { mode: StoreMode.Overwrite });
		await t.patch.via.ui.unpatch(3);
		await t.patch.expect(3).unpatched();

		await t.app.open();
		await t.app.expect.ready();
		await t.command.execute("GROUP 5");
		await t.encoder.intensity.dimmer.via.ui.set(50);
		await t.group.expect(5).fixtures(
			1,
			2,
			3,
			4,
			5,
			6,
			7,
			8,
			9,
			10,
			11,
			12,
		);
		await t.clock.advanceBy("3s");
		await t.expectFixtureDMX(fixtureRange(1, 2), { Intensity: 128 });
		await t.expectFixtureDMXAbsent(fixture(3));
		await t.expectFixtureDMX(fixtureRange(4, 12), { Intensity: 128 });
	},
);

scenario(
	"GROUP-005",
	"stored empty Groups remain distinct from missing references",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();
		await openFoundationPools(t);

		await t.group.via.api.delete(4);
		await t.group.expect(4).absent();
		await t.command.execute("GROUP 1 THRU 5");
		await t.expect.selection(groupRange(1, 5));

		await t.selection.clear();
		await t.group.via.pool.store(4, { mode: StoreMode.Overwrite });
		await t.group.expect(4).present();
		await t.group.expect(4).empty();
		await t.group.via.pool.select(4);
		await t.expect.selection(group(4));
		await t.encoder.intensity.dimmer.via.ui.set(50);
		await t.clock.advanceBy("3s");
		await t.expectFixtureDMX(fixtureRange(1, 12), { Intensity: 0 });

		await t.selection.fixtures.via.fixtureSheet.item(1);
		await t.expect.selection(fixture(1));
		await t.group.via.pool.store(4, { mode: StoreMode.Merge });
		await t.group.expect(4).fixtures(1);
		await t.clock.advanceBy("0ms");
		await t.expectFixtureDMX(fixture(1), { Intensity: 128 });
	},
);

async function openFoundationPools(
	t: Parameters<Parameters<typeof scenario>[2]>[0],
) {
	const desktop = t.desktop.configure("Foundational Groups");
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
