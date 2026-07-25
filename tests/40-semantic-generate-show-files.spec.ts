// @bench-semantic-world

import {
	StoreMode,
} from "./bench/groups-presets/groupScenario";
import { PaneType } from "./bench/window-system/paneTypes";
import { scenario } from "./bench/core/scenario";
import { fixture } from "./bench/command-selection/selectionContract";
import { Show } from "./bench/show/showScenario";

scenario(
	"SHOW-000",
	"Save As produces independent reusable show files",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();
		await openShowPools(t);

		await t.selection.fixtures.via.fixtureSheet.item(1);
		await t.expect.selection(fixture(1));
		await t.group.via.pool.store(4, { mode: StoreMode.Overwrite });
		await t.group.via.pool.edit(4, {
			name: "Copy Center Spot",
			color: "#1bd6ec",
			icon: "★",
		});
		await t.group.expect(4).fixtures(1);
		await t.group.expect(4).metadata({
			name: "Copy Center Spot",
			color: "#1bd6ec",
			icon: "★",
		});

		const compactCopy = await t.show.via.ui.saveAs(
			`show-000-compact-copy-${crypto.randomUUID()}`,
		);
		const compactRevision = await t.show.via.ui.saveRevision(
			"SHOW-000 compact UI mutation",
		);
		const compactRevisionCopy = await t.show.via.ui.loadRevision(
			compactCopy,
			compactRevision,
		);
		await t.show.expect.active(compactRevisionCopy);
		await t.group.expect(4).fixtures(1);
		await t.group.expect(4).metadata({
			name: "Copy Center Spot",
			color: "#1bd6ec",
			icon: "★",
		});

		await t.show.resetWorkingCopy();
		await t.show.expect.active(Show.CompactRig);
		await t.group.expect(4).empty();
		await t.group.expect(4).metadata({
			name: "Center Spot",
			color: null,
			icon: null,
		});

		await t.show.use(Show.DefaultStage);
		await t.app.open();
		await t.app.expect.ready();
		await t.selection.fixtures.via.fixtureSheet.item(1);
		await t.expect.selection(fixture(1));
		await t.group.via.keypad.store(900, { mode: StoreMode.Overwrite });
		await t.group.expect(900).fixtures(1);
		await t.show.via.ui.saveAs(
			`show-000-default-copy-${crypto.randomUUID()}`,
		);
		await t.show.via.ui.saveRevision("SHOW-000 default UI mutation");

		await t.show.resetWorkingCopy();
		await t.show.expect.active(Show.DefaultStage);
		await t.group.expect(900).absent();
	},
);

async function openShowPools(
	t: Parameters<Parameters<typeof scenario>[2]>[0],
) {
	const desktop = t.desktop.configure("Generated show workflow");
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
