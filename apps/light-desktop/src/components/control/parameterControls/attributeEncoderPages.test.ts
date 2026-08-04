import { describe, expect, it } from "vitest";
import {
	ATTRIBUTE_ENCODER_GROUPS,
	type AttributeEncoderGroupId,
	type AttributeEncoderPlacement,
	attributeEncoderGroups,
	projectPushTurnPlacements,
	resolveAnchoredEncoderPage,
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

	it("packs the real protected mechanisms into the accepted four five and six-wide pages", () => {
		const beam = [
			descriptor("gobo.1", "Gobo 1", "beam", 1, 1),
			descriptor("gobo.1.rotation", "Gobo 1 Rotation", "beam", 1, 2),
			descriptor("gobo.2", "Gobo 2", "beam", 1, 3),
			descriptor("gobo.2.rotation", "Gobo 2 Rotation", "beam", 1, 4),
			descriptor("prism.1", "Prism 1", "beam", 1, 5),
			descriptor("prism.2", "Prism 2", "beam", 1, 6),
			descriptor("animation.1", "Animation", "beam", 2, 1),
			{
				...descriptor("prism.1.rotation", "Prism 1 Rotation", "beam", 2, 2),
				push_turn_of: "prism.1",
			},
			{
				...descriptor("prism.2.rotation", "Prism 2 Rotation", "beam", 2, 3),
				push_turn_of: "prism.2",
			},
			{
				...descriptor(
					"animation.1.rotation",
					"Animation Rotation",
					"beam",
					2,
					4,
				),
				push_turn_of: "animation.1",
			},
		];
		const shapers = [
			descriptor("iris", "Iris", "shapers", 1, 1),
			descriptor(
				"shaper.blade.1.position",
				"Blade 1 Position",
				"shapers",
				1,
				2,
			),
			descriptor("shaper.blade.1.angle", "Blade 1 Angle", "shapers", 1, 3),
			descriptor(
				"shaper.blade.2.position",
				"Blade 2 Position",
				"shapers",
				1,
				4,
			),
			descriptor("shaper.blade.2.angle", "Blade 2 Angle", "shapers", 1, 5),
			descriptor("shaper.rotation", "Shaper Rotation", "shapers", 1, 6),
			descriptor(
				"shaper.blade.3.position",
				"Blade 3 Position",
				"shapers",
				2,
				1,
			),
			descriptor("shaper.blade.3.angle", "Blade 3 Angle", "shapers", 2, 2),
			descriptor(
				"shaper.blade.4.position",
				"Blade 4 Position",
				"shapers",
				2,
				3,
			),
			descriptor("shaper.blade.4.angle", "Blade 4 Angle", "shapers", 2, 4),
		];
		const media = [
			descriptor("media.folder", "Folder", "media", 1, 1),
			descriptor("media.file", "File", "media", 1, 2),
			descriptor("media.mask.folder", "Mask Folder", "media", 1, 3),
			descriptor("media.mask.file", "Mask File", "media", 1, 4),
			descriptor("media.mask.invert", "Mask Invert", "media", 1, 5),
		];
		const placements = projectPushTurnPlacements([
			...beam,
			...shapers,
			...media,
		]);
		const supported = new Set(placements.map(({ id }) => id));
		const ids = (group: AttributeEncoderGroupId, width: 4 | 5 | 6) =>
			attributeEncoderGroups(placements, supported, width)
				.find((candidate) => candidate.id === group)
				?.pages.map((page) => page.slots.map((slot) => slot?.id ?? null));

		expect(ids("beam", 4)).toEqual([
			["gobo.1", "gobo.1.rotation", "gobo.2", "gobo.2.rotation"],
			["prism.1", "prism.2", "animation.1", null],
		]);
		expect(ids("beam", 5)).toEqual([
			["gobo.1", "gobo.1.rotation", "gobo.2", "gobo.2.rotation", null],
			["prism.1", "prism.2", "animation.1", null, null],
		]);
		expect(ids("beam", 6)).toEqual([
			[
				"gobo.1",
				"gobo.1.rotation",
				"gobo.2",
				"gobo.2.rotation",
				"prism.1",
				"prism.2",
			],
			["animation.1", null, null, null, null, null],
		]);

		expect(ids("shapers", 4)).toEqual([
			[
				"shaper.blade.1.position",
				"shaper.blade.1.angle",
				"shaper.blade.2.position",
				"shaper.blade.2.angle",
			],
			[
				"shaper.blade.3.position",
				"shaper.blade.3.angle",
				"shaper.blade.4.position",
				"shaper.blade.4.angle",
			],
			["iris", "shaper.rotation", null, null],
		]);
		expect(ids("shapers", 5)).toEqual([
			[
				"iris",
				"shaper.blade.1.position",
				"shaper.blade.1.angle",
				"shaper.blade.2.position",
				"shaper.blade.2.angle",
			],
			[
				"shaper.rotation",
				"shaper.blade.3.position",
				"shaper.blade.3.angle",
				"shaper.blade.4.position",
				"shaper.blade.4.angle",
			],
		]);
		expect(ids("media", 4)).toEqual([
			["media.folder", "media.file", "media.mask.folder", "media.mask.file"],
			["media.mask.invert", null, null, null],
		]);
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

	it("keeps the active attribute anchored when width changes page boundaries", () => {
		const placements = [
			descriptor("a", "A", "color", 1, 1),
			descriptor("b", "B", "color", 1, 2),
			descriptor("c", "C", "color", 1, 3),
			descriptor("d", "D", "color", 1, 4),
			descriptor("e", "E", "color", 1, 5),
			descriptor("f", "F", "color", 1, 6),
		];
		const supported = new Set(placements.map(({ id }) => id));
		const six = attributeEncoderGroups(placements, supported, 6).find(
			(group) => group.id === "color",
		);
		const four = attributeEncoderGroups(placements, supported, 4).find(
			(group) => group.id === "color",
		);

		expect(resolveAnchoredEncoderPage(six, 1, "e")).toBe(1);
		expect(resolveAnchoredEncoderPage(four, 1, "e")).toBe(2);
		expect(resolveAnchoredEncoderPage(four, 9, "missing")).toBe(2);
	});

	it("projects a push-turn companion onto one encoder without losing companion-only applicability", () => {
		const primary = descriptor("prism.1", "Prism 1", "beam", 1, 5);
		const rotation = {
			...descriptor("prism.1.rotation", "Prism 1 Rotation", "beam", 1, 6),
			push_turn_of: "prism.1",
		};
		const controls = projectPushTurnPlacements([primary, rotation]);

		expect(controls).toEqual([
			{
				...primary,
				push_turn_attribute: "prism.1.rotation",
				push_turn_label: "Prism 1 Rotation",
			},
		]);
		const beam = attributeEncoderGroups(
			controls,
			new Set(["prism.1.rotation"]),
			4,
		).find((group) => group.id === "beam");
		expect(beam?.pages[0]?.slots[0]).toMatchObject({
			id: "prism.1",
			push_turn_attribute: "prism.1.rotation",
		});
		expect(beam?.pages[0]?.slots.filter(Boolean)).toHaveLength(1);
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
