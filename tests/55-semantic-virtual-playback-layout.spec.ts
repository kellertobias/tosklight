// @bench-semantic-world

import { scenario } from "./bench/core/scenario";
import { PlaybackButton } from "./bench/playbacks/playbackScenario";
import { Show } from "./bench/show/showScenario";
import { PaneType } from "./bench/window-system/paneTypes";

scenario(
	"PRELOAD-003",
	"Virtual Playbacks use a persisted pane-native 2×2 grid and real GO/TOGGLE playbacks",
	async (t) => {
		await t.show.use(Show.TwelveDimmers);
		await t.app.open();
		await t.app.expect.ready();
		await t.timing.programmerFade.via.api.set("0s");
		await t.timing.cueFade.set("0s");

		const recordSource = async (
			slot: number,
			fixture: number,
			name: string,
		) => {
			await t.selection.clear();
			await t.selection.fixtures.via.api.item(fixture);
			await t.encoder.intensity.dimmer.via.api.set(20);
			const playback = await t.record.playback(slot);
			await t.encoder.clear();
			await t.encoder.intensity.dimmer.via.api.set(80);
			await t.record.cue({ playback, cue: 2 });
			await t.encoder.clear();
			await t.playback.configure(playback, {
				name,
				buttonCount: 1,
				hasFader: false,
				buttons: [
					PlaybackButton.Go,
					PlaybackButton.Empty,
					PlaybackButton.Empty,
				],
			});
			await t.playback.via.api.off(playback);
			return playback;
		};

		await recordSource(5, 3, "Virtual Source A");
		await recordSource(6, 4, "Virtual Source B");

		const desktop = t.desktop.configure("Virtual Playback Desktop");
		const pane = desktop.addPane(
			PaneType.VirtualPlaybacks,
			{
				slug: "virtual-playbacks",
				column: 1,
				row: 1,
				width: 12,
				height: 10,
			},
			{ rows: 2, columns: 2 },
		);
		await desktop.apply();
		await t.virtualPlayback.expect.cells(pane, 4);

		const firstPlayback = await t.virtualPlayback.assignSource(
			pane,
			"Virtual Source A",
			1,
		);
		const secondPlayback = await t.virtualPlayback.assignSource(
			pane,
			"Virtual Source B",
			2,
		);
		await t.virtualPlayback.configureTopButton(pane, 2, PlaybackButton.Toggle);

		await t.virtualPlayback.activate(pane, 1);
		await t.virtualPlayback.activate(pane, 2);
		await t.playback.expect(firstPlayback).runtime({
			enabled: true,
			current_cue_number: 1,
		});
		await t.playback.expect(secondPlayback).runtime({
			enabled: true,
			current_cue_number: 1,
		});

		await t.virtualPlayback.reload(pane);
		await t.virtualPlayback.expect.cells(pane, 4);
		await t.virtualPlayback.expect.button(pane, 1, "GO");
		await t.virtualPlayback.expect.button(pane, 2, "TOGGLE");
		await t.playback.expect(firstPlayback).configuration({
			button_count: 1,
			has_fader: false,
			buttons: ["go", "none", "none"],
		});
		await t.playback.expect(secondPlayback).configuration({
			button_count: 1,
			has_fader: false,
			buttons: ["toggle", "none", "none"],
		});
	},
);
