// @bench-semantic-world

import {
	dereferencedGroup,
	fixture,
	group,
	groupRange,
} from "./bench/command-selection/selectionContract";
import { expect } from "./bench/core/fixtures";
import { scenario } from "./bench/core/scenario";
import { StoreMode } from "./bench/groups-presets/groupScenario";
import { PresetFamily } from "./bench/groups-presets/presetScenario";
import { fixture as dmxFixture } from "./bench/output/fixtureDmx";
import { Show } from "./bench/show/showScenario";
import { PaneType } from "./bench/window-system/paneTypes";

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

scenario(
	"BENCH-ENCODER-001",
	"normalized Dimmer absolute and relative API intents retain distinct semantics",
	async (t) => {
		await t.show.use(Show.DefaultStage);
		await t.app.open();
		await t.app.expect.ready();
		await t.show.expect.active(Show.DefaultStage);
		await t.selection.fixtures.via.api.item(1);
		await t.expect.selection(fixture(1));

		await t.encoder.intensity.dimmer.via.api.set(50);
		await t.clock.advanceBy("3s");
		await t.encoder.intensity.dimmer.via.api.add(3);
		await t.clock.advanceBy("3s");
		await t.encoder.intensity.dimmer.via.api.subtract(2);
		await t.clock.advanceBy("3s");
		await t.expectFixtureDMX(dmxFixture(1), { Intensity: 130 });
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
		});
		await t.preset.expect(PresetFamily.Mixed, 1).button({
			title: "Stage look",
		});

		await t.encoder.clear();
		await t.selection.clear();
		await t.preset.expectVisibleSelectionCount(0);
		await t.preset.tapEmptyPoolSlot(PresetFamily.Mixed, 9);
		await t.expect.selection();

		await t.preset.via.pool.recall(PresetFamily.Mixed, 1);
		await waitForSelectionCount(t, 2);
		await expectPresetTargetSelection(t);
		await t.preset.expectVisibleSelection(101, 102);
		await t.expectFixtureDMX({ fixture: 101 }, { Intensity: 0 });
		await t.expectFixtureDMX({ fixture: 102 }, { Intensity: 0 });

		await t.preset.via.pool.recall(PresetFamily.Mixed, 1);
		await t.expectFixtureDMX({ fixture: 101 }, { Intensity: 179 });
		await t.expectFixtureDMX({ fixture: 102 }, { Intensity: 179 });

		await t.preset.via.keypad.recall(PresetFamily.Position, 1);
		await expectPresetTargetSelection(t);

		await t.encoder.clear();
		await t.selection.clear();
		await t.preset.expectVisibleSelectionCount(0);
		await t.preset.via.api.recall(PresetFamily.Color, 1);
		await waitForSelectionCount(t, 2);
		await expectPresetTargetSelection(t);
		await t.preset.via.api.recall(PresetFamily.Color, 1);

		await t.hardware.connect();
		try {
			await t.preset.via.osc.recall(PresetFamily.Mixed, 2);
			await expectPresetTargetSelection(t);
			await t.expectFixtureDMX({ fixture: 101 }, { Intensity: 179 });
			await t.expectFixtureDMX({ fixture: 102 }, { Intensity: 179 });
			await t.highlight.via.osc.toggle();
			await expectPresetTargetSelection(t);
			await t.highlight.via.osc.toggle();
		} finally {
			await t.hardware.disconnect();
		}
	},
);

async function openPools(t: Parameters<Parameters<typeof scenario>[2]>[0]) {
	const desktop = t.desktop.configure("Groups and Presets");
	desktop.addPane(PaneType.Presets, {
		slug: "presets",
		column: 1,
		row: 1,
		width: 12,
		height: 18,
	});
	desktop.addPane(PaneType.Fixtures, {
		slug: "fixtures",
		column: 13,
		row: 1,
		width: 6,
		height: 18,
	});
	desktop.addPane(PaneType.Stage, {
		slug: "stage",
		column: 19,
		row: 1,
		width: 6,
		height: 18,
	});
	await desktop.apply();
}

async function waitForSelectionCount(
	t: Parameters<Parameters<typeof scenario>[2]>[0],
	count: number,
) {
	await expect
		.poll(async () => (await t.selection.observe()).selected.length)
		.toBe(count);
}

async function expectPresetTargetSelection(
	t: Parameters<Parameters<typeof scenario>[2]>[0],
) {
	const selection = await t.selection.observe();
	expect(selection.targets.map((target) => target.number)).toEqual([102, 101]);
	expect(selection.expression).toEqual({ type: "static" });
}
