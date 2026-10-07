import { describe, expect, it } from "vitest";
import type {
	OutputReadoutSnapshot,
	ProgrammingAttributeValue,
} from "../../../../api/familyEncoderModels";
import { colorSlotDisplay, DIRECT_COLOR_LABEL, withFrameColors } from "./colorSlotDisplay";
import type { ProgrammerValueEntry } from "./familyEncoderDisplay";
import { componentSlot, FIXTURE_A, FIXTURE_B, PAN, RED, WHEEL } from "./familyEncoderTestSupport";

const GREEN = componentSlot("color.green", { kind: "color", component: "green" });
const WHITE_BLEND = componentSlot("color.white_blend", { kind: "color", component: "white_blend" });
const TEMPERATURE = componentSlot(
	"color.temperature",
	{ kind: "color", component: "temperature" },
	{
		descriptor: {
			...RED.descriptor,
			unit: "kelvin",
			display_scale: 1,
			step: 100,
			fine_step: 10,
			domain: { kind: "bounded", bounds: { min: 1000, max: 20000 } },
		},
	},
);

function semantic(rgb: [number, number, number], whiteBlend = 0): ProgrammingAttributeValue {
	return {
		kind: "color_program",
		value: {
			kind: "semantic",
			intent: {
				base_xyz: { x: 0.4, y: 0.2, z: 0.02 },
				recipe: { version: 1, rgb, amber: 0, approximate: false },
				white_blend: whiteBlend,
				white_target: { kelvin: 3200, duv: 0 },
				uv: { amount: 0 },
				relative_output: 1,
				allocation: "preserve_recipe",
			},
		},
	} as ProgrammingAttributeValue;
}

const DIRECT = {
	kind: "color_program",
	value: { kind: "direct", recipe: { channels: [] } },
} as unknown as ProgrammingAttributeValue;

const held = (fixtureId: string, value: ProgrammingAttributeValue, programmerOrder = 1) =>
	({ fixtureId, attribute: "color", value, programmerOrder }) as ProgrammerValueEntry;

describe("semantic Color encoder readouts (TL-653)", () => {
	it("reads the open-white start of fixtures that hold no colour, never a dash", () => {
		const none = { programmerValues: [] };
		expect(colorSlotDisplay(RED, none)).toEqual({ value: 1, text: "100%", source: "requested", start: true });
		expect(colorSlotDisplay(GREEN, none)).toMatchObject({ value: 1, text: "100%", start: true });
		expect(colorSlotDisplay(WHITE_BLEND, none)).toMatchObject({ value: 0, text: "0%", start: true });
		expect(colorSlotDisplay(TEMPERATURE, none)).toMatchObject({ value: 6500, text: "6500 K", start: true });
	});

	it("reads the requested value, keeps black and zero, and reads Mixed when fixtures differ", () => {
		const black = [held(FIXTURE_A, semantic([0, 0, 0])), held(FIXTURE_B, semantic([0, 0, 0]))];
		expect(colorSlotDisplay(RED, { programmerValues: black })).toEqual({ value: 0, text: "0%", source: "requested" });
		const red = [held(FIXTURE_A, semantic([1, 0, 0], 0.5)), held(FIXTURE_B, semantic([1, 0, 0], 0.5))];
		expect(colorSlotDisplay(GREEN, { programmerValues: red })).toMatchObject({ value: 0, text: "0%" });
		expect(colorSlotDisplay(WHITE_BLEND, { programmerValues: red })).toMatchObject({ value: 0.5, text: "50%" });
		expect(colorSlotDisplay(TEMPERATURE, { programmerValues: red })).toMatchObject({ text: "3200 K" });
		// One fixture programmed green-free, the other still at its open-white start: Mixed.
		const partial = [held(FIXTURE_A, semantic([1, 0, 0]))];
		expect(colorSlotDisplay(GREEN, { programmerValues: partial })).toEqual({
			value: null,
			text: "Mixed",
			source: "requested",
		});
		expect(colorSlotDisplay(RED, { programmerValues: partial })).toEqual({
			value: 1,
			text: "100%",
			source: "requested",
		});
	});

	it("reads Direct for a Direct colour and Mixed beside a semantic one", () => {
		const direct = [held(FIXTURE_A, DIRECT), held(FIXTURE_B, DIRECT)];
		expect(colorSlotDisplay(RED, { programmerValues: direct })).toEqual({
			value: null,
			text: DIRECT_COLOR_LABEL,
			source: "requested",
		});
		const mixed = [held(FIXTURE_A, DIRECT)];
		expect(colorSlotDisplay(RED, { programmerValues: mixed })).toMatchObject({ value: null, text: "Mixed" });
	});

	it("reads the selected group's colour for members without a newer value of their own", () => {
		const groupValues = [{ groupId: "front", attribute: "color", value: semantic([0, 0, 1]), programmerOrder: 5 }];
		expect(
			colorSlotDisplay(RED, { programmerValues: [], groupValues, groupId: "front" }),
		).toEqual({ value: 0, text: "0%", source: "requested" });
		// An older fixture value is covered by the group; a newer one wins for its fixture.
		const older = [held(FIXTURE_A, semantic([1, 0, 0]), 1)];
		expect(
			colorSlotDisplay(RED, { programmerValues: older, groupValues, groupId: "front" }),
		).toMatchObject({ value: 0, text: "0%" });
		const newer = [held(FIXTURE_A, semantic([1, 0, 0]), 9)];
		expect(
			colorSlotDisplay(RED, { programmerValues: newer, groupValues, groupId: "front" }),
		).toMatchObject({ value: null, text: "Mixed" });
		// Another group's value is not the selection's.
		expect(
			colorSlotDisplay(RED, { programmerValues: [], groupValues, groupId: "back" }),
		).toMatchObject({ value: 1, start: true });
	});

	it("leaves Wheels and other families to their own readouts", () => {
		expect(colorSlotDisplay(WHEEL, { programmerValues: [] })).toBeNull();
		expect(colorSlotDisplay(PAN, { programmerValues: [] })).toBeNull();
	});
});

