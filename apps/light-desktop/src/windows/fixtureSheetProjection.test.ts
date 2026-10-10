import { describe, expect, it } from "vitest";
import type { RuntimeGroup } from "../features/groupRuntime/groupRuntimeAuthority";
import type { ProgrammerValuesProjection } from "../features/programmerValues/contracts";
import type { AttributeValue } from "../api/types/playback";
import {
	srgbToXyz,
	UV_PREVIEW_HEX,
} from "../features/presetPreview/colorDisplay";
import {
	fixtureSheetColorSwatch,
	fixtureSheetColorReadout,
	fixtureSheetProgrammerValueIndex,
} from "./fixtureSheetProjection";

describe("Fixture Sheet programmer value projection", () => {
	it("uses Programmer order across direct and spread Group values", () => {
		const projection: ProgrammerValuesProjection = {
			revision: 1,
			fixtureValues: [
				{
					fixtureId: "fixture-2",
					attribute: "intensity",
					value: { kind: "normalized", value: 0.8 },
					programmerOrder: 20,
					fade: true,
					fadeMillis: null,
					delayMillis: null,
				},
			],
			groupValues: [
				{
					groupId: "line",
					attribute: "intensity",
					value: { kind: "spread", value: [0.1, 0.5] },
					programmerOrder: 10,
					fade: true,
					fadeMillis: null,
					delayMillis: null,
				},
			],
		};
		const group = {
			id: "line",
			body: { fixtures: ["fixture-1", "fixture-2", "fixture-3"] },
		} as RuntimeGroup;

		const values = fixtureSheetProgrammerValueIndex(projection, [group]);

		expect(values.get("fixture-1")?.get("intensity")?.value).toEqual({
			kind: "normalized",
			value: 0.1,
		});
		expect(values.get("fixture-2")?.get("intensity")?.value).toEqual({
			kind: "normalized",
			value: 0.8,
		});
		expect(values.get("fixture-3")?.get("intensity")?.value).toEqual({
			kind: "normalized",
			value: 0.5,
		});
	});

	it("spreads Group values over canonical source order instead of a legacy cache", () => {
		const projection: ProgrammerValuesProjection = {
			revision: 1,
			fixtureValues: [],
			groupValues: [
				{
					groupId: "derived",
					attribute: "intensity",
					value: { kind: "spread", value: [0, 1] },
					programmerOrder: 1,
					fade: true,
					fadeMillis: null,
					delayMillis: null,
				},
			],
		};
		const groups = [
			{
				id: "source",
				body: {
					fixtures: [],
					source: {
						type: "explicit",
						fixture_ids: ["fixture-1", "fixture-2", "fixture-3"],
					},
				},
				runtime: { master: 1, flashLevel: 1, playbackNumber: null },
			},
			{
				id: "derived",
				body: {
					fixtures: ["stale-cache"],
					source: {
						type: "references",
						references: [{ group_id: "source", rule: { type: "odd" } }],
					},
				},
				runtime: { master: 1, flashLevel: 1, playbackNumber: null },
			},
		] as RuntimeGroup[];

		const values = fixtureSheetProgrammerValueIndex(projection, groups);

		expect(values.get("fixture-1")?.get("intensity")?.value).toEqual({
			kind: "normalized",
			value: 0,
		});
		expect(values.get("fixture-3")?.get("intensity")?.value).toEqual({
			kind: "normalized",
			value: 1,
		});
		expect(values.has("stale-cache")).toBe(false);
	});
});

