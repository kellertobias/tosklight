import type {
	ChannelFunction,
	ColorWheelSlot,
	FixtureChannel,
	XyzValue,
} from "../../wire";

/**
 * Named filter colours as sRGB, matched word by word in the order a name spells them. This mirrors
 * `nominal_wheel_srgb` in the fixture crate, which the simulator uses for an unmeasured slot, so the
 * editor previews exactly the colour the Visualizer will show.
 */
const NAMED_COLORS: ReadonlyArray<readonly [string, [number, number, number]]> =
	[
		["open", [1, 1, 1]],
		["white", [1, 1, 1]],
		["clear", [1, 1, 1]],
		["cto", [1, 0.78, 0.55]],
		["ctb", [0.78, 0.87, 1]],
		["ctc", [1, 0.9, 0.8]],
		["congo", [0.25, 0, 0.6]],
		["uv", [0.35, 0, 0.8]],
		["red", [1, 0, 0]],
		["fire", [1, 0.3, 0]],
		["orange", [1, 0.5, 0]],
		["amber", [1, 0.72, 0]],
		["gold", [1, 0.82, 0.3]],
		["straw", [1, 0.9, 0.6]],
		["yellow", [1, 1, 0]],
		["lime", [0.6, 1, 0]],
		["green", [0, 1, 0]],
		["aquamarine", [0.4, 1, 0.8]],
		["teal", [0, 0.6, 0.6]],
		["cyan", [0, 1, 1]],
		["blue", [0, 0.2, 1]],
		["lavender", [0.7, 0.55, 1]],
		["lilac", [0.8, 0.6, 1]],
		["mauve", [0.8, 0.5, 0.8]],
		["purple", [0.5, 0, 1]],
		["violet", [0.55, 0, 1]],
		["magenta", [1, 0, 1]],
		["pink", [1, 0.45, 0.7]],
		["rose", [1, 0.5, 0.6]],
	];

/** The nominal sRGB colour a wheel-slot name describes, or null when no word of it names one. */
export function nominalWheelSrgb(name: string): [number, number, number] | null {
	const words = name
		.toLowerCase()
		.split(/[^a-z0-9]+/)
		.filter(Boolean);
	let color: [number, number, number] | undefined;
	for (const word of words) {
		color = NAMED_COLORS.find(([candidate]) => candidate === word)?.[1];
		if (color) break;
	}
	if (!color) return null;
	const white = words.some((word) => ["pale", "light", "tint"].includes(word));
	const dark = words.some((word) => ["deep", "dark"].includes(word));
	return color.map((channel) =>
		white ? channel + (1 - channel) * 0.5 : dark ? channel * 0.7 : channel,
	) as [number, number, number];
}

const linear = (value: number) => {
	const clamped = Math.max(0, Math.min(1, value));
	return clamped <= 0.04045
		? clamped / 12.92
		: ((clamped + 0.055) / 1.055) ** 2.4;
};

const encoded = (value: number) => {
	const clamped = Math.max(0, Math.min(1, value));
	return clamped <= 0.0031308
		? clamped * 12.92
		: 1.055 * clamped ** (1 / 2.4) - 0.055;
};

const round = (value: number) => Math.round(value * 10_000) / 10_000;

export function srgbToXyz(red: number, green: number, blue: number): XyzValue {
	const [r, g, b] = [linear(red), linear(green), linear(blue)];
	return {
		x: round(0.4124564 * r + 0.3575761 * g + 0.1804375 * b),
		y: round(0.2126729 * r + 0.7151522 * g + 0.072175 * b),
		z: round(0.0193339 * r + 0.119192 * g + 0.9503041 * b),
	};
}

export function hexToXyz(hex: string): XyzValue {
	const value = Number.parseInt(hex.replace("#", ""), 16);
	return srgbToXyz(
		((value >> 16) & 255) / 255,
		((value >> 8) & 255) / 255,
		(value & 255) / 255,
	);
}

/** An XYZ colour as a display hex, scaled so its brightest channel is full. */
export function xyzToHex(xyz: XyzValue): string {
	const r = 3.2404542 * xyz.x - 1.5371385 * xyz.y - 0.4985314 * xyz.z;
	const g = -0.969266 * xyz.x + 1.8760108 * xyz.y + 0.041556 * xyz.z;
	const b = 0.0556434 * xyz.x - 0.2040259 * xyz.y + 1.0572252 * xyz.z;
	const channels = [r, g, b].map((value) => Math.max(0, value));
	const peak = Math.max(...channels);
	const scale = peak > 1 ? 1 / peak : 1;
	return `#${channels
		.map((value) =>
			Math.round(encoded(value * scale) * 255)
				.toString(16)
				.padStart(2, "0"),
		)
		.join("")}`;
}

/** The colour a slot shows in the simulator: its measurement, else the colour its name describes. */
export function wheelSlotDisplayXyz(slot: ColorWheelSlot): XyzValue | null {
	if (slot.measured_xyz) return slot.measured_xyz;
	const rgb = nominalWheelSrgb(slot.label) ?? nominalWheelSrgb(slot.semantic_id);
	return rgb ? srgbToXyz(...rgb) : null;
}

function portableId(fn: ChannelFunction) {
	const behavior = fn.behavior;
	const source =
		behavior.type === "fixed" || behavior.type === "indexed"
			? behavior.semantic_id || behavior.label || fn.name
			: fn.name;
	return source
		.trim()
		.toLowerCase()
		.replaceAll(/[^a-z0-9]+/g, "_")
		.replaceAll(/^_+|_+$/g, "");
}

/**
 * One wheel slot for every fixed or indexed function of the wheel channel, in DMX order, keeping
 * any colour already defined for a slot of the same portable ID. Rotation and other continuous
 * ranges are not colours and are skipped.
 */
export function wheelSlotsFromChannel(
	channel: FixtureChannel,
	existing: ColorWheelSlot[],
): ColorWheelSlot[] {
	const seen = new Set<string>();
	return [...channel.functions]
		.filter(
			(fn) => fn.behavior.type === "fixed" || fn.behavior.type === "indexed",
		)
		.sort((left, right) => left.dmx_from - right.dmx_from)
		.flatMap((fn) => {
			const semantic_id = portableId(fn);
			if (!semantic_id || seen.has(semantic_id)) return [];
			seen.add(semantic_id);
			const label =
				(fn.behavior.type === "fixed" || fn.behavior.type === "indexed"
					? fn.behavior.label
					: "") || fn.name;
			const previous = existing.find(
				(slot) => slot.semantic_id === semantic_id,
			);
			return [
				{
					semantic_id,
					label,
					dmx_from: fn.dmx_from,
					dmx_to: fn.dmx_to,
					measured_xyz: previous?.measured_xyz ?? null,
				},
			];
		});
}
