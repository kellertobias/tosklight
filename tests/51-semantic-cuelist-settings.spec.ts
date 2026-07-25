// @bench-semantic-world

import { scenario } from "./bench/core/scenario";
import { PlaybackButton } from "./bench/playbacks/playbackScenario";
import { Show } from "./bench/show/showScenario";

scenario(
	"CUE-012",
	"Cuelist Settings persist Sequence and Chaser runtime policy",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();

		await t.selection.fixtures.via.api.item(1);
		await t.encoder.intensity.dimmer.via.api.set(25);
		const playback = await t.record.playback(1);
		await t.encoder.clear();
		await t.encoder.intensity.dimmer.via.api.set(50);
		await t.record.cue({ playback, cue: 2 });
		await t.encoder.clear();
		await t.playback.configure(playback, {
			name: "CUE-012 Settings",
			buttons: [PlaybackButton.GoBack, PlaybackButton.Go, PlaybackButton.Off],
		});

		const editor = await t.cue.openEditor(playback);
		const settings = await editor.openSettings();
		await settings.expectDefaults();
		await settings.configure({
			mode: "Chaser",
			priority: 42,
			intensityPriority: "LTP",
			wrap: "Reset",
			restart: "Continue Current Cue",
			forceCueTiming: true,
			disableCueTiming: true,
			speedMultiplier: 2,
			chaserXfade: 50,
		});
		await settings.save();

		await t.cue.expectList(playback).configuration({
			mode: "chaser",
			priority: 42,
			intensity_priority_mode: "ltp",
			wrap_mode: "reset",
			restart_mode: "continue_current_cue",
			force_cue_timing: true,
			disable_cue_timing: true,
			speed_group: "A",
			speed_multiplier: 2,
			chaser_xfade_percent: 50,
		});
	},
);
