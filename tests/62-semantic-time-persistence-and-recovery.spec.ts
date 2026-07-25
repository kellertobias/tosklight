// @bench-semantic-world

import { expect } from "./bench/core/fixtures";
import { scenario } from "./bench/core/scenario";
import { StoreMode } from "./bench/groups-presets/groupScenario";
import { fixture, group } from "./bench/command-selection/selectionContract";
import {
	RestartMode,
	Show,
} from "./bench/show/showScenario";
import { PaneType } from "./bench/window-system/paneTypes";

scenario(
	"TIME-001",
	"zero ticks emit current state without advancing behavior time",
	async (t) => {
		await t.show.use(Show.TwelveDimmers);
		await t.app.open();
		await t.app.expect.ready();
		await openProgrammingPools(t);
		await t.timing.programmerFade.via.api.set("0ms");
		await t.selection.fixtures.via.ui.item(1);
		await t.encoder.intensity.dimmer.via.ui.set(50);
		await t.hardware.connect();
		try {
			const first = await t.clock.advanceStep();
			const second = await t.clock.advanceStep();
			expect(second.now).toBe(first.now);
			await t.expectFixtureDMX(fixture(1), { Intensity: 128 });
		} finally {
			await t.hardware.disconnect();
		}
	},
);

scenario(
	"TIME-002",
	"touch-set fixture values bypass Programmer Fade at every timing boundary",
	async (t) => {
		await t.show.use(Show.TwelveDimmers);
		await t.app.open();
		await t.app.expect.ready();
		await openProgrammingPools(t);
		await t.timing.programmerFade.via.api.set("3s");
		await t.selection.fixtures.via.ui.item(1);
		await t.encoder.intensity.dimmer.via.ui.set(0);
		await t.clock.advanceBy("3s");
		await t.encoder.intensity.dimmer.via.ui.set(100);
		await t.clock.at(
			[
				{ name: "start", at: "0ms" },
				{ name: "midpoint", at: "1.5s" },
				{ name: "complete", at: "3s" },
			],
			async () => {
				await t.expectFixtureDMX(fixture(1), {
					Intensity: 255,
				});
			},
		);
	},
);

scenario(
	"TIME-002",
	"touch-set Group values bypass Programmer Fade for every ordered member",
	async (t) => {
		await t.show.use(Show.TwelveDimmers);
		await t.app.open();
		await t.app.expect.ready();
		await openProgrammingPools(t);
		await t.timing.programmerFade.via.api.set("3s");
		await t.selection.groups.via.pool.item(3);
		await t.encoder.intensity.dimmer.via.ui.set(0);
		await t.clock.advanceBy("3s");
		await t.encoder.intensity.dimmer.via.ui.set(100);
		await t.clock.at(
			[
				{ name: "start", at: "0ms" },
				{ name: "complete", at: "3s" },
			],
			async () => {
				for (const number of [1, 2, 3, 4])
					await t.expectFixtureDMX(fixture(number), {
						Intensity: 255,
					});
			},
		);
	},
);

scenario(
	"SHOW-001",
	"operator programming and a named revision survive an abrupt restart",
	async (t) => {
		await t.show.use(Show.TwelveDimmers);
		await t.app.open();
		await t.app.expect.ready();
		await openProgrammingPools(t);
		await t.selection.fixtures.via.ui.items(5, 6);
		await t.group.via.pool.store(3, { mode: StoreMode.Merge });
		await t.selection.targets(group(3));
		await t.encoder.intensity.dimmer.via.ui.set(40);
		const revision = await t.show.saveRevision("SHOW-001 before restart");
		await t.show.restart(RestartMode.Abrupt);
		await t.app.expect.ready();
		await t.group.expect(3).fixtures(1, 2, 3, 4, 5, 6);
		await t.show.expect.revision({
			number: revision,
			name: "SHOW-001 before restart",
		});
	},
);

async function openProgrammingPools(
	t: Parameters<Parameters<typeof scenario>[2]>[0],
) {
	const desktop = t.desktop.configure("Time and recovery programming");
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

scenario(
	"SHOW-003",
	"a malformed active show remains intact while the operator recovers safely",
	async (t) => {
		await t.show.use(Show.TwelveDimmers);
		await t.app.open();
		await t.show.recovery.prepareMalformedActive();
		await t.show.expect.recoveryRequired();
		await t.show.loadCleanDefault();
		await t.show.expect.recovered();
	},
);

scenario(
	"SHOW-005",
	"named revisions load as durable, visibly independent copies",
	async (t) => {
		await t.show.use(Show.TwelveDimmers);
		await t.app.open();
		await t.app.expect.ready();
		const source = await t.show.saveAs(`SHOW-005 source ${crypto.randomUUID()}`);
		const revision = await t.show.saveRevision("SHOW-005 named revision");
		await t.selection.fixtures.via.api.items(5, 6);
		await t.group.via.api.store(3, { mode: StoreMode.Merge });
		const changed = await t.show.saveAs(`SHOW-005 changed ${crypto.randomUUID()}`);
		await t.show.expect.active(changed);
		const revisionCopy = await t.show.loadRevision(source, revision);
		await t.show.expect.active(revisionCopy);
		await t.group.expect(3).fixtures(1, 2, 3, 4);
		await t.show.save();
		await t.show.expect.active(revisionCopy);
	},
);
