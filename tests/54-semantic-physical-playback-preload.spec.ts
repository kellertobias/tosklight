// @bench-semantic-world

import { scenario } from "./bench/core/scenario";
import { PlaybackButton } from "./bench/playbacks/playbackScenario";
import { Show } from "./bench/show/showScenario";

scenario(
	"PRELOAD-002",
	"physical Playback Preload preserves all seven ordered action verbs",
	async (t) => {
		await t.show.use(Show.TwelveDimmers);
		await t.app.open();
		await t.app.expect.ready();
		await t.timing.programmerFade.via.api.set("0s");
		await t.timing.cueFade.set("0s");

		const recordPlayback = async (
			slot: number,
			fixture: number,
			button: PlaybackButton,
		) => {
			await t.selection.clear();
			await t.selection.fixtures.via.api.item(fixture);
			await t.encoder.intensity.dimmer.via.api.set(30);
			const playback = await t.record.playback(slot);
			await t.encoder.clear();
			await t.encoder.intensity.dimmer.via.api.set(70);
			await t.record.cue({ playback, cue: 2 });
			await t.encoder.clear();
			await t.playback.configure(playback, {
				name: `Physical ${button}`,
				buttons: [button, PlaybackButton.Empty, PlaybackButton.Empty],
			});
			await t.playback.via.api.off(playback);
			return playback;
		};

		const togglePlayback = await recordPlayback(1, 1, PlaybackButton.Toggle);
		const goPlayback = await recordPlayback(2, 2, PlaybackButton.Go);
		const backPlayback = await recordPlayback(3, 3, PlaybackButton.GoBack);
		const offPlayback = await recordPlayback(4, 4, PlaybackButton.Off);
		const onPlayback = await recordPlayback(5, 5, PlaybackButton.On);
		const tempPlayback = await recordPlayback(6, 6, PlaybackButton.Temp);

		await t.playback.via.api.go(backPlayback);
		await t.playback.via.api.go(backPlayback);
		await t.playback.via.api.go(offPlayback);
		await t.preload.configure({
			programmer: false,
			physicalPlaybacks: true,
			virtualPlaybacks: false,
			programmerFade: 2_000,
			cueFade: 0,
		});
		await t.playback.open();
		await t.preload.start();

		await t.playback.toggle(togglePlayback);
		await t.playback.go(goPlayback);
		await t.playback.goBack(backPlayback);
		await t.playback.off(offPlayback);
		await t.playback.on(onPlayback);
		await t.playback.via.ui.temp(tempPlayback);
		await t.playback.via.ui.temp(tempPlayback);
		await t.preload.expect.pendingPlaybackActions([
			"toggle",
			"go",
			"go-minus",
			"off",
			"on",
			"temp-on",
			"temp-off",
		]);

		await t.preload.commit();
		await t.clock.advanceBy("2s");
		await t.playback.expect(togglePlayback).runtime({
			enabled: true,
			current_cue_number: 1,
		});
		await t.playback.expect(goPlayback).runtime({
			enabled: true,
			current_cue_number: 1,
		});
		await t.playback.expect(backPlayback).runtime({
			enabled: true,
			current_cue_number: 1,
		});
		await t.playback.expect(onPlayback).runtime({ enabled: true });

		await t.preload.via.ui.release();
		await t.playback.expect(togglePlayback).runtime({
			enabled: true,
			current_cue_number: 1,
		});
		await t.playback.expect(onPlayback).runtime({ enabled: true });
	},
);
