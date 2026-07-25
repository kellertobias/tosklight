import { expect } from "../../apps/control-ui/e2e/bench/fixtures";
import { PaneType } from "../../apps/control-ui/e2e/bench/paneTypes";
import {
	PresetFamily,
} from "../../apps/control-ui/e2e/bench/presetScenario";
import { scenario } from "../../apps/control-ui/e2e/bench/scenario";
import {
	dereferencedGroup,
	fixture,
	group,
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

scenario(
	"BENCH-GROUP-002",
	"live Group references refresh while dereferenced captures retain concrete fixtures",
	async (t) => {
		await t.show.use(Show.DefaultStage);
		await t.app.open();
		await t.app.expect.ready();

		await t.selection.fixtures.via.api.items(1, 2);
		await t.group.via.api.store(80, { mode: StoreMode.Overwrite });

		await t.selection.targets(dereferencedGroup(80));
		await t.group.via.api.store(80, { mode: StoreMode.Subtract });
		await t.group.expect(80).empty();
		await t.expect.selection(fixture(1), fixture(2));

		await t.group.via.api.store(80, { mode: StoreMode.Overwrite });
		await t.group.via.api.select(80);
		await t.expect.selection(group(80));
		await t.group.via.api.store(80, { mode: StoreMode.Subtract });
		await t.group.expect(80).empty();
		await t.expect.selection(group(80));
	},
);

scenario(
	"BENCH-PRESET-001",
	"Preset families record and recall through pool, keypad, API, and OSC",
	async (t) => {
		await t.show.use(Show.DefaultStage);
		await t.app.open();
		await t.app.expect.ready();
		await openPools(t);
		await t.selection.fixtures.via.api.items(101, 102);

		await t.encoder.intensity.dimmer.via.api.set(70);
		await t.encoder.color.red.via.api.set(40);
		await t.encoder.position.pan.via.api.set(25);
		await t.encoder.beam.gobo1.via.api.set(30);

		await t.preset.via.api.store(PresetFamily.Mixed, 1, {
			mode: "overwrite",
		});
		await t.preset.via.pool.store(PresetFamily.Intensity, 1, {
			mode: "overwrite",
		});
		await t.preset.via.keypad.store(PresetFamily.Color, 1, {
			mode: "overwrite",
		});

		await t.hardware.connect();
		try {
			await t.preset.via.api.store(PresetFamily.Mixed, 2, {
				mode: "overwrite",
			});
			await t.preset.expect(PresetFamily.Mixed, 2).present();
			await t.preset.via.osc.recall(PresetFamily.Mixed, 2);
		} finally {
			await t.hardware.disconnect();
		}

		await t.preset.via.api.store(PresetFamily.Position, 1, {
			mode: "overwrite",
		});
		await t.preset.via.api.store(PresetFamily.Beam, 1, {
			mode: "overwrite",
		});
		for (const family of Object.values(PresetFamily)) {
			await t.preset.expect(family, 1).present();
			await t.preset.expect(family, 1).metadata({ family, number: 1 });
		}

		await t.preset.via.pool.edit(PresetFamily.Mixed, 1, {
			title: "Stage look",
			color: "#1bd6ec",
			icon: "★",
		});
		await t.preset.expect(PresetFamily.Mixed, 1).button({
			title: "Stage look",
			color: "#1bd6ec",
			icon: "★",
		});

		await t.encoder.clear();
		await t.selection.fixtures.via.api.item(102);
		await t.preset.via.pool.recall(PresetFamily.Mixed, 1);
		await t.expectFixtureDMX({ fixture: 102 }, { Intensity: 179 });
		await t.expectFixtureDMX({ fixture: 101 }, { Intensity: 0 });

		await t.preset.via.keypad.delete(PresetFamily.Color, 1);
		await t.preset.expect(PresetFamily.Color, 1).absent();

		await t.preset.recall(PresetFamily.Intensity, 1);
		expect(t.preset.routeReports.at(-1)).toMatchObject({
			action: "recall",
			family: PresetFamily.Intensity,
			candidates: ["api", "keypad", "pool"],
			selected: expect.stringMatching(/^(api|keypad|pool)$/),
		});
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
