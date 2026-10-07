import { describe, expect, it } from "vitest";
import type { ProgrammingColorComponent } from "../../api/familyEncoderModels";
import type { StoredPreset } from "../../api/types";
import {
	colorValueColors,
	requestedVisibleXyz,
	srgbToXyz,
	UV_PREVIEW_HEX,
	xyzDisplayHex,
} from "./colorDisplay";
import {
	MAX_POSITION_DOTS,
	type PositionPoint,
	previewDots,
	representativePoints,
} from "./positionDots";
import {
	MAX_COLOR_SEGMENTS,
	presetIntentPreview,
	presetTileArtwork,
} from "./presetPreview";

type Spread = { component: ProgrammingColorComponent; points: number[] };

/** A semantic Color intent recorded from an sRGB recipe, as the virtual engine stores it. */
function semantic(rgb: [number, number, number], options: { spreads?: Spread[]; whiteBlend?: number; output?: number; uv?: number } = {}) {
	return {
		kind: "color_program",
		value: {
			kind: "semantic",
			intent: {
				base_xyz: srgbToXyz(...rgb),
				recipe: { version: 1, rgb, amber: 0, approximate: false },
				white_blend: options.whiteBlend ?? 0,
				white_target: { kelvin: 6504, duv: 0 },
				uv: { amount: options.uv ?? 0 },
				relative_output: options.output ?? 1,
				allocation: "preserve_recipe" as const,
				...(options.spreads ? { spreads: options.spreads } : {}),
			},
		},
	};
}

function angles(pan: number | number[], tilt: number | number[]) {
	const scalar = (value: number | number[]) =>
		Array.isArray(value) ? { kind: "spread", value } : { kind: "value", value };
	return { kind: "position", value: { kind: "angles", pan_degrees: scalar(pan), tilt_degrees: scalar(tilt) } };
}

function target(x: number, y: number, z = 0) {
	const value = (number: number) => ({ kind: "value", value: number });
	return {
		kind: "position",
		value: { kind: "target", reference: { kind: "origin" }, offset_metres: [value(x), value(y), value(z)] },
	};
}

function preset(family: StoredPreset["family"], parts: Partial<StoredPreset> = {}): StoredPreset {
	return { name: "Preset", number: 1, family, values: {}, ...parts };
}

function perFixture(attribute: string, values: unknown[]) {
	return Object.fromEntries(values.map((value, index) => [`fixture-${index + 1}`, { [attribute]: value }]));
}

function colors(body: StoredPreset, members?: ReadonlyMap<string, readonly string[]>) {
	const preview = presetIntentPreview(body, members);
	if (preview?.kind !== "color") throw new Error(`expected a colour preview, got ${preview?.kind}`);
	return preview;
}

describe("Color Intent display colour", () => {
	it("shows the requested hue at full brightness and keeps a requested black black", () => {
		expect(xyzDisplayHex(srgbToXyz(1, 0, 0))).toBe("#ff0000");
		// Intensity is its own family: a dim blue still reads as blue.
		expect(xyzDisplayHex(srgbToXyz(0, 0, 0.3))).toBe("#0000ff");
		expect(xyzDisplayHex({ x: 0, y: 0, z: 0 })).toBe("#000000");
	});

	it("follows the engine's White Blend envelope and Relative Output", () => {
		const red = semantic([1, 0, 0]).value.intent;
		const channels = (hex: string) => [1, 3, 5].map((at) => Number.parseInt(hex.slice(at, at + 2), 16));
		// Full White Blend is the white target: near-neutral at 6504 K, amber at 3200 K.
		const neutral = channels(xyzDisplayHex(requestedVisibleXyz({ ...red, white_blend: 1 })));
		expect(Math.min(...neutral)).toBeGreaterThan(0xf0);
		const warm = channels(
			xyzDisplayHex(requestedVisibleXyz({ ...red, white_blend: 1, white_target: { kelvin: 3200, duv: 0 } })),
		);
		expect(warm[0]).toBe(0xff);
		expect(warm[2]).toBeLessThan(0xc0);
		expect(xyzDisplayHex(requestedVisibleXyz({ ...red, white_blend: 0.5 }))).not.toBe("#ff0000");
		expect(xyzDisplayHex(requestedVisibleXyz({ ...red, relative_output: 0 }))).toBe("#000000");
	});

	it("draws a UV-only request as UV and an unknown Direct appearance as unknown", () => {
		expect(colorValueColors(semantic([0, 0, 0], { uv: 1 }) as never)).toEqual([{ hex: UV_PREVIEW_HEX, uv: true }]);
		const direct = {
			kind: "color_program",
			value: {
				kind: "direct",
				recipe: { source: {}, channels: [] },
				portable: { model_revision: 1, visible: null, uv: null, quality: "unknown", limitations: [] },
			},
		};
		expect(colorValueColors(direct as never)).toEqual([{ hex: null }]);
	});

	it("samples a hue spread along its shortest arc through the local recipes", () => {
		const spread = semantic([1, 0, 0], { spreads: [{ component: "hue", points: [0, 240] }] });
		// Red to blue the short way passes magenta, never green.
		expect(colorValueColors(spread as never).map((color) => color.hex)).toEqual(["#ff0000", "#ff00ff", "#0000ff"]);
	});
});

