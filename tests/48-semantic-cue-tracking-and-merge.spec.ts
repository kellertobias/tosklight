// @bench-semantic-world

import { fixture } from "./bench/command-selection/selectionContract";
import { scenario } from "./bench/core/scenario";
import { PlaybackButton } from "./bench/playbacks/playbackScenario";
import { Show } from "./bench/show/showScenario";

scenario(
	"CUE-010",
	"tracking and LTP ownership stay per attribute and reveal the underlying programmer",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();
		await t.timing.programmerFade.via.api.set("0s");
		await t.timing.cueFade.set("0s");

		await t.selection.fixtures.via.api.item(21);
		await t.encoder.intensity.dimmer.via.api.set(100);
		const playback = await t.record.playback(1);
		await t.encoder.clear();
		await t.selection.fixtures.via.api.item(21);
		await t.encoder.intensity.dimmer.via.api.set(50);
		await t.record.cue({ playback, cue: 2 });
		await t.encoder.clear();
		await t.selection.fixtures.via.api.item(21);
		await t.encoder.color.red.via.api.set(0);
		await t.encoder.color.green.via.api.set(0);
		await t.encoder.color.blue.via.api.set(100);
		await t.record.cue({ playback, cue: 3 });
		await t.encoder.clear();
		await t.selection.clear();
		await t.selection.fixtures.via.api.item(2);
		await t.encoder.intensity.dimmer.via.api.set(40);
		await t.record.cue({ playback, cue: 4 });
		await t.encoder.clear();
		await t.playback.configure(playback, {
			buttons: [PlaybackButton.GoBack, PlaybackButton.Go, PlaybackButton.Off],
		});
		await t.cue.configure(playback, { priority: 100 });
		await t.programmer.priority.via.api.set(100);
		await t.playback.via.api.off(playback);

		await t.selection.fixtures.via.api.item(21);
		await t.encoder.color.red.via.api.set(0);
		await t.encoder.color.green.via.api.set(100);
		await t.encoder.color.blue.via.api.set(0);
		await t.clock.advanceBy("1ms");

		await t.playback.go(playback);
		await t.playback.go(playback);
		await t.clock.advanceStep();
		await t.expectFixtureValue(fixture(21), {
			intensity: 0.5,
			"color.red": 0,
			"color.green": 1,
			"color.blue": 0,
		});
		await t.playback.go(playback);
		await t.clock.advanceStep();
		await t.expectFixtureValue(fixture(21), {
			intensity: 0.5,
			"color.red": 0,
			"color.green": 0,
			"color.blue": 1,
		});
		await t.playback.go(playback);
		await t.clock.advanceStep();
		await t.expectFixtureValue(fixture(21), {
			intensity: 0.5,
			"color.red": 0,
			"color.green": 0,
			"color.blue": 1,
		});
		await t.playback.off(playback);
		await t.clock.advanceStep();
		await t.expectFixtureValue(fixture(21), {
			intensity: 0,
			"color.red": 0,
			"color.green": 1,
			"color.blue": 0,
		});
	},
);

scenario(
	"MERGE-002",
	"independent Sequences coexist and retrigger only their stored addresses",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();
		await t.timing.programmerFade.via.api.set("0s");
		await t.timing.cueFade.set("0s");
		await t.programmer.priority.via.api.set(100);

		await t.selection.fixtures.via.api.item(21);
		await t.encoder.intensity.dimmer.via.api.set(60);
		await t.encoder.color.red.via.api.set(0);
		await t.encoder.color.green.via.api.set(0);
		await t.encoder.color.blue.via.api.set(100);
		const sequenceA = await t.record.playback(1);
		await t.playback.configure(sequenceA, {
			name: "Sequence A",
			buttons: [PlaybackButton.On, PlaybackButton.Go, PlaybackButton.Off],
		});
		await t.cue.configure(sequenceA, { priority: 100 });
		await t.encoder.clear();
		await t.selection.clear();

		await t.selection.fixtures.via.api.item(22);
		await t.encoder.intensity.dimmer.via.api.set(40);
		await t.encoder.color.red.via.api.set(100);
		await t.encoder.color.green.via.api.set(70);
		await t.encoder.color.blue.via.api.set(40);
		const sequenceB = await t.record.playback(2);
		await t.playback.configure(sequenceB, {
			name: "Sequence B",
			buttons: [PlaybackButton.On, PlaybackButton.Go, PlaybackButton.Off],
		});
		await t.cue.configure(sequenceB, { priority: 100 });
		await t.encoder.clear();
		await t.selection.clear();
		await t.playback.via.api.off(sequenceA);
		await t.playback.via.api.off(sequenceB);
		await t.playback.via.api.on(sequenceA);
		await t.playback.via.api.on(sequenceB);
		await t.clock.advanceBy("1ms");

		await t.selection.fixtures.via.api.item(21);
		await t.encoder.color.red.via.api.set(100);
		await t.selection.clear();
		await t.selection.fixtures.via.api.item(22);
		await t.encoder.color.blue.via.api.set(80);
		await t.clock.advanceBy("1ms");
		await t.expectFixtureValue(fixture(22), { "color.blue": 0.8 });

		await t.command.clear();
		await t.playback.select(sequenceA);
		// CUE selects a Cue; running one again is the manual's GO TO.
		await t.cue.goto(sequenceA, 1);
		await t.clock.advanceStep();
		await t.expectFixtureValue(fixture(21), {
			intensity: 0.6,
			"color.red": 0,
			"color.green": 0,
			"color.blue": 1,
		});
		await t.expectFixtureValue(fixture(22), {
			intensity: 0.4,
			"color.red": 1,
			"color.green": 0.7,
			"color.blue": 0.8,
		});
	},
);

