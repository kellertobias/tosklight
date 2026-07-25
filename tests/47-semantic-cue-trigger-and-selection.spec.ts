// @bench-semantic-world

import { fixtureRange } from "../apps/control-ui/e2e/bench/command-selection/selectionContract";
import { scenario } from "../apps/control-ui/e2e/bench/core/scenario";
import { StoreMode } from "../apps/control-ui/e2e/bench/groups-presets/groupScenario";
import { Show } from "../apps/control-ui/e2e/bench/show/showScenario";

scenario(
	"CUE-005",
	"GO, FOLLOW, and TIME measure from the preceding Cue's latest value endpoint",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();
		await t.timing.programmerFade.via.api.set("0s");

		await t.command.execute("GROUP 1 AT 50");
		const playback = await t.record.playback(1);
		await t.command.type("RECORD SET 1 CUE 2 TIME TIME 0");
		await t.command.expect("RECORD SET 1 CUE 2 DELAY 0");
		await t.keypad.press(["ENT"]);
		await t.command.type("RECORD SET 1 CUE 3 TIME TIME 4");
		await t.command.expect("RECORD SET 1 CUE 3 DELAY 4");
		await t.keypad.press(["ENT"]);

		await t.cue.expect(playback, 1).trigger({ type: "manual" });
		await t.cue
			.expect(playback, 2)
			.trigger({ type: "follow", delay_millis: 0 });
		await t.cue
			.expect(playback, 3)
			.trigger({ type: "wait", delay_millis: 4_000 });
	},
);

scenario(
	"CUE-006",
	"explicit playback selection supplies the implicit Cuelist without following execution order",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();
		await t.timing.programmerFade.via.api.set("0s");

		await t.selection.fixtures.via.api.item(1);
		await t.encoder.intensity.dimmer.via.api.set(20);
		const first = await t.record.playback(1);
		await t.encoder.clear();
		await t.selection.fixtures.via.api.item(2);
		await t.encoder.intensity.dimmer.via.api.set(30);
		const second = await t.record.playback(2);
		await t.encoder.clear();

		await t.playback.select(second);
		await t.playback.expect(second).selected();
		await t.playback.go(first);
		await t.playback.expect(second).selected();

		await t.selection.fixtures.via.api.item(3);
		await t.encoder.intensity.dimmer.via.api.set(70);
		await t.command.execute("RECORD CUE 7");
		await t.cue.expect(second, 7).present();
		await t.cue.expect(first, 7).absent();
		await t.playback.expect(second).selected();
	},
);

scenario(
	"CUE-007",
	"explicit zeroes block a later inserted on Cue from tracking past Cue 4",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();
		await t.timing.programmerFade.via.api.set("0s");
		await t.timing.cueFade.set("0s");

		await t.selection.fixtures.via.api.items(1, 2, 3, 4);
		await t.group.via.api.store(1, { mode: StoreMode.Overwrite });
		await t.selection.fixtures.via.api.items(5, 6, 7, 8);
		await t.group.via.api.store(2, { mode: StoreMode.Overwrite });
		await t.selection.fixtures.via.api.items(9, 10, 11, 12);
		await t.group.via.api.store(3, { mode: StoreMode.Overwrite });
		await t.selection.clear();
		await t.command.execute("GROUP 1 AT 100");
		const playback = await t.record.playback(1);
		await t.encoder.clear();
		await t.command.execute("GROUP 1 AT 0");
		await t.record.cue({ playback, cue: 2 });
		await t.encoder.clear();
		await t.command.execute("GROUP 2 AT 100");
		await t.record.cue({ playback, cue: 3 });
		await t.encoder.clear();
		await t.command.execute("GROUP 1 AT 0");
		await t.record.cue({ playback, cue: 4 });
		await t.encoder.clear();
		await t.command.execute("GROUP 3 AT 100");
		await t.record.cue({ playback, cue: 5 });
		await t.encoder.clear();
		await t.command.execute("GROUP 1 AT 100");
		await t.record.cue({ playback, cue: 3.5 });
		await t.encoder.clear();

		await t.cue.expect(playback, 1).groupValue(1, "intensity", 1);
		await t.cue.expect(playback, 4).groupValue(1, "intensity", 0);
		await t.playback.release(playback);
		await t.playback.go(playback);
		await t.clock.advanceStep();
		await t.playback.expect(playback).runtime({ current_cue_number: 1 });
		await t.expectFixtureDMX(fixtureRange(1, 4), { Intensity: 255 });
		await t.expectFixtureDMX(fixtureRange(5, 12), { Intensity: 0 });
		await t.playback.go(playback);
		await t.clock.advanceStep();
		await t.expectFixtureDMX(fixtureRange(1, 12), { Intensity: 0 });
		await t.playback.go(playback);
		await t.clock.advanceStep();
		await t.expectFixtureDMX(fixtureRange(1, 4), { Intensity: 0 });
		await t.expectFixtureDMX(fixtureRange(5, 8), { Intensity: 255 });
		await t.expectFixtureDMX(fixtureRange(9, 12), { Intensity: 0 });
		await t.playback.go(playback);
		await t.clock.advanceStep();
		await t.expectFixtureDMX(fixtureRange(1, 8), { Intensity: 255 });
		await t.expectFixtureDMX(fixtureRange(9, 12), { Intensity: 0 });
		await t.playback.go(playback);
		await t.clock.advanceStep();
		await t.expectFixtureDMX(fixtureRange(1, 4), { Intensity: 0 });
		await t.expectFixtureDMX(fixtureRange(5, 8), { Intensity: 255 });
		await t.expectFixtureDMX(fixtureRange(9, 12), { Intensity: 0 });
		await t.playback.go(playback);
		await t.clock.advanceStep();
		await t.expectFixtureDMX(fixtureRange(1, 4), { Intensity: 0 });
		await t.expectFixtureDMX(fixtureRange(5, 12), { Intensity: 255 });
	},
);