describe("Color preset preview", () => {
	it("shows one segment per distinct colour in hue order, never an average", () => {
		const preview = colors(
			preset("Color", {
				values: perFixture("color", [semantic([0, 0, 1]), semantic([1, 0, 0]), semantic([1, 0, 0]), semantic([0, 1, 0])]),
			}),
		);
		expect(preview.colors.map((color) => color.hex)).toEqual(["#ff0000", "#00ff00", "#0000ff"]);
		expect(preview.distinct).toBe(3);
	});

	it("treats rounding noise between equal intents as one colour", () => {
		const preview = colors(
			preset("Color", { values: perFixture("color", [semantic([1, 0, 0]), semantic([0.999, 0.001, 0])]) }),
		);
		expect(preview.colors).toHaveLength(1);
	});

	it("previews a universal colour and a stored XYZ colour", () => {
		const universal = colors(preset("Color", { universal_values: { color: semantic([0, 0, 1]) } }));
		expect(universal.colors.map((color) => color.hex)).toEqual(["#0000ff"]);
		const xyz = colors(
			preset("Color", { universal_values: { color: { kind: "color_xyz", value: srgbToXyz(0, 1, 0) } } }),
		);
		expect(xyz.colors.map((color) => color.hex)).toEqual(["#00ff00"]);
	});

	it("keeps the first and last of more distinct colours than a tile can show", () => {
		const hues = Array.from({ length: 12 }, (_, index) => index * 30);
		const preview = colors(
			preset("Color", {
				universal_values: { color: semantic([1, 0, 0], { spreads: [{ component: "hue", points: [0, 330] }] }) },
				values: perFixture(
					"color",
					hues.map((hue) =>
						semantic([1, 0, 0], { spreads: [{ component: "hue", points: [hue, hue] }] }),
					),
				),
			}),
		);
		expect(preview.colors).toHaveLength(MAX_COLOR_SEGMENTS);
		expect(preview.distinct).toBeGreaterThan(MAX_COLOR_SEGMENTS);
		expect(preview.colors[0].hex).toBe("#ff0000");
	});

	it("resolves a Group family template and its member exceptions over the stored membership", () => {
		const groupValue = {
			kind: "group_family",
			value: { owner: "color", template: semantic([1, 0, 0]), members: { b: semantic([0, 0, 1]) } },
		};
		const body = preset("Color", { group_values: { front: { color: groupValue } } });
		expect(colors(body, new Map([["front", ["a", "b"]]])).colors.map((c) => c.hex)).toEqual(["#ff0000", "#0000ff"]);
		// Every member is an exception: the template colour reaches nobody.
		expect(colors(body, new Map([["front", ["b"]]])).colors.map((c) => c.hex)).toEqual(["#0000ff"]);
	});

	it("samples a Group spread once per ordered member", () => {
		const spread = semantic([1, 0, 0], { spreads: [{ component: "hue", points: [0, 120] }] });
		const body = preset("Color", { group_values: { front: { color: spread } } });
		const members = new Map([["front", ["a", "b", "c"]]]);
		expect(colors(body, members).colors.map((c) => c.hex)).toEqual(["#ff0000", "#ffff00", "#00ff00"]);
	});

	it("gives a preset stored before intentions no preview", () => {
		const legacy = preset("Color", {
			values: { "fixture-1": { "color.red": { kind: "normalized", value: 1 } } },
		});
		expect(presetIntentPreview(legacy)).toBeNull();
	});
});

