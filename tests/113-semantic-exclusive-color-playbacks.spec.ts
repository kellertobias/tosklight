// @bench-semantic-world

import { scenario } from "./bench/core/scenario";
import { StoreMode } from "./bench/groups-presets/groupScenario";
import { PresetFamily } from "./bench/groups-presets/presetScenario";
import { fixture } from "./bench/output/fixtureDmxContract";
import { PlaybackButton } from "./bench/playbacks/playbackScenario";
import { Show } from "./bench/show/showScenario";
import { PaneType } from "./bench/window-system/paneTypes";

// The Südbahnhof colour rig: a Sunstrip pixel bar, two PARs, and an RGBW wash mover.
const SUNSTRIP = {
	manufacturer: "Showtec",
	profile: "Sunstrip LED RGB 42206",
	mode: "30 Channel",
} as const;
const PAR = {
	manufacturer: "Generic",
	profile: "Dimmer RGB Control PAR",
	mode: "Fixed 0, Red, Green, Blue, Fixed 0",
} as const;
const WASH_MOVER = {
	manufacturer: "ROBE",
	profile: "Robin 300 LEDWash",
	mode: "Mode 3",
} as const;

/**
 * The basic colours as the Easy Color encoders set them (Red, Green, Blue and White Blend, in
 * percent), with the DMX each head shows. The show programs Color Intent, so a colour is matched,
 * not copied channel by channel: an encoder percentage is an sRGB component, which an RGB emitter
 * shows at its linear-light level (three-quarter green is 52%, DMX 133), and a full White Blend is
 * the 6500 K white rather than every emitter at full. Every head of the rig shows the same colour.
 */
const COLORS = [
	{ name: "Red", red: 100, green: 0, blue: 0, white: 0, rgb: [255, 0, 0], moverWhite: 0 },
	{ name: "Green", red: 0, green: 100, blue: 0, white: 0, rgb: [0, 255, 0], moverWhite: 0 },
	{ name: "Blue", red: 0, green: 0, blue: 100, white: 0, rgb: [0, 0, 255], moverWhite: 0 },
	{ name: "Amber", red: 100, green: 75, blue: 0, white: 0, rgb: [255, 133, 0], moverWhite: 0 },
	{
		name: "White",
		red: 100,
		green: 100,
		blue: 100,
		white: 100,
		rgb: [255, 240, 253],
		// The RGBW mover makes the white from its white emitter, trimmed by red and blue.
		moverRgb: [15, 0, 13],
		moverWhite: 251,
	},
] as const;

/** A DMX byte, give or take one step of rounding. */
const byte = (value: number) => ({
	between: [Math.max(0, value - 1), Math.min(255, value + 1)] as [number, number],
});

