// @bench-semantic-world

import { scenario } from "./bench/core/scenario";
import { StoreMode } from "./bench/groups-presets/groupScenario";
import { fixture } from "./bench/output/fixtureDmxContract";
import { Show } from "./bench/show/showScenario";
import { PaneType } from "./bench/window-system/paneTypes";

const MOVER = {
	manufacturer: "ROBE",
	profile: "Robin 300 LEDWash",
	mode: "Mode 3",
} as const;
const MOVERS = [1, 2];
/** The Robin 300 LEDWash's declared travel, centred on home (Pan 0°, Tilt 0°). */
const TRAVEL = { pan: 450, tilt: 300 };
const CYCLE_MILLIS = 4_000;
const PAN_AMPLITUDE = 90;
const TILT_AMPLITUDE = 30;

/** The DMX byte an Angle lands on, give or take a couple of steps of rounding. */
const angle = (axis: "pan" | "tilt", degrees: number) => {
	const byte = Math.round((degrees / TRAVEL[axis] + 0.5) * 255);
	return { between: [byte - 3, byte + 3] as [number, number] };
};

scenario(
	"BENCH-DYNAMIC-EDITOR-001",
	"a two-axis Circle built by touch in the Dynamics editor stores typed Angle lanes in degrees and moves the movers",
	async (t) => {
		await t.show.use(Show.Empty);
		await t.app.open();
		await t.app.expect.ready();
		await t.timing.programmerFade.via.api.set("0s");

		for (const number of MOVERS)
			await t.patch.via.api.add({
				number,
				name: `Mover ${number}`,
				address: `1.${1 + (number - 1) * 15}`,
				...MOVER,
			});
		await t.selection.fixtures.via.api.items(...MOVERS);
		await t.group.via.api.store(1, { mode: StoreMode.Overwrite });
		await t.group.via.api.select(1);

		await t.builtIn.open(PaneType.Dynamics);

		// Pan is an Angle lane in degrees; its Tilt partner follows Current until it is animated.
		const circle = await t.dynamic.editor.create(1, {
			group: "position",
			attribute: "Pan",
		});
		await t.dynamic.editor.expect.lanes(circle, [
			{ family: "position", component: "pan" },
			{ family: "position", component: "tilt" },
		]);
		await t.dynamic.editor.expect.encoder("Middle", "Current");
		await t.dynamic.editor.expect.encoder("Amplitude", "45°");
		await t.dynamic.editor.setEncoder("Amplitude", String(PAN_AMPLITUDE));
		await t.dynamic.editor.expect.encoder("Amplitude", `${PAN_AMPLITUDE}°`);

		// Adding Tilt replaces the Current partner with an animated Tilt lane.
		await t.dynamic.editor.addLane(circle, { group: "position", attribute: "Tilt" });
		await t.dynamic.editor.expect.lanes(circle, [
			{ family: "position", component: "pan" },
			{ family: "position", component: "tilt" },
		]);
		await t.dynamic.editor.selectLane(2, "Tilt");
		await t.dynamic.editor.chooseCurve("Cosinus");
		await t.dynamic.editor.expect.encoder("Amplitude", `${TILT_AMPLITUDE}°`);
		await t.dynamic.editor.expect.laneConfiguration(circle, 1, {
			mode: "middle_amplitude",
			configuration: {
				function: "sinus",
				middle: { kind: "current" },
				amplitude: { kind: "scalar", value: PAN_AMPLITUDE },
			},
		});
		await t.dynamic.editor.expect.laneConfiguration(circle, 2, {
			mode: "middle_amplitude",
			configuration: {
				function: "cosinus",
				middle: { kind: "current" },
				amplitude: { kind: "scalar", value: TILT_AMPLITUDE },
			},
		});
		await t.dynamic.editor.expect.noError();

		await t.dynamic.editor.takeSelectionAndClose();
		await t.dynamic.editor.toggle(circle);

		/**
		 * Mover 1 at `phase` of its cycle and mover 2 half a cycle on: Pan swings on a sine and Tilt
		 * on a cosine around home, so together the beam draws a circle.
		 */
		const expectCircle = async (phase: number) => {
			for (const [index, number] of MOVERS.entries()) {
				const turn = 2 * Math.PI * ((phase + index * 0.5) % 1);
				await t.expectFixtureDMX(fixture(number), {
					"Pan coarse": angle("pan", PAN_AMPLITUDE * Math.sin(turn)),
					"Tilt coarse": angle("tilt", TILT_AMPLITUDE * Math.cos(turn)),
				});
			}
		};
		for (const phase of [0.25, 0.5, 0.75]) {
			await t.clock.advanceBy(`${CYCLE_MILLIS / 4}ms`);
			await expectCircle(phase);
		}
	},
);

scenario(
	"BENCH-DYNAMIC-EDITOR-002",
	"Color and Zoom lanes chosen in the Dynamics editor are stored as semantic family lanes in their own units",
	async (t) => {
		await t.show.use(Show.Empty);
		await t.app.open();
		await t.app.expect.ready();
		await t.builtIn.open(PaneType.Dynamics);

		const look = await t.dynamic.editor.create(2, { group: "color", attribute: "Red" });
		await t.dynamic.editor.expect.encoder("Top", "100%");
		await t.dynamic.editor.expect.encoder("Bottom", "0%");
		await t.dynamic.editor.addLane(look, { group: "color", attribute: "White Blend" });
		await t.dynamic.editor.addLane(look, { group: "focus", attribute: "Zoom" });
		await t.dynamic.editor.selectLane(3, "Zoom");
		await t.dynamic.editor.expect.encoder("Top", "40°");
		await t.dynamic.editor.expect.encoder("Bottom", "10°");
		await t.dynamic.editor.expect.lanes(look, [
			{ family: "color", component: "red" },
			{ family: "color", component: "white_blend" },
			{ family: "zoom" },
		]);
		await t.dynamic.editor.expect.noError();
	},
);
