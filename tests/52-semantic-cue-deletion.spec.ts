// @bench-semantic-world

import { fixture } from "../apps/control-ui/e2e/bench/command-selection/selectionContract";
import { scenario } from "../apps/control-ui/e2e/bench/core/scenario";
import { PlaybackButton } from "../apps/control-ui/e2e/bench/playbacks/playbackScenario";
import { Show } from "../apps/control-ui/e2e/bench/show/showScenario";

scenario(
	"CUE-013",
	"deleting the active Cue holds output and anchors GO around the surviving Cues",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();
		await t.timing.programmerFade.via.api.set("0s");
		await t.timing.cueFade.set("0s");

		await t.selection.fixtures.via.api.item(1);
		await t.encoder.intensity.dimmer.via.api.set(25);
		const playback = await t.record.playback(1);
		await t.encoder.clear();
		await t.encoder.intensity.dimmer.via.api.set(50);
		await t.record.cue({ playback, cue: 2 });
		await t.encoder.clear();
		await t.encoder.intensity.dimmer.via.api.set(75);
		await t.record.cue({ playback, cue: 3 });
		await t.encoder.clear();
		await t.playback.configure(playback, {
			name: "Delete Active Sequence",
			buttons: [PlaybackButton.GoBack, PlaybackButton.Go, PlaybackButton.Off],
		});

		await t.playback.via.api.off(playback);
		await t.playback.go(playback);
		await t.playback.go(playback);
		await t.clock.advanceStep();
		await t.expectFixtureDMX(fixture(1), { Intensity: 128 });

		await t.cue.delete(playback, 2);
		await t.cue.expect(playback, 2).absent();
		await t.playback.expect(playback).runtime({
			current_cue_number: 2,
			deleted_cue_hold: {
				deleted_number: 2,
				previous_number: 1,
				next_number: 3,
			},
		});
		await t.expectFixtureDMX(fixture(1), { Intensity: 128 });

		await t.playback.go(playback);
		await t.clock.advanceStep();
		await t.playback.expect(playback).runtime({ current_cue_number: 3 });
		await t.expectFixtureDMX(fixture(1), { Intensity: 191 });

		await t.playback.goBack(playback);
		await t.clock.advanceStep();
		await t.playback.expect(playback).runtime({ current_cue_number: 1 });
		await t.expectFixtureDMX(fixture(1), { Intensity: 64 });
	},
);
