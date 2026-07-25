import { expect } from "../../apps/control-ui/e2e/bench/fixtures";
import {
	currentPagePlayback,
	explicitPagePlayback,
	PlaybackButton,
} from "../../apps/control-ui/e2e/bench/playbackScenario";
import { scenario } from "../../apps/control-ui/e2e/bench/scenario";
import { Show } from "../../apps/control-ui/e2e/bench/showScenario";

scenario(
	"BENCH-PAGE-PLAYBACK-001",
	"current and explicit Page targets retain distinct Playback authority",
	async (t) => {
		await t.show.use(Show.DefaultStage);
		await t.app.open();
		await t.app.expect.ready();
		await t.timing.programmerFade.via.api.set("0s");

		await t.selection.fixtures.via.api.item(101);
		await t.encoder.intensity.dimmer.via.api.set(25);
		const first = await t.record.playback(1);
		await t.encoder.clear();
		await t.selection.fixtures.via.api.item(102);
		await t.encoder.intensity.dimmer.via.api.set(75);
		const second = await t.record.playback(2);
		await t.encoder.clear();

		await t.page.create(2);
		await t.page.rename(2, "Second");
		await t.page.map({ page: 2, slot: 1, playback: second });
		await t.page.expect(2).named("Second");

		await t.playback.via.api.off(first);
		await t.playback.via.api.off(second);
		await t.page.via.api.select(1);
		await t.playback.via.api.go(currentPagePlayback(1));
		await t.playback.expect(first).runtime({ enabled: true });
		await t.playback.expect(second).runtime({ enabled: false });

		await t.page.via.ui.select(2);
		await t.playback.via.api.go(currentPagePlayback(1));
		await t.playback.expect(second).runtime({ enabled: true });

		await t.playback.via.api.off(first);
		await t.playback.via.api.go(explicitPagePlayback(1, 1));
		await t.playback.expect(first).runtime({ enabled: true });
		await t.page.expect(2).selected();

		const screen = await t.screen.create({
			name: "Independent Playbacks",
			showPlaybacks: true,
			showPageControls: true,
			playbacks: {
				perRow: 1,
				rows: [{ first: 1, fader: true, buttons: 3 }],
				pageMode: "dedicated",
			},
		});
		await screen.page.select(1);
		await screen.page.expectSelected(1);
		await t.page.expect(2).selected();
	},
);

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