scenario(
	"BENCH-RITUAL-COLOR-001",
	"Color Preset cues in one Virtual Playback exclusion zone keep exactly one colour on the rig and fall back to its resting colour",
	async (t) => {
		await t.show.use(Show.Empty);
		await t.app.open();
		await t.app.expect.ready();
		await t.timing.programmerFade.via.api.set("0s");
		await t.timing.cueFade.set("0s");

		await t.patch.via.api.add({ number: 1, name: "Sunstrip", address: "1.1", ...SUNSTRIP });
		await t.patch.via.api.add({ number: 2, name: "PAR L", address: "1.41", ...PAR });
		await t.patch.via.api.add({ number: 3, name: "PAR R", address: "1.46", ...PAR });
		await t.patch.via.api.add({ number: 4, name: "Wash", address: "1.61", ...WASH_MOVER });
		await t.selection.fixtures.via.api.items(1, 2, 3, 4);
		await t.group.via.api.store(1, { mode: StoreMode.Overwrite });
		await t.selection.clear();

		// One Color Preset per colour, stored for the whole rig.
		for (const [index, color] of COLORS.entries()) {
			await t.group.via.api.select(1);
			await t.encoder.color.red.via.api.set(color.red);
			await t.encoder.color.green.via.api.set(color.green);
			await t.encoder.color.blue.via.api.set(color.blue);
			await t.encoder.color.white.via.api.set(color.white);
			await t.preset.via.api.store(PresetFamily.Color, index + 1, {
				mode: "overwrite",
			});
			await t.encoder.clear();
			await t.selection.clear();
		}

		// A colour cue on its own Playback for each preset: the rig at full on that preset.
		for (const [index, color] of COLORS.entries()) {
			await t.group.via.api.select(1);
			await t.encoder.intensity.dimmer.via.api.set(100);
			await t.preset.via.api.recall(PresetFamily.Color, index + 1);
			const playback = await t.record.playback(1 + index);
			await t.encoder.clear();
			await t.selection.clear();
			await t.playback.configure(playback, {
				name: color.name,
				buttonCount: 1,
				hasFader: false,
				buttons: [
					PlaybackButton.Toggle,
					PlaybackButton.Empty,
					PlaybackButton.Empty,
				],
			});
			await t.playback.via.api.off(playback);
		}

		const desktop = t.desktop.configure("Colour ritual");
		const pane = desktop.addPane(
			PaneType.VirtualPlaybacks,
			{ slug: "colour-ritual", column: 1, row: 1, width: 12, height: 10 },
			{ rows: 1, columns: COLORS.length },
		);
		await desktop.apply();
		const playbacks = [];
		for (const [index, color] of COLORS.entries())
			playbacks.push(
				await t.virtualPlayback.assignSource(pane, color.name, index + 1),
			);
		const cells = COLORS.map((_, index) => index + 1);
		// The zone keeps the name the desk proposes for a show's first zone.
		const zone = "Solo Region 1";
		await t.virtualPlayback.createExclusionZoneWithAttachedShift(pane, zone, cells);
		await t.virtualPlayback.expect.zones([
			{ name: zone, playback_numbers: cells.map((cell) => 1000 + cell) },
		]);

		const expectOnly = async (active: number | null) => {
			for (const [index, playback] of playbacks.entries())
				await t.virtualPlayback.expect.runtime(playback, {
					runtime: { enabled: index === active },
				});
		};
		const expectRig = async (color: (typeof COLORS)[number]) => {
			const rgb = (values: readonly number[]) => ({
				"Color red": byte(values[0]),
				"Color green": byte(values[1]),
				"Color blue": byte(values[2]),
			});
			// The first and last pixel of the bar, both PARs, and the mover at full.
			await t.expectFixtureDMX(fixture(1, 2), rgb(color.rgb));
			await t.expectFixtureDMX(fixture(1, 11), rgb(color.rgb));
			await t.expectFixtureDMX(fixture(2), rgb(color.rgb));
			await t.expectFixtureDMX(fixture(3), rgb(color.rgb));
			await t.expectFixtureDMX(fixture(4), {
				...rgb("moverRgb" in color ? color.moverRgb : color.rgb),
				"Color white": byte(color.moverWhite),
				Intensity: 255,
			});
		};

		// Each colour in turn wins the zone and takes the whole rig.
		for (const [index, color] of COLORS.entries()) {
			await t.virtualPlayback.activate(pane, index + 1);
			await expectOnly(index);
			await t.clock.advanceStep();
			await expectRig(color);
		}
		// Back to an earlier colour: the last one goes off, the earlier one takes over.
		await t.virtualPlayback.activate(pane, 2);
		await expectOnly(1);
		await t.clock.advanceStep();
		await expectRig(COLORS[1]);

		// Turning the winner off starts nothing else and the rig returns to its resting colour.
		await t.virtualPlayback.activate(pane, 2);
		await expectOnly(null);
		await t.clock.advanceStep();
		const resting = { "Color red": 255, "Color green": 255, "Color blue": 255 };
		await t.expectFixtureDMX(fixture(1, 2), resting);
		await t.expectFixtureDMX(fixture(1, 11), resting);
		await t.expectFixtureDMX(fixture(2), resting);
		await t.expectFixtureDMX(fixture(3), resting);
		await t.expectFixtureDMX(fixture(4), {
			...resting,
			"Color white": 255,
			Intensity: 0,
		});
	},
);
