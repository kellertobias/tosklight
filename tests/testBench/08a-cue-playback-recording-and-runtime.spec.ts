import { expect } from "../bench/core/fixtures";
import {
	PlaybackButton,
	PlaybackFader,
} from "../bench/playbacks/playbackScenario";
import { scenario } from "../bench/core/scenario";
import { Show } from "../bench/show/showScenario";

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
