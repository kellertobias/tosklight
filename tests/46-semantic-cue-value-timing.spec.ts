// @bench-semantic-world

import { fixtureRange } from "./bench/command-selection/selectionContract";
import { scenario } from "./bench/core/scenario";
import { Show } from "./bench/show/showScenario";

scenario(
	"CUE-004",
	"per-value timing overrides Cue fallback and Force Cue Timing is reversible",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();
		await t.timing.programmerFade.via.api.set("0s");

		await t.command.execute("GROUP 1 AT 0");
		const playback = await t.record.playback(1);
		await t.encoder.clear();

		await t.timing.programmerFade.via.api.set("9s");
		await t.command.execute("GROUP 1 AT 50 TIME 2");
		await t.record.cue({ playback, cue: 2, timing: { fade: "3" } });
		await t.encoder.clear();
		await t.cue.expect(playback, 2).metadata({
			fade_millis: 3_000,
			delay_millis: 0,
		});
		await t.cue
			.expect(playback, 2)
			.groupValueTiming(1, "intensity", { fade: "2s" });

		await t.playback.via.api.off(playback);
		await t.playback.go(playback);
		await t.playback.go(playback);
		await t.clock.advanceBy("1999ms");
		await t.expectFixtureDMX(fixtureRange(1, 12), { Intensity: 127 });
		await t.clock.advanceBy("1ms");
		await t.expectFixtureDMX(fixtureRange(1, 12), { Intensity: 128 });
	},
);
