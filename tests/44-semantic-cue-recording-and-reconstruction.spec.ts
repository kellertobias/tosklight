// @bench-semantic-world

import { CueRecordMode } from "../apps/control-ui/e2e/bench/cuePlaybackScenario";
import { StoreMode as GroupStoreMode } from "../apps/control-ui/e2e/bench/groupScenario";
import { scenario } from "../apps/control-ui/e2e/bench/scenario";
import { fixtureRange } from "../apps/control-ui/e2e/bench/selectionContract";
import { Show } from "../apps/control-ui/e2e/bench/showScenario";

scenario(
	"CUE-008",
	"blind Preload records the same Cue without activating playback or output",
	async (t) => {
		await t.show.use(Show.TwelveDimmers);
		await t.app.open();
		await t.app.expect.ready();
		await installCompactGroups(t);

		await t.preload.start();
		await t.command.execute("GROUP 1 AT 100");
		const playback = await t.record.playback(1);
		await t.playback.expect(playback).present();
		await t.clock.advanceBy("0ms");
		await t.expectFixtureDMX(fixtureRange(1, 12), { Intensity: 0 });

		await t.preload.release();
		await t.playback.go(playback);
		await t.clock.advanceBy("3s");
		await t.expectFixtureDMX(fixtureRange(1, 4), { Intensity: 255 });
		await t.expectFixtureDMX(fixtureRange(5, 12), { Intensity: 0 });
	},
);

scenario(
	"CUE-001",
	"Record targets playbacks while decimal insertion and Record operations preserve tracking",
	async (t) => {
		await t.show.use(Show.TwelveDimmers);
		await t.app.open();
		await t.app.expect.ready();
		await installCompactGroups(t);

		await t.command.execute("GROUP 1 AT 100");
		const playback = await t.record.playback(1);
		await t.encoder.clear();
		await t.command.execute("GROUP 3 AT 100");
		await t.record.cue({ playback, cue: 1.5 });
		await t.encoder.clear();
		await t.command.execute("GROUP 2 AT 80");
		await t.record.cue({ playback, cue: 2 });
		await t.encoder.clear();
		await t.command.execute("GROUP 2 AT 50");
		await t.record.cue({
			playback,
			cue: 2,
			mode: CueRecordMode.Merge,
		});
		await t.record.cue({
			playback,
			cue: 2,
			mode: CueRecordMode.Subtract,
		});
		await t.encoder.clear();

		await t.cue.expect(playback, 1).present();
		await t.cue.expect(playback, 1.5).present();
		await t.cue.expect(playback, 2).present();
		await t.playback.via.api.off(playback);

		await t.playback.go(playback);
		await t.clock.advanceBy("3s");
		await t.expectFixtureDMX(fixtureRange(1, 4), { Intensity: 255 });
		await t.expectFixtureDMX(fixtureRange(5, 12), { Intensity: 0 });

		await t.playback.go(playback);
		await t.clock.advanceBy("3s");
		await t.expectFixtureDMX(fixtureRange(1, 4), { Intensity: 255 });
		await t.expectFixtureDMX(fixtureRange(5, 8), { Intensity: 0 });
		await t.expectFixtureDMX(fixtureRange(9, 12), { Intensity: 255 });

		await t.playback.go(playback);
		await t.clock.advanceBy("3s");
		await t.expectFixtureDMX(fixtureRange(1, 4), { Intensity: 255 });
		await t.expectFixtureDMX(fixtureRange(5, 8), { Intensity: 0 });
		await t.expectFixtureDMX(fixtureRange(9, 12), { Intensity: 255 });
	},
);

scenario(
	"CUE-002",
	"Cue-only restoration reconstructs identically for sequential GO and direct jumps",
	async (t) => {
		await t.show.use(Show.TwelveDimmers);
		await t.app.open();
		await t.app.expect.ready();
		await installCompactGroups(t);

		await t.command.execute("GROUP 1 AT 30");
		const playback = await t.record.playback(1);
		await t.encoder.clear();
		await t.command.execute("GROUP 1 AT 80");
		await t.record.cueOnly(true);
		await t.record.append(playback);
		await t.encoder.clear();
		await t.command.execute("GROUP 2 AT 60");
		await t.record.cueOnly(false);
		await t.record.append(playback);
		await t.encoder.clear();
		await t.playback.via.api.off(playback);

		await t.playback.go(playback);
		await t.clock.advanceBy("3s");
		await t.expectFixtureDMX(fixtureRange(1, 4), { Intensity: 77 });
		await t.expectFixtureDMX(fixtureRange(5, 8), { Intensity: 0 });

		await t.playback.go(playback);
		await t.clock.advanceBy("3s");
		await t.expectFixtureDMX(fixtureRange(1, 4), { Intensity: 204 });
		await t.expectFixtureDMX(fixtureRange(5, 8), { Intensity: 0 });

		await t.playback.go(playback);
		await t.clock.advanceBy("3s");
		await t.expectFixtureDMX(fixtureRange(1, 4), { Intensity: 77 });
		await t.expectFixtureDMX(fixtureRange(5, 8), { Intensity: 153 });

		for (const cue of [1, 2, 3]) {
			await t.playback.via.api.off(playback);
			await t.cue.goto(playback, cue);
			await t.clock.advanceBy("3s");
			await t.expectFixtureDMX(fixtureRange(1, 4), {
				Intensity: cue === 2 ? 204 : 77,
			});
			await t.expectFixtureDMX(fixtureRange(5, 8), {
				Intensity: cue === 3 ? 153 : 0,
			});
		}
	},
);

async function installCompactGroups(
	t: Parameters<Parameters<typeof scenario>[2]>[0],
) {
	for (const [group, fixtures] of [
		[1, [1, 2, 3, 4]],
		[2, [5, 6, 7, 8]],
		[3, [9, 10, 11, 12]],
	] as const) {
		await t.selection.fixtures.via.api.items(...fixtures);
		await t.group.via.api.store(group, { mode: GroupStoreMode.Overwrite });
	}
	await t.selection.clear();
}
