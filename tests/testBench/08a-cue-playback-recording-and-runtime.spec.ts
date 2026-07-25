import { expect } from "../../apps/control-ui/e2e/bench/fixtures";
import {
	CueRecordMode,
} from "../../apps/control-ui/e2e/bench/cuePlaybackScenario";
import {
	PlaybackButton,
	PlaybackFader,
} from "../../apps/control-ui/e2e/bench/playbackScenario";
import { scenario } from "../../apps/control-ui/e2e/bench/scenario";
import { Show } from "../../apps/control-ui/e2e/bench/showScenario";

scenario(
	"BENCH-CUE-PLAYBACK-001",
	"visible recording and typed runtime actions preserve Cue timing and concrete Playback identity",
	async (t) => {
		await t.show.use(Show.DefaultStage);
		await t.app.open();
		await t.app.expect.ready();
		await t.timing.programmerFade.via.api.set("0s");
		await t.selection.fixtures.via.api.item(101);

		await t.encoder.intensity.dimmer.via.api.set(20);
		const playback = await t.record.via.ui.playback(1);
		await t.playback.expect(playback).present();
		await t.cue.expect(playback, 1).present();

		await t.encoder.clear();
		await t.encoder.intensity.dimmer.via.api.set(80);
		await t.record.via.api.cue({
			playback,
			cue: 2,
			mode: CueRecordMode.Overwrite,
			timing: { fade: "1" },
		});
		await t.cue.expect(playback, 2).metadata({ fade_millis: 1_000 });
		await t.encoder.clear();

		await t.playback.via.api.off(playback);
		await t.playback.via.ui.go(playback);
		await t.playback.expect(playback).runtime({
			current_cue_number: 1,
			enabled: true,
		});
		await t.expectFixtureDMX({ fixture: 101 }, { Intensity: 51 });

		await t.playback.via.api.go(playback);
		await t.clock.advanceBy("500ms");
		await t.expectFixtureDMX({ fixture: 101 }, { Intensity: 128 });
		await t.clock.advanceBy("500ms");
		await t.expectFixtureDMX({ fixture: 101 }, { Intensity: 204 });

		await t.playback.via.api.select(playback);
		await t.playback.expect(playback).selected();
	},
);

scenario(
	"BENCH-CUE-PLAYBACK-002",
	"Cue transfer and typed Playback configuration converge on normalized observations",
	async (t) => {
		await t.show.use(Show.DefaultStage);
		await t.app.open();
		await t.app.expect.ready();
		await t.timing.programmerFade.via.api.set("0s");
		await t.selection.fixtures.via.api.item(102);

		await t.encoder.intensity.dimmer.via.api.set(25);
		const playback = await t.record.playback(2);
		await t.encoder.clear();
		await t.encoder.intensity.dimmer.via.api.set(75);
		await t.record.cue({ playback, cue: 2 });

		await t.cue.via.api.copy(playback, 2, 3);
		await t.cue.expect(playback, 3).present();
		await t.cue.via.ui.move(playback, 3, 4);
		await t.cue.expect(playback, 3).absent();
		await t.cue.expect(playback, 4).present();
		await t.cue.via.api.delete(playback, 4);
		await t.cue.expect(playback, 4).absent();

		await t.playback.configure(playback, {
			name: "Typed sequence",
			color: "#1bd6ec",
			buttons: [
				PlaybackButton.Toggle,
				PlaybackButton.On,
				PlaybackButton.Off,
			],
			fader: PlaybackFader.Master,
		});
		await t.playback.expect(playback).configuration({
			name: "Typed sequence",
			color: "#1bd6ec",
			buttons: ["toggle", "on", "off"],
			fader: "master",
		});

		await t.playback.via.api.fader(playback, 40);
		await t.playback.expect(playback).runtime({ master: 0.4 });
		await t.playback.via.api.toggle(playback);
		await t.playback.expect(playback).runtime({ enabled: false });
		await t.playback.via.api.off(playback);
		await t.playback.expect(playback).runtime({ enabled: false });
		await t.playback.via.api.on(playback);
		await t.playback.expect(playback).runtime({ enabled: true });
		await t.playback.via.api.goBack(playback);
		await t.playback.expect(playback).runtime({ current_cue_number: 1 });

		expect(await t.playback.runtime(playback)).toMatchObject({
			playback_number: playback,
		});
		expect((await t.playback.runtime(playback))?.master).toBeCloseTo(1, 5);
	},
);