scenario(
	"MERGE-003",
	"full normal overwrite auto-Offs while partial, disabled, Flash, and Temp restore",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.app.expect.ready();
		await t.timing.programmerFade.via.api.set("0s");
		await t.timing.cueFade.set("0s");

		const recordLook = async (
			number: number,
			name: string,
			intensity: number | undefined,
			color: readonly [number, number, number],
			autoOff: boolean,
			buttons: [PlaybackButton, PlaybackButton, PlaybackButton],
		) => {
			await t.selection.fixtures.via.api.item(21);
			if (intensity != null)
				await t.encoder.intensity.dimmer.via.api.set(intensity);
			await t.encoder.color.red.via.api.set(color[0]);
			await t.encoder.color.green.via.api.set(color[1]);
			await t.encoder.color.blue.via.api.set(color[2]);
			const playback = await t.record.playback(number);
			await t.playback.configure(playback, { name, autoOff, buttons });
			await t.encoder.clear();
			await t.playback.via.api.off(playback);
			return playback;
		};

		const underlying = await recordLook(
			1,
			"Underlying blue",
			undefined,
			[0, 0, 100],
			true,
			[PlaybackButton.On, PlaybackButton.Off, PlaybackButton.Empty],
		);
		const replacing = await recordLook(
			2,
			"Replacing red",
			undefined,
			[100, 0, 0],
			false,
			[PlaybackButton.On, PlaybackButton.Flash, PlaybackButton.Temp],
		);
		const partialUnderlying = await recordLook(
			3,
			"Partial underlying blue",
			100,
			[0, 0, 100],
			true,
			[PlaybackButton.On, PlaybackButton.Off, PlaybackButton.Empty],
		);
		const partialReplacing = await recordLook(
			4,
			"Partial replacing red",
			undefined,
			[100, 0, 0],
			false,
			[PlaybackButton.On, PlaybackButton.Flash, PlaybackButton.Temp],
		);
		await t.playback.expect(underlying).configuration({ auto_off: true });
		await t.playback.expect(replacing).configuration({ auto_off: false });

		await t.playback.on(underlying);
		await t.clock.advanceBy("1ms");
		await t.playback.on(replacing);
		await t.clock.advanceStep();
		await t.playback.expect(underlying).runtime({ enabled: false });
		await t.expectFixtureValue(fixture(21), {
			"color.red": 1,
			"color.green": 0,
			"color.blue": 0,
		});

		await t.playback.configure(underlying, { autoOff: false });
		await t.playback.via.api.off(replacing);
		await t.playback.on(underlying);
		await t.clock.advanceBy("1ms");
		await t.playback.on(replacing);
		await t.clock.advanceStep();
		await t.playback.expect(underlying).runtime({ enabled: true });
		await t.playback.via.api.off(replacing);
		await t.clock.advanceStep();
		await t.expectFixtureValue(fixture(21), {
			"color.red": 0,
			"color.green": 0,
			"color.blue": 1,
		});

		await t.playback.configure(underlying, { autoOff: true });
		await t.clock.advanceBy("1ms");
		await t.playback.via.ui.flash(replacing).hold(async () => {
			await t.clock.advanceStep();
			await t.playback.expect(underlying).runtime({ enabled: true });
			await t.expectFixtureValue(fixture(21), {
				"color.red": 1,
				"color.green": 0,
				"color.blue": 0,
			});
		});
		await t.clock.advanceStep();
		await t.expectFixtureValue(fixture(21), {
			"color.red": 0,
			"color.green": 0,
			"color.blue": 1,
		});

		await t.playback.via.ui.temp(replacing);
		await t.clock.advanceStep();
		await t.playback.expect(underlying).runtime({ enabled: true });
		await t.expectFixtureValue(fixture(21), {
			"color.red": 1,
			"color.green": 0,
			"color.blue": 0,
		});
		await t.playback.via.ui.temp(replacing);
		await t.clock.advanceStep();
		await t.expectFixtureValue(fixture(21), {
			"color.red": 0,
			"color.green": 0,
			"color.blue": 1,
		});

		await t.playback.via.api.off(underlying);
		await t.playback.via.api.off(replacing);
		await t.playback.on(partialUnderlying);
		await t.clock.advanceBy("1ms");
		await t.playback.on(partialReplacing);
		await t.clock.advanceStep();
		await t.playback.expect(partialUnderlying).runtime({ enabled: true });
		await t.expectFixtureValue(fixture(21), {
			intensity: 1,
			"color.red": 1,
			"color.green": 0,
			"color.blue": 0,
		});
	},
);
