import type { StoredPreset } from "../../../light-desktop/src/api/types";
import { srgbToXyz } from "../../../light-desktop/src/features/presetPreview/colorDisplay";
import type { PresetCard } from "../../../light-desktop/src/features/presetRecording/presetCards";

/**
 * Stored Color and Position presets whose pool tiles preview their programming intention, beside
 * the explicit and legacy tiles that keep today's icon. Shared by the pool stories and their tests.
 */

type Spread = { component: "hue" | "saturation"; points: number[] };

function semantic(
	rgb: [number, number, number],
	options: { spreads?: Spread[]; whiteBlend?: number; kelvin?: number; uv?: number } = {},
) {
	return {
		kind: "color_program",
		value: {
			kind: "semantic",
			intent: {
				base_xyz: srgbToXyz(...rgb),
				recipe: { version: 1, rgb, amber: 0, approximate: false },
				white_blend: options.whiteBlend ?? 0,
				white_target: { kelvin: options.kelvin ?? 6504, duv: 0 },
				uv: { amount: options.uv ?? 0 },
				relative_output: 1,
				allocation: "preserve_recipe",
				...(options.spreads ? { spreads: options.spreads } : {}),
			},
		},
	};
}

function scalar(value: number | number[]) {
	return Array.isArray(value)
		? { kind: "spread", value }
		: { kind: "value", value };
}

function angles(pan: number | number[], tilt: number | number[]) {
	return {
		kind: "position",
		value: { kind: "angles", pan_degrees: scalar(pan), tilt_degrees: scalar(tilt) },
	};
}

function target(x: number, y: number) {
	return {
		kind: "position",
		value: {
			kind: "target",
			reference: { kind: "origin" },
			offset_metres: [scalar(x), scalar(y), scalar(0)],
		},
	};
}

function perFixture(attribute: string, values: readonly unknown[]) {
	return Object.fromEntries(
		values.map((value, index) => [`fixture-${101 + index}`, { [attribute]: value }]),
	);
}

function preset(
	family: "Color" | "Position",
	number: number,
	name: string,
	body: Partial<StoredPreset>,
): PresetCard {
	return {
		id: `${family === "Color" ? 2 : 3}.${number}`,
		revision: 1,
		body: { name, number, family, values: {}, ...body },
	};
}

export const previewColorPresets: PresetCard[] = [
	preset("Color", 1, "Red", { universal_values: { color: semantic([1, 0, 0]) } }),
	preset("Color", 2, "Red / Blue", {
		values: perFixture("color", [
			semantic([1, 0, 0]),
			semantic([0, 0, 1]),
			semantic([1, 0, 0]),
			semantic([0, 0, 1]),
		]),
	}),
	preset("Color", 3, "Rainbow", {
		universal_values: {
			color: semantic([1, 0, 0], { spreads: [{ component: "hue", points: [0, 120, 240] }] }),
		},
	}),
	preset("Color", 4, "Warm White", {
		universal_values: { color: semantic([1, 1, 1], { whiteBlend: 1, kelvin: 3200 }) },
	}),
	preset("Color", 5, "UV", { universal_values: { color: semantic([0, 0, 0], { uv: 1 }) } }),
	preset("Color", 6, "Chosen Icon", {
		universal_values: { color: semantic([0, 1, 0]) },
		icon: "★",
		color: "#f4b942",
	}),
	preset("Color", 7, "Wheel Red", {
		values: { "fixture-101": { "color.wheel.1": { kind: "discrete", value: "deep_red" } } },
	}),
];

/** Thirty fixtures in a 6 × 5 Pan/Tilt grid: the tile keeps ten representative dots. */
const grid = Array.from({ length: 30 }, (_, index) =>
	angles(-50 + (index % 6) * 20, 10 + Math.floor(index / 6) * 15),
);

export const previewPositionPresets: PresetCard[] = [
	preset("Position", 1, "Fan", {
		values: perFixture("position", [
			angles(-60, 45),
			angles(-30, 45),
			angles(0, 45),
			angles(30, 45),
			angles(60, 45),
		]),
	}),
	preset("Position", 2, "Audience Grid", { values: perFixture("position", grid) }),
	preset("Position", 3, "Centre Stage", { universal_values: { position: target(0, 2) } }),
	preset("Position", 4, "Cross", {
		values: perFixture("position", [
			target(-3, 0),
			target(3, 0),
			target(0, -3),
			target(0, 3),
			target(0, 0),
		]),
	}),
	preset("Position", 5, "Chosen Icon", {
		universal_values: { position: target(0, 0) },
		icon: "◎",
	}),
	preset("Position", 6, "Legacy Pan", {
		values: { "fixture-101": { pan: { kind: "normalized", value: 0.5 } } },
	}),
];
