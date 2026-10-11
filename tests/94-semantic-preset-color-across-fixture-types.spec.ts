// @bench-semantic-world

import { expect } from "@playwright/test";

import { fixture } from "./bench/output/fixtureDmxContract";
import { scenario } from "./bench/core/scenario";
import { StoreMode } from "./bench/groups-presets/groupScenario";
import { PresetFamily } from "./bench/groups-presets/presetScenario";
import { Show } from "./bench/show/showScenario";

const WHEEL_SPOT = {
	manufacturer: "Cameo",
	profile: "AURO SPOT Z300",
	mode: "20-Channel",
} as const;
const RGBWA_WASH = {
	manufacturer: "Generic",
	profile: "RGBWA LED",
	mode: "DRGBWA 8-bit dimmer first",
} as const;
// Cameo DMX chart D002965, channel 8: Deep Red is 006–011.
const AURO_DEEP_RED_SLOT = { between: [6, 11] as [number, number] };

scenario(
	"BENCH-PRESET-COLOR-001",
	"a Cue on a Group Color Preset follows a replaced fixture from a color wheel to RGBWA mixing",
	async (t) => {
		await t.show.use(Show.Empty);
		await t.show.colorModel("intent");
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

		// A whole-color Preset keeps its live Color owner across fixture types.
		await t.command.via.api.execute("GROUP 1 AT COLOR 100 DIV 0 DIV 0 DIV 0");
		await t.preset.via.api.store(PresetFamily.Color, 1, { mode: "overwrite" });
		await t.encoder.clear();
		await t.selection.clear();

		// Cue 1 holds Group 1 at full on Color Preset 1, recorded onto a Playback.
		await t.command.via.ui.execute("GROUP 1 AT 100");
		await t.command.via.ui.execute("GROUP 1 AT 2.1");
		const playback = await t.record.via.ui.playback(1);
		await t.cue.expect(playback, 1).present();
		await t.cue.expect(playback, 1).groupValue(1, "intensity", 1);
		const recordedCue = await t.cue.cue(playback, 1);
		const colorPreset = await t.preset.requiredObject(PresetFamily.Color, 1);
		expect(recordedCue?.group_changes).toEqual(
			expect.arrayContaining([
				expect.objectContaining({
					group_id: "1",
					preset_reference: expect.objectContaining({
						preset_instance_id: colorPreset.body.instance_id,
						source_attribute: "color",
					}),
				}),
			]),
		);
		await t.encoder.clear();
		await t.selection.clear();

		await t.playback.via.api.go(playback);
		await t.playback.expect(playback).runtime({
			current_cue_number: "1",
			enabled: true,
		});
		await t.clock.advanceBy("5s");
		await t.expectFixtureDMX(
			fixture(1),
			{
				"Intensity coarse": 255,
				"Intensity fine": 255,
				"Color wheel 1": AURO_DEEP_RED_SLOT,
			},
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

		// Exercise real updates of the same live Color owner and Preset instance.
		await t.command.via.api.execute("GROUP 1 AT COLOR 0 DIV 100 DIV 0 DIV 0");
		await t.preset.via.api.update(PresetFamily.Color, 1, { mode: "all" });
		await t.command.via.api.execute("GROUP 1 AT COLOR 100 DIV 0 DIV 0 DIV 0");
		// Update All preserves the Preset instance and its live Cue reference.
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
			fixture(2),
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
			fixture(2),
			{ Intensity: 0, "Color green": 255, "Color blue": 255, "Color white": 255 },
		);
	},
);
