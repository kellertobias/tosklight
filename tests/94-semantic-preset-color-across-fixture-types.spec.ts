// @bench-semantic-world

import { scenario } from "./bench/core/scenario";
import { StoreMode } from "./bench/groups-presets/groupScenario";
import { PresetFamily } from "./bench/groups-presets/presetScenario";
import { Show } from "./bench/show/showScenario";

// The shipped Cameo Auro Spot Z300 profile carries its color wheel as one continuous range with
// no named slots, so "red" has no wheel position there. The Claypaky Sharpy names its RED slot.
const WHEEL_SPOT = {
	manufacturer: "Claypaky",
	profile: "Sharpy",
	mode: "Standard",
} as const;
const RGBWA_WASH = {
	manufacturer: "Generic",
	profile: "RGBWA LED",
	mode: "DRGBWA 8-bit dimmer first",
} as const;
const SHARPY_RED_SLOT = { between: [9, 12] as [number, number] };

scenario(
	"BENCH-PRESET-COLOR-001",
	"a Cue on a Group Color Preset follows a replaced fixture from a color wheel to RGBWA mixing",
	async (t) => {
		await t.show.use(Show.Empty);
		await t.app.open();
		await t.app.expect.ready();
		await t.timing.programmerFade.via.api.set("0s");

		// A moving light with a color wheel, stored in Group 1.
		await t.patch.via.api.add({
			number: 1,
			name: "Wheel Spot",
			address: "1.1",
			...WHEEL_SPOT,
		});
		await t.selection.fixtures.via.api.item(1);
		await t.group.via.api.store(1, { mode: StoreMode.Overwrite });
		await t.group.expect(1).fixtures(1);

		// Color Preset 1 is red on the wheel.
		await t.group.via.api.select(1);
		await t.encoder.discrete.set("color.wheel.1", "red");
		await t.preset.via.api.store(PresetFamily.Color, 1, { mode: "overwrite" });
		await t.encoder.clear();
		await t.selection.clear();

		// Cue 1 holds Group 1 at full on Color Preset 1, recorded onto a Playback.
		await t.group.via.api.select(1);
		await t.encoder.intensity.dimmer.via.api.set(100);
		await t.preset.via.api.recall(PresetFamily.Color, 1);
		const playback = await t.record.via.ui.playback(1);
		await t.cue.expect(playback, 1).present();
		await t.encoder.clear();
		await t.selection.clear();

		await t.playback.via.api.go(playback);
		await t.playback.expect(playback).runtime({
			current_cue_number: "1",
			enabled: true,
		});
		await t.clock.advanceBy("5s");
		await t.expectFixtureDMX(
			{ fixture: 1 },
			{ Intensity: 255, "Color wheel 1": SHARPY_RED_SLOT },
		);

		await t.playback.via.api.off(playback);
		await t.playback.expect(playback).runtime({ enabled: false });
		await t.clock.advanceBy("5s");

		// Replace the wheel light with an RGBWA wash in the same Group.
		await t.patch.via.api.remove(1);
		await t.group.expect(1).empty();
		await t.patch.via.api.add({
			number: 2,
			name: "RGBWA Wash",
			address: "1.1",
			...RGBWA_WASH,
		});
		await t.selection.fixtures.via.api.item(2);
		await t.group.via.api.store(1, { mode: StoreMode.Merge });
		await t.group.expect(1).fixtures(2);
		await t.selection.clear();

		// Update Color Preset 1 so the wash is red as well.
		await t.group.via.api.select(1);
		await t.encoder.color.red.via.api.set(100);
		await t.encoder.color.green.via.api.set(0);
		await t.encoder.color.blue.via.api.set(0);
		await t.encoder.color.white.via.api.set(0);
		await t.encoder.color.amber.via.api.set(0);
		// Update All, because the wash's color addresses are not yet part of the Preset.
		await t.preset.via.api.update(PresetFamily.Color, 1, { mode: "all" });
		await t.encoder.clear();
		await t.selection.clear();

		// The unchanged Cue now plays the wash red, with every color emitter under its control.
		await t.playback.via.api.go(playback);
		await t.playback.expect(playback).runtime({
			current_cue_number: "1",
			enabled: true,
		});
		await t.clock.advanceBy("5s");
		await t.expectFixtureDMX(
			{ fixture: 2 },
			{
				Intensity: 255,
				"Color red": 255,
				"Color green": 0,
				"Color blue": 0,
				"Color white": 0,
				"Color amber": 0,
			},
		);

		// Those values belong to the Playback: switching it off returns the wash to its defaults.
		await t.playback.via.api.off(playback);
		await t.clock.advanceBy("5s");
		await t.expectFixtureDMX(
			{ fixture: 2 },
			{ Intensity: 0, "Color green": 255, "Color blue": 255, "Color white": 255 },
		);
	},
);
