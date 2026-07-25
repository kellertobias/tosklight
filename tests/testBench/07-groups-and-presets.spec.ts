import { PaneType } from "../../apps/control-ui/e2e/bench/paneTypes";
import { scenario } from "../../apps/control-ui/e2e/bench/scenario";
import {
	groupRange,
} from "../../apps/control-ui/e2e/bench/selectionContract";
import {
	StoreMode,
} from "../../apps/control-ui/e2e/bench/groupScenario";
import { Show } from "../../apps/control-ui/e2e/bench/showScenario";

scenario(
	"BENCH-GROUP-001",
	"Group routes preserve order and distinguish stored empty from absent",
	async (t) => {
		await t.show.use(Show.DefaultStage);
		await t.app.open();
		await t.app.expect.ready();
		await openPools(t);

		await t.selection.fixtures.via.api.items(3, 1);
		await t.group.via.api.store(10, { mode: StoreMode.Overwrite });
		await t.group.expect(10).fixtures(3, 1);

		await t.selection.fixtures.via.api.item(2);
		await t.group.via.pool.store(10, { mode: StoreMode.Merge });
		await t.group.expect(10).fixtures(3, 1, 2);

		await t.selection.fixtures.via.api.item(1);
		await t.group.via.keypad.store(10, { mode: StoreMode.Subtract });
		await t.group.expect(10).fixtures(3, 2);

		await t.group.via.pool.edit(10, {
			name: "Ordered stage pair",
			color: "#1bd6ec",
			icon: "★",
		});
		await t.group.expect(10).metadata({
			name: "Ordered stage pair",
			color: "#1bd6ec",
			icon: "★",
		});

		await t.selection.clear();
		await t.group.via.api.store(11, { mode: StoreMode.Overwrite });
		await t.group.expect(11).present();
		await t.group.expect(11).empty();
		await t.group.via.api.delete(11);
		await t.group.expect(11).absent();

		await t.hardware.connect();
		try {
			await t.selection.fixtures.via.api.item(4);
			await t.group.via.osc.store(12, { mode: StoreMode.Overwrite });
			await t.group.expect(12).fixtures(4);
		} finally {
			await t.hardware.disconnect();
		}

		await t.selection.groups.via.api.range(10, 12);
		await t.expect.selection(groupRange(10, 12));
	},
);

async function openPools(
	t: Parameters<Parameters<typeof scenario>[2]>[0],
) {
	const desktop = t.desktop.configure("Groups and Presets");
	desktop.addPane(PaneType.Groups, {
		slug: "groups",
		column: 1,
		row: 1,
		width: 12,
		height: 18,
	});
	desktop.addPane(PaneType.Presets, {
		slug: "presets",
		column: 13,
		row: 1,
		width: 12,
		height: 18,
	});
	await desktop.apply();
}