describe("Position preset preview", () => {
	const grid = Array.from({ length: 25 }, (_, index) => ({
		space: "angles" as const,
		x: (index % 5) * 10,
		y: Math.floor(index / 5) * 10,
	}));

	it("keeps at most ten dots and always the extremes on both axes", () => {
		const points: PositionPoint[] = [
			...grid,
			{ space: "angles", x: -90, y: 20 },
			{ space: "angles", x: 140, y: 20 },
			{ space: "angles", x: 20, y: -45 },
			{ space: "angles", x: 20, y: 95 },
		];
		const chosen = representativePoints(points);
		expect(chosen).toHaveLength(MAX_POSITION_DOTS);
		for (const extreme of [
			{ x: -90, y: 20 },
			{ x: 140, y: 20 },
			{ x: 20, y: -45 },
			{ x: 20, y: 95 },
		])
			expect(chosen).toContainEqual({ space: "angles", ...extreme });
	});

	it("keeps every corner of a grid", () => {
		const chosen = representativePoints(grid);
		for (const corner of [
			{ x: 0, y: 0 },
			{ x: 40, y: 0 },
			{ x: 0, y: 40 },
			{ x: 40, y: 40 },
		])
			expect(chosen).toContainEqual({ space: "angles", ...corner });
	});

	it("samples the rest of the distribution evenly, so a dense cluster keeps more dots", () => {
		const cluster = Array.from({ length: 40 }, (_, index) => ({ space: "angles" as const, x: index * 0.5, y: 0 }));
		const points: PositionPoint[] = [...cluster, { space: "angles", x: 100, y: 0 }, { space: "angles", x: 101, y: 0 }];
		const chosen = representativePoints(points);
		expect(chosen).toHaveLength(MAX_POSITION_DOTS);
		expect(chosen.filter((point) => point.x < 50).length).toBeGreaterThanOrEqual(8);
		expect(chosen).toContainEqual({ space: "angles", x: 101, y: 0 });
	});

	it("is deterministic whatever order the fixtures arrive in", () => {
		const shuffled = [...grid].reverse();
		const rotated = [...grid.slice(7), ...grid.slice(0, 7)];
		const expected = representativePoints(grid);
		expect(representativePoints(shuffled)).toEqual(expected);
		expect(representativePoints(rotated)).toEqual(expected);
	});

	it("collapses identical aims to one dot and keeps the true shape of a spread", () => {
		expect(representativePoints([grid[0], { ...grid[0] }])).toHaveLength(1);
		expect(previewDots([{ space: "angles", x: 30, y: 30 }])).toEqual([{ x: 0.5, y: 0.5 }]);
		const line = previewDots([
			{ space: "target", x: -4, y: 0 },
			{ space: "target", x: 4, y: 0 },
		]);
		expect(line.map((dot) => dot.y)).toEqual([0.5, 0.5]);
		expect(line[0].x).toBeCloseTo(0.1);
		expect(line[1].x).toBeCloseTo(0.9);
	});

	it("previews stored Pan/Tilt per fixture, a universal Target, and a spread", () => {
		const fan = presetIntentPreview(
			preset("Position", { values: perFixture("position", [angles(-30, 40), angles(0, 40), angles(30, 40)]) }),
		);
		expect(fan).toMatchObject({ kind: "position", space: "angles", distinct: 3 });
		const aim = presetIntentPreview(preset("Position", { universal_values: { position: target(2, 3) } }));
		expect(aim).toMatchObject({ kind: "position", space: "target", dots: [{ x: 0.5, y: 0.5 }] });
		const spread = presetIntentPreview(
			preset("Position", { group_values: { truss: { position: angles([-40, 40], 30) } } }),
			new Map([["truss", Array.from({ length: 24 }, (_, index) => `f${index}`)]]),
		);
		expect(spread).toMatchObject({ kind: "position", distinct: 24 });
		if (spread?.kind !== "position") throw new Error("expected a position preview");
		expect(spread.dots).toHaveLength(MAX_POSITION_DOTS);
	});

	it("has no preview for raw Pan/Tilt channels or for Intensity and Beam presets", () => {
		const raw = preset("Position", { values: { "fixture-1": { pan: { kind: "normalized", value: 0.5 } } } });
		expect(presetIntentPreview(raw)).toBeNull();
		expect(presetIntentPreview(preset("Intensity", { universal_values: { color: semantic([1, 0, 0]) } }))).toBeNull();
		expect(presetIntentPreview(preset("Beam", { universal_values: { position: target(0, 0) } }))).toBeNull();
	});

	it("lets a Mixed preset show its colour first and otherwise its aim", () => {
		expect(
			presetIntentPreview(
				preset("Mixed", { universal_values: { color: semantic([1, 0, 0]), position: target(0, 0) } }),
			)?.kind,
		).toBe("color");
		expect(presetIntentPreview(preset("Mixed", { universal_values: { position: target(0, 0) } }))?.kind).toBe(
			"position",
		);
	});
});

describe("Preset tile artwork precedence", () => {
	const preview = presetIntentPreview(preset("Color", { universal_values: { color: semantic([1, 0, 0]) } }));

	it("uses the automatic preview when nothing was chosen", () => {
		expect(presetTileArtwork({}, undefined, preview)).toEqual({ preview });
		// A cleared icon or an unset colour is not a choice.
		expect(presetTileArtwork({ icon: "★" }, { icon: "", color: null }, preview)).toEqual({ preview });
	});

	it("lets an operator-chosen icon or colour override it, on the button or in the show", () => {
		expect(presetTileArtwork({}, { icon: "★" }, preview)).toEqual({ icon: "★", color: undefined, preview: null });
		expect(presetTileArtwork({}, { color: "#123456" }, preview)).toEqual({
			icon: undefined,
			color: "#123456",
			preview: null,
		});
		expect(presetTileArtwork({ color: "#654321", icon: "●" }, undefined, preview)).toEqual({
			icon: "●",
			color: "#654321",
			preview: null,
		});
	});

	it("keeps the current plain tile for a preset without an intention", () => {
		expect(presetTileArtwork({}, undefined, null)).toEqual({ preview: null });
	});
});
