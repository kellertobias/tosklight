// @bench-semantic-world

import { fixture } from "./bench/output/fixtureDmxContract";
import { scenario } from "./bench/core/scenario";
import { PlaybackButton } from "./bench/playbacks/playbackScenario";
import { Show } from "./bench/show/showScenario";
import { PaneType } from "./bench/window-system/paneTypes";

/** Ten dimmer lamps: the strip's level, on DMX that rests dark. */
const SUNSTRIP_DIMMERS = {
	manufacturer: "Showtec",
	profile: "Sunstrip Active DMX",
	mode: "10 Channel",
} as const;
/** Ten RGB pixels: the strip's colour. */
const SUNSTRIP = {
	manufacturer: "Showtec",
	profile: "Sunstrip LED RGB 42206",
	mode: "30 Channel",
} as const;

scenario(
	"BENCH-TEMP-RELEASE-001",
	"a Temp runs a Sunstrip Cuelist with its timing, follows into the release, and turns itself off once nothing is held",
	async (t) => {
		await t.show.use(Show.Empty);
		await t.app.open();
		await t.app.expect.ready();
		await t.timing.programmerFade.via.api.set("0s");

		await t.patch.via.api.add({ number: 1, name: "Sunstrip", address: "1.1", ...SUNSTRIP_DIMMERS });
		await t.patch.via.api.add({ number: 2, name: "Sunstrip RGB", address: "1.11", ...SUNSTRIP });

		// Cue 1: both strips full up, and the pixels white.
		await t.selection.fixtures.via.api.items(1, 2);
		await t.encoder.intensity.dimmer.via.api.set(100);
		await t.selection.fixtures.via.api.item(2);
		await t.encoder.color.red.via.api.set(100);
		await t.encoder.color.green.via.api.set(100);
		await t.encoder.color.blue.via.api.set(100);
		const playback = await t.record.playback(1);
		await t.encoder.clear();

		// Cue 2: every attribute of the strip released — Release is a value a Cue can hold.
		await t.command.type("FIXTURE 1 THRU 2");
		await t.keypad.press(["SHIFT", "OFF", "0", "SHIFT", "ENT"]);
		await t.record.cue({ playback, cue: 2 });
		await t.cue.expect(playback, 2).present();
		await t.encoder.clear();
		await t.selection.clear();

		// 0.2 s in and 1 s out on Cue 1; Cue 2 follows 0.3 s after Cue 1 completes, releasing in 1 s.
		const editor = await t.cue.openEditor(playback);
		await editor.edit(1, { fade: "0.2", outFade: "1" });
		await editor.edit(2, { fade: "1", trigger: "FOLLOW", triggerTime: "0.3" });

		await t.playback.configure(playback, {
			name: "Strip release",
			buttonCount: 1,
			hasFader: false,
			buttons: [PlaybackButton.Temp, PlaybackButton.Empty, PlaybackButton.Empty],
		});
		await t.playback.via.api.off(playback);
		const desktop = t.desktop.configure("Temp release");
		const pane = desktop.addPane(
			PaneType.VirtualPlaybacks,
			{ slug: "temp-release", column: 1, row: 1, width: 12, height: 10 },
			{ rows: 1, columns: 1 },
		);
		await desktop.apply();
		const temp = await t.virtualPlayback.assignSource(pane, "Strip release", 1);

		// One press of Temp starts Cue 1 with its own fade.
		const lamps = (level: number | { between: [number, number] }) =>
			Promise.all(
				[2, 11].map((head) =>
					t.expectFixtureDMX(fixture(1, head), { Intensity: level }),
				),
			);
		await t.clock.advanceStep();
		await lamps(0);
		await t.virtualPlayback.activate(pane, 1);
		await t.virtualPlayback.expect.runtime(temp, {
			runtime: { temporary_active: true },
		});
		await t.clock.advanceBy("100ms");
		await lamps({ between: [100, 155] });
		await t.clock.advanceBy("110ms");
		await lamps(255);

		// After the 0.3 s hold, Cue 2 follows without another press and the release fades.
		await t.clock.advanceBy("300ms");
		await t.virtualPlayback.expect.runtime(temp, {
			runtime: { temporary_active: true },
		});
		await t.clock.advanceBy("500ms");
		await lamps({ between: [100, 155] });

		// Once the second-long release has finished, nothing is held: the Playback is off, and so
		// is its Temp button.
		await t.clock.advanceBy("510ms");
		await lamps(0);
		await t.virtualPlayback.expect.runtime(temp, { runtime: { enabled: false } });
		// Temp reads off again: the next press starts the list afresh rather than switching it off.
		await t.virtualPlayback.activate(pane, 1);
		await t.virtualPlayback.expect.runtime(temp, {
			runtime: { temporary_active: true },
		});
		await t.clock.advanceBy("210ms");
		await lamps(255);
	},
);
