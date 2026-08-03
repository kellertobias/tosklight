import { describe, expect, it } from "vitest";
import {
	ATTRIBUTE_ENCODER_GROUPS,
	type AttributeEncoderPlacement,
	attributeEncoderGroups,
} from "./attributeEncoderPages";

function descriptor(
	id: string,
	label: string,
	encoder_group: AttributeEncoderPlacement["encoder_group"],
	encoder_page: number,
	encoder_slot: number,
): AttributeEncoderPlacement {
	return { id, label, encoder_group, encoder_page, encoder_slot };
}

describe("attribute encoder pages", () => {
	it("returns the eight ordered encoder groups with their operator labels", () => {
		const groups = attributeEncoderGroups([], new Set());

		expect(groups.map(({ id, label }) => ({ id, label }))).toEqual(
			ATTRIBUTE_ENCODER_GROUPS,
		);
		expect(groups.every((group) => group.pages.length === 0)).toBe(true);
	});

	it("projects a mixed RGB and color-wheel union onto ordered applicable pages", () => {
		const red = descriptor("color.red", "Red", "color", 1, 1);
		const green = descriptor("color.green", "Green", "color", 1, 2);
		const blue = descriptor("color.blue", "Blue", "color", 1, 3);
		const wheel = descriptor("color.wheel.1", "Color Wheel 1", "color", 3, 3);
		const wheelRotation = descriptor(
			"color.wheel.1.rotation",
			"Color Wheel 1 Rotation",
			"color",
			3,
			4,
		);
		const groups = attributeEncoderGroups(
			[wheelRotation, blue, wheel, red, green],
			new Set([
				"color.red",
				"color.green",
				"color.blue",
				"color.wheel.1",
				"color.wheel.1.rotation",
			]),
		);

		const color = groups.find((group) => group.id === "color");
		expect(color?.pages.map((page) => page.number)).toEqual([1, 3]);
		expect(color?.pages[0]?.slots).toEqual([
			red,
			green,
			blue,
			null,
			null,
			null,
		]);
		expect(color?.pages[1]?.slots).toEqual([
			null,
			null,
			wheel,
			wheelRotation,
			null,
			null,
		]);
	});

	it("omits empty pages while preserving holes and six stable slots", () => {
		const pan = descriptor("pan", "Pan", "position", 1, 1);
		const tilt = descriptor("tilt", "Tilt", "position", 1, 2);
		const speed = descriptor(
			"position.speed",
			"Pan/Tilt Speed",
			"position",
			4,
			6,
		);
		const position = attributeEncoderGroups(
			[pan, tilt, speed],
			new Set(["pan", "position.speed"]),
		).find((group) => group.id === "position");

		expect(position?.pages.map((page) => page.number)).toEqual([1, 4]);
		expect(position?.pages[0]?.slots).toEqual([
			pan,
			null,
			null,
			null,
			null,
			null,
		]);
		expect(position?.pages[1]?.slots).toEqual([
			null,
			null,
			null,
			null,
			null,
			speed,
		]);
		expect(position?.pages.every((page) => page.slots.length === 6)).toBe(true);
	});

	it("derives four- and five-encoder pages from one semantic order", () => {
		const placements = [
			descriptor("a", "A", "beam", 1, 1),
			descriptor("b", "B", "beam", 1, 2),
			descriptor("c", "C", "beam", 1, 3),
			{
				...descriptor("pair.a", "Pair A", "beam", 1, 4),
				compound_group: "pair",
			},
			{
				...descriptor("pair.b", "Pair B", "beam", 1, 5),
				compound_group: "pair",
			},
		];
		const supported = new Set(placements.map(({ id }) => id));

		const four = attributeEncoderGroups(placements, supported, 4).find(
			(group) => group.id === "beam",
		);
		expect(
			four?.pages.map((page) => page.slots.map((slot) => slot?.id ?? null)),
		).toEqual([
			["a", "b", "c", null],
			["pair.a", "pair.b", null, null],
		]);

		const five = attributeEncoderGroups(placements, supported, 5).find(
			(group) => group.id === "beam",
		);
		expect(
			five?.pages.map((page) => page.slots.map((slot) => slot?.id ?? null)),
		).toEqual([["a", "b", "c", "pair.a", "pair.b"]]);
	});

	it("keeps derived page identity stable while omitting wholly unsupported pages", () => {
		const placements = [
			descriptor("a", "A", "color", 1, 1),
			descriptor("b", "B", "color", 1, 2),
			descriptor("c", "C", "color", 1, 3),
			descriptor("d", "D", "color", 1, 4),
			descriptor("e", "E", "color", 1, 5),
		];
		const color = attributeEncoderGroups(placements, new Set(["e"]), 4).find(
			(group) => group.id === "color",
		);

		expect(color?.pages).toEqual([
			{
				number: 2,
				slots: [placements[4], null, null, null],
			},
		]);
	});

	it("omits unsupported unknown IDs unless the registry gives them a valid placement", () => {
		const custom = descriptor("vendor.sparkle", "Sparkle", "beam", 7, 2);
		const supported = new Set(["legacy.unknown", "vendor.sparkle"]);

		const withoutPlacement = attributeEncoderGroups([], supported);
		expect(
			withoutPlacement.flatMap((group) =>
				group.pages.flatMap((page) => page.slots),
			),
		).not.toContainEqual(expect.objectContaining({ id: "legacy.unknown" }));

		const withPlacement = attributeEncoderGroups([custom], supported);
		const beam = withPlacement.find((group) => group.id === "beam");
		expect(beam?.pages).toEqual([
			{
				number: 7,
				slots: [null, custom, null, null, null, null],
			},
		]);
	});

	it("rejects duplicate placement deterministically regardless of registry order", () => {
		const alpha = descriptor("custom.alpha", "Alpha", "beam", 2, 4);
		const beta = descriptor("custom.beta", "Beta", "beam", 2, 4);
		for (const registry of [
			[alpha, beta],
			[beta, alpha],
		]) {
			expect(() => attributeEncoderGroups(registry, new Set())).toThrow(
				"Duplicate encoder placement beam:2:4: custom.alpha, custom.beta",
			);
		}
	});

	it.each([
		[
			descriptor("bad.page", "Bad page", "focus", 0, 1),
			"Invalid encoder page for bad.page: 0",
		],
		[
			descriptor("bad.slot", "Bad slot", "focus", 1, 7),
			"Invalid encoder slot for bad.slot: 7",
		],
	] as const)("rejects invalid placement metadata", (invalid, message) => {
		expect(() => attributeEncoderGroups([invalid], new Set())).toThrow(message);
	});
});