/** A displayed-frame readout whose owners resolve `colors` (absent: no colour in the frame). */
function frame(colors: Record<string, ProgrammingAttributeValue | undefined>): OutputReadoutSnapshot {
	return {
		lane: "normal",
		scope: { show_id: null },
		lease: 3,
		revision: 1,
		owners: Object.entries(colors).map(([fixture_id, color]) => ({
			fixture_id,
			...(color ? { color } : {}),
			position: { available: false, commands: [] },
		})),
	} as OutputReadoutSnapshot;
}

describe("semantic Color encoder readouts of a running Cue or Playback (TL-657)", () => {
	it("reads the frame's colour, not the open-white start, while the Programmer is empty", () => {
		const readouts = frame({ [FIXTURE_A]: semantic([1, 0, 0]), [FIXTURE_B]: semantic([1, 0, 0]) });
		const input = { programmerValues: [], readouts };
		expect(colorSlotDisplay(RED, input)).toEqual({ value: 1, text: "100%", source: "requested" });
		expect(colorSlotDisplay(GREEN, input)).toEqual({ value: 0, text: "0%", source: "requested" });
		expect(colorSlotDisplay(TEMPERATURE, input)).toMatchObject({ text: "3200 K" });
	});

	it("keeps black at 0% and reads differing frame colours Mixed", () => {
		const black = frame({ [FIXTURE_A]: semantic([0, 0, 0]), [FIXTURE_B]: semantic([0, 0, 0]) });
		expect(colorSlotDisplay(RED, { programmerValues: [], readouts: black })).toEqual({
			value: 0,
			text: "0%",
			source: "requested",
		});
		const differing = frame({ [FIXTURE_A]: semantic([1, 0, 0]), [FIXTURE_B]: semantic([0, 0, 1]) });
		expect(colorSlotDisplay(RED, { programmerValues: [], readouts: differing })).toMatchObject({
			value: null,
			text: "Mixed",
		});
	});

	it("reads the start only for a fixture whose frame holds no colour", () => {
		const partial = frame({ [FIXTURE_A]: semantic([0, 0, 1]), [FIXTURE_B]: undefined });
		expect(colorSlotDisplay(RED, { programmerValues: [], readouts: partial })).toMatchObject({
			value: null,
			text: "Mixed",
		});
		const none = frame({ [FIXTURE_A]: undefined, [FIXTURE_B]: undefined });
		expect(colorSlotDisplay(RED, { programmerValues: [], readouts: none })).toMatchObject({
			value: 1,
			start: true,
		});
	});

	it("lets the Programmer and the selected group win over the frame", () => {
		const readouts = frame({ [FIXTURE_A]: semantic([1, 0, 0]), [FIXTURE_B]: semantic([1, 0, 0]) });
		const programmed = [held(FIXTURE_A, semantic([0, 1, 0])), held(FIXTURE_B, semantic([0, 1, 0]))];
		expect(colorSlotDisplay(RED, { programmerValues: programmed, readouts })).toMatchObject({
			value: 0,
			text: "0%",
		});
		const groupValues = [{ groupId: "front", attribute: "color", value: semantic([0, 0, 1]) }];
		expect(
			colorSlotDisplay(RED, { programmerValues: [], groupValues, groupId: "front", readouts }),
		).toMatchObject({ value: 0, text: "0%" });
	});

	it("reads Direct for a frame Direct colour", () => {
		const readouts = frame({ [FIXTURE_A]: DIRECT, [FIXTURE_B]: DIRECT });
		expect(colorSlotDisplay(RED, { programmerValues: [], readouts })).toMatchObject({
			text: DIRECT_COLOR_LABEL,
		});
	});

	it("adds frame colours only for fixtures without a Programmer Color", () => {
		const readouts = frame({ [FIXTURE_A]: semantic([1, 0, 0]), [FIXTURE_B]: semantic([0, 0, 1]) });
		const entries = [held(FIXTURE_A, semantic([0, 1, 0]))];
		expect(withFrameColors(entries, readouts, [FIXTURE_A, FIXTURE_B])).toEqual([
			entries[0],
			{ fixtureId: FIXTURE_B, attribute: "color", value: semantic([0, 0, 1]) },
		]);
		expect(withFrameColors(entries, null, [FIXTURE_A, FIXTURE_B])).toBe(entries);
	});
});
