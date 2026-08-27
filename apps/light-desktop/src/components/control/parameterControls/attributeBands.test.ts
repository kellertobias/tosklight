import { describe, expect, it } from "vitest";
import {
	type AttributeBand,
	attributeBands,
	bandLabel,
	steppedValue,
} from "./attributeBands";

/// A gobo wheel: four gobos, then a band the wheel spins in.
const wheel: AttributeBand[] = [
	{ from: 0, to: 9, kind: "slot", label: "Open", rawValue: 0 },
	{ from: 10, to: 19, kind: "slot", label: "Gobo 1", rawValue: 10 },
	{ from: 20, to: 29, kind: "slot", label: "Gobo 2", rawValue: 20 },
	{ from: 30, to: 39, kind: "slot", label: "Gobo 3", rawValue: 30 },
	{ from: 128, to: 255, kind: "range", label: "Rotate", rawValue: 128 },
];

function raw(normalized: number): number {
	return Math.round(normalized * 255);
}

describe("stepping an indexed attribute", () => {
	it("moves one slot per detent, fast or slow", () => {
		expect(raw(steppedValue(wheel, 0, 1, false))).toBe(10);
		expect(raw(steppedValue(wheel, 10 / 255, 1, true))).toBe(20);
		expect(raw(steppedValue(wheel, 20 / 255, -1, false))).toBe(10);
	});

	it("lands on the value the slot is selected from, not between two slots", () => {
		const stepped = steppedValue(wheel, 15 / 255, 1, false);
		expect(bandLabel(wheel, stepped)).toBe("Gobo 2");
		expect(raw(stepped)).toBe(20);
	});

	it("stays where it is at either end", () => {
		expect(raw(steppedValue(wheel, 0, -1, false))).toBe(0);
		expect(raw(steppedValue(wheel, 1, 1, false))).toBe(255);
	});

	it("sweeps a ranged area in percent once it is reached", () => {
		const entered = steppedValue(wheel, 30 / 255, 1, false);
		// The last slot steps into the rotation band at its near edge.
		expect(raw(entered)).toBe(128);
		expect(bandLabel(wheel, entered)).toBe("Rotate");
		// Inside it the encoder sweeps rather than jumping to the far end.
		const slow = steppedValue(wheel, entered, 1, false);
		const fast = steppedValue(wheel, entered, 1, true);
		expect(raw(slow)).toBe(131);
		expect(raw(fast)).toBe(141);
		expect(bandLabel(wheel, fast)).toBe("Rotate");
	});

	it("leaves a ranged area back onto the slot beside it", () => {
		const stepped = steppedValue(wheel, 128 / 255, -1, false);
		expect(raw(stepped)).toBe(30);
		expect(bandLabel(wheel, stepped)).toBe("Gobo 3");
	});

	it("steps out of a gap onto the nearest band that way", () => {
		expect(raw(steppedValue(wheel, 80 / 255, 1, false))).toBe(128);
		expect(raw(steppedValue(wheel, 80 / 255, -1, false))).toBe(30);
	});

	it("names the slot a value is sitting on", () => {
		expect(bandLabel(wheel, 0)).toBe("Open");
		expect(bandLabel(wheel, 25 / 255)).toBe("Gobo 2");
		expect(bandLabel(wheel, 80 / 255)).toBeUndefined();
	});

	it("does not read a plain continuous channel as a wheel", () => {
		// Pan is one continuous function end to end. Reading that single band as a ranged area
		// would step it in coarse raw jumps instead of the movement its own units ask for.
		expect(attributeBands(fixtures("pan", [sweepFunction("pan")]), ["f1"], "pan")).toBeNull();
	});

	it("reads a channel as a wheel as soon as it has one slot in it", () => {
		const wheelChannel = [
			slotFunction("shutter", "Closed", 0, 15),
			sweepFunction("shutter", 16, 255),
		];
		const bands = attributeBands(fixtures("shutter", wheelChannel), ["f1"], "shutter");
		expect(bands?.map((band) => band.kind)).toEqual(["slot", "range"]);
	});

	it("offers no wheel when two selected fixtures lay the channel out differently", () => {
		const wide = fixtures("gobo.1", [slotFunction("gobo.1", "Open", 0, 127)]);
		const narrow = fixtures("gobo.1", [slotFunction("gobo.1", "Open", 0, 63)], "f2");
		expect(
			attributeBands([...wide, ...narrow], ["f1", "f2"], "gobo.1"),
		).toBeNull();
	});
});

function sweepFunction(attribute: string, from = 0, to = 255) {
	return {
		id: `${attribute}-sweep`,
		name: "",
		dmx_from: from,
		dmx_to: to,
		attribute,
		priority: 0,
		behavior: { type: "continuous", physical_min: 0, physical_max: 1, unit: null },
	};
}

function slotFunction(attribute: string, label: string, from: number, to: number) {
	return {
		id: `${attribute}-${label}`,
		name: label,
		dmx_from: from,
		dmx_to: to,
		attribute,
		priority: 0,
		behavior: {
			type: "indexed",
			semantic_id: label.toLowerCase(),
			label,
			raw_value: from,
		},
	};
}

/// One selected fixture whose mode holds a single channel of the given shape.
function fixtures(attribute: string, functions: unknown[], fixtureId = "f1") {
	const head = { id: "head", master_shared: true };
	return [
		{
			fixture_id: fixtureId,
			logical_heads: [],
			definition: {
				mode_id: "mode",
				profile_snapshot: {
					modes: [
						{
							id: "mode",
							heads: [head],
							channels: [{ head_id: "head", attribute, functions }],
						},
					],
				},
			},
		},
	] as unknown as Parameters<typeof attributeBands>[0];
}
