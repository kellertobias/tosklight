// @bench-semantic-world

import { scenario } from "./bench/core/scenario";
import { PlaybackButton } from "./bench/playbacks/playbackScenario";
import { Show } from "./bench/show/showScenario";

scenario(
	"CUE-011",
	"Cuelist View edits preserve Cue identity, runtime selection, and persisted values",
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
			name: "CUE-011 Sequence",
			buttons: [PlaybackButton.GoBack, PlaybackButton.Go, PlaybackButton.Off],
		});
		await t.playback.expect(playback).runtime({ current_cue_number: 1 });

		let editor = await t.cue.openEditor(playback);
		await editor.expect.structure();
		await editor.select(2);
		await editor.edit(2, {
			name: "Center transition",
			fade: "2.5",
			delay: "1.25",
			trigger: "TIME",
			triggerTime: "4",
		});
		await t.cue.expect(playback, 2).metadata({
			name: "Center transition",
			fade_millis: 2_500,
			delay_millis: 1_250,
		});
		await t.cue
			.expect(playback, 2)
			.trigger({ type: "wait", delay_millis: 4_000 });

		await editor.select(3);
		await editor.select(2);
		await editor.expect.selected(2, {
			name: "Center transition",
			fade: "2.5",
			delay: "1.25",
			triggerTime: "4",
		});
		await editor.inspectSettings();
		await editor.expect.selected(2);
		await t.playback.expect(playback).runtime({ current_cue_number: 1 });

		await editor.reject(2, { fade: "-1" });
		await t.cue.expect(playback, 2).metadata({
			name: "Center transition",
			fade_millis: 2_500,
			delay_millis: 1_250,
		});
		await t.playback.expect(playback).runtime({ current_cue_number: 1 });

		editor = await t.cue.reopenEditor(playback);
		await editor.select(2);
		await editor.expect.selected(2, {
			name: "Center transition",
			fade: "2.5",
			delay: "1.25",
			triggerTime: "4",
		});
	},
);
