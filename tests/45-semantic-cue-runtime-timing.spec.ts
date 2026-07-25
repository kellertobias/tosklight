// @bench-semantic-world

import { PlaybackButton } from "../apps/control-ui/e2e/bench/playbacks/playbackScenario";
import { scenario } from "../apps/control-ui/e2e/bench/core/scenario";
import { fixture } from "../apps/control-ui/e2e/bench/command-selection/selectionContract";
import { Show } from "../apps/control-ui/e2e/bench/show/showScenario";

scenario(
	"CUE-003",
	"GO, pause, resume, back, and release use exact application-time boundaries",
	async (t) => {
		await t.show.use(Show.TwelveDimmers);
		await t.app.open();
		await t.app.expect.ready();
		await t.timing.programmerFade.via.api.set("0s");
		await t.timing.cueFade.set("0s");

		await t.selection.fixtures.via.api.item(1);
		await t.encoder.intensity.dimmer.via.api.set(0);
		const playback = await t.record.playback(1);
		await t.encoder.clear();
		await t.selection.fixtures.via.api.item(1);
		await t.encoder.intensity.dimmer.via.api.set(100);
		await t.record.cue({ playback, cue: 2, timing: { fade: "4" } });
		await t.encoder.clear();
		await t.playback.configure(playback, {
			buttons: [PlaybackButton.GoBack, PlaybackButton.Go, PlaybackButton.Pause],
		});
		await t.playback.via.api.off(playback);

		await t.playback.go(playback);
		await t.playback.go(playback);
		await t.clock.advanceBy("1s");
		await t.expectFixtureDMX(fixture(1), { Intensity: 64 });
		await t.playback.pause(playback);
		await t.clock.advanceBy("10s");
		await t.expectFixtureDMX(fixture(1), { Intensity: 64 });
		await t.playback.pause(playback);
		await t.clock.advanceBy("3s");
		await t.expectFixtureDMX(fixture(1), { Intensity: 255 });

		await t.playback.goBack(playback);
		await t.clock.advanceBy("0ms");
		await t.expectFixtureDMX(fixture(1), { Intensity: 0 });
		await t.playback.release(playback);
		await t.clock.advanceBy("0ms");
		await t.expectFixtureDMX(fixture(1), { Intensity: 0 });
	},
);
