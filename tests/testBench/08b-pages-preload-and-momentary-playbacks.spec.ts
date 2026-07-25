import { expect } from "../bench/core/fixtures";
import {
	explicitPagePlayback,
	PlaybackButton,
} from "../bench/playbacks/playbackScenario";
import { scenario } from "../bench/core/scenario";
import { Show } from "../bench/show/showScenario";

scenario(
	"BENCH-PRELOAD-MOMENTARY-001",
	"Preload and OSC momentary Playback controls preserve their phase boundaries",
	async (t) => {
		await t.show.use(Show.DefaultStage);
		await t.app.open();
		await t.app.expect.ready();
		await t.timing.programmerFade.via.api.set("0s");
		await t.selection.fixtures.via.api.item(101);
		await t.encoder.intensity.dimmer.via.api.set(60);
		const playback = await t.record.playback(1);
		await t.encoder.clear();
		await t.playback.configure(playback, {
			buttons: [
				PlaybackButton.Flash,
				PlaybackButton.Temp,
				PlaybackButton.Swap,
			],
		});
		await t.playback.open();

		await t.preload.via.api.start();
		await t.preload.setFixtureValue({
			fixture: 102,
			attribute: "intensity",
			value: { kind: "normalized", value: 0.8 },
		});
		await t.expectFixtureDMX({ fixture: 102 }, { Intensity: 0 });
		await t.preload.via.api.commit();
		await t.clock.advanceBy("0ms");
		await t.expectFixtureDMX({ fixture: 102 }, { Intensity: 204 });
		await t.preload.release();

		await t.playback.via.ui.flash(playback).hold(async () => {
			await t.playback.expect(playback).runtime({
				flash: true,
				temporary_active: true,
			});
		});
		await t.playback.expect(playback).runtime({
			flash: false,
			temporary_active: false,
		});

		await t.hardware.connect();
		try {
			const target = explicitPagePlayback(1, 1);
			await t.playback.via.osc.flash(target).press();
			await t.playback.expect(playback).runtime({
				flash: true,
				temporary_active: true,
			});
			await t.playback.via.osc.flash(target).release();
			await t.playback.expect(playback).runtime({
				flash: false,
				temporary_active: false,
			});

			await t.playback.via.osc.swap(target).hold(async () => {
				await t.playback.expect(playback).runtime({
					swap_active: true,
					temporary_active: true,
				});
			});
			await t.playback.expect(playback).runtime({
				swap_active: false,
				temporary_active: false,
			});
		} finally {
			await t.hardware.disconnect();
		}

		expect(await t.page.current()).toBe(1);
	},
);