describe("requested Color family swatch", () => {
	const semantic = (rgb: [number, number, number]): AttributeValue => ({
		kind: "color_program",
		value: {
			kind: "semantic",
			intent: {
				base_xyz: srgbToXyz(...rgb),
				recipe: { version: 1, rgb, amber: 0, approximate: false },
				white_blend: 0,
				white_target: { kelvin: 6504, duv: 0 },
				uv: { amount: 0 },
				relative_output: 1,
				allocation: "preserve_recipe",
			},
		},
	});
	const direct = (
		visible: {
			xyz: { x: number; y: number; z: number };
			relative_output: number;
		} | null,
	): AttributeValue => ({
		kind: "color_program",
		value: {
			kind: "direct",
			recipe: {
				source: {
					profile_id: "profile",
					profile_revision: 1,
					mode_id: "mode",
					head_id: "head",
					path_id: "path",
					profile_digest: "digest",
					native_layout_signature: "layout",
					model_revision: 0,
				},
				channels: [],
			},
			portable: {
				model_revision: 0,
				visible,
				uv: null,
				quality: "unknown",
				limitations: [],
			},
		},
	});
	it("distinguishes requested red, true black, absent and unknown appearance", () => {
		expect(fixtureSheetColorSwatch(semantic([1, 0, 0]))).toBe("#ff0000");
		expect(fixtureSheetColorSwatch(semantic([0, 0, 0]))).toBe("#000000");
		expect(fixtureSheetColorSwatch(null)).toBe("transparent");
		expect(fixtureSheetColorSwatch(direct(null))).toBe("transparent");
		expect(
			fixtureSheetColorSwatch(
				direct({ xyz: srgbToXyz(0, 0, 1), relative_output: 0.25 }),
			),
		).toBe("#0000ff");
		expect(
			fixtureSheetColorSwatch(
				direct({ xyz: srgbToXyz(0, 0, 1), relative_output: 0 }),
			),
		).toBe("#000000");
	});
	it("does not pick a first spread sample or invent a Direct spread appearance", () => {
		const semanticSpread = semantic([1, 0, 0]);
		if (
			semanticSpread.kind !== "color_program" ||
			semanticSpread.value.kind !== "semantic"
		)
			throw new Error("test semantic");
		semanticSpread.value.intent.spreads = [
			{ component: "hue", points: [0, 240] },
		];
		expect(fixtureSheetColorSwatch(semanticSpread)).toBe("transparent");
		const directSpread = direct({
			xyz: srgbToXyz(1, 0, 0),
			relative_output: 1,
		});
		if (
			directSpread.kind !== "color_program" ||
			directSpread.value.kind !== "direct"
		)
			throw new Error("test direct");
		directSpread.value.recipe.spreads = [
			{ binding: { channel_id: "red", function_id: "red" }, points: [0, 255] },
		];
		expect(fixtureSheetColorSwatch(directSpread)).toBe("transparent");
	});
	it("keeps UV-only distinct from visible black", () => {
		const value = semantic([0, 0, 0]);
		if (value.kind !== "color_program" || value.value.kind !== "semantic")
			throw new Error("test semantic");
		value.value.intent.uv.amount = 1;
		expect(fixtureSheetColorSwatch(value)).toBe(UV_PREVIEW_HEX);
	});
});

it("whole-family pending readout overrides legacy native defaults without borrowing its absent Normal", () => {
	const value: AttributeValue = {
		kind: "color_xyz",
		value: srgbToXyz(0, 0, 1),
	};
	const group = {
		id: "color" as const,
		available: true,
		source: "default" as const,
		accessibleName: "Color: Unavailable; Preload Color",
		members: [
			{
				attribute: "color",
				label: "Color",
				value: null,
				text: "Unavailable",
				preloadValue: value,
				preloadText: "Color",
				source: "default" as const,
				dynamics: [],
			},
			{
				attribute: "color.red",
				label: "Red",
				value: { kind: "normalized" as const, value: 0 },
				text: "0%",
				preloadValue: null,
				preloadText: null,
				source: "default" as const,
				dynamics: [],
			},
		],
	};
	expect(fixtureSheetColorReadout(group)).toEqual({
		color: "transparent",
		preloadColor: "#0000ff",
	});
	expect(group.members[0].value).toBeNull();
});
