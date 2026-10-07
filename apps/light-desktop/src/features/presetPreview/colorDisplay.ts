import type { AttributeValue } from "../../api/types/playback";

/**
 * Display colours of stored Color programs, for pool previews only.
 *
 * Mirrors the requested-colour arithmetic of the semantic engine (`light_core` virtual recipe,
 * White Blend envelope and Krystek white target) so a preview shows the colour the intent asks
 * for, never a fixture's achievable output. Intensity is a separate family, so the displayed
 * colour is normalized to its brightest channel: a dim blue reads as blue, and only a requested
 * black reads as black.
 */

type ColorProgram = Extract<AttributeValue, { kind: "color_program" }>["value"];
type ColorIntent = Extract<ColorProgram, { kind: "semantic" }>["intent"];
type Xyz = { x: number; y: number; z: number };
type ColorComponent = NonNullable<ColorIntent["spreads"]>[number]["component"];

/** One colour a preview can show. `null` hex means the stored program has no known appearance. */
export interface PreviewColor {
	hex: string | null;
	/** Visible black that still requests UV. */
	uv?: boolean;
}

/** Ultraviolet has no visible appearance; desks conventionally draw it as deep violet. */
export const UV_PREVIEW_HEX = "#6a2bd9";

function srgbToLinear(value: number) {
	const clamped = Math.min(1, Math.max(0, value));
	return clamped <= 0.04045
		? clamped / 12.92
		: ((clamped + 0.055) / 1.055) ** 2.4;
}

function linearToSrgb(value: number) {
	const clamped = Math.min(1, Math.max(0, value));
	return clamped <= 0.0031308
		? 12.92 * clamped
		: 1.055 * clamped ** (1 / 2.4) - 0.055;
}

/** Encoded sRGB (0–1) to XYZ, as `light_core::srgb_to_xyz`. */
export function srgbToXyz(red: number, green: number, blue: number): Xyz {
	const [r, g, b] = [srgbToLinear(red), srgbToLinear(green), srgbToLinear(blue)];
	return {
		x: 0.4124564 * r + 0.3575761 * g + 0.1804375 * b,
		y: 0.2126729 * r + 0.7151522 * g + 0.072175 * b,
		z: 0.0193339 * r + 0.119192 * g + 0.9503041 * b,
	};
}

/** Brightness-normalized display hex of an XYZ colour; black stays black. */
export function xyzDisplayHex({ x, y, z }: Xyz): string {
	const linear = [
		3.2404542 * x - 1.5371385 * y - 0.4985314 * z,
		-0.969266 * x + 1.8760108 * y + 0.041556 * z,
		0.0556434 * x - 0.2040259 * y + 1.0572252 * z,
	].map((channel) => (Number.isFinite(channel) ? Math.max(0, channel) : 0));
	const peak = Math.max(...linear);
	if (!(peak > 1e-6)) return "#000000";
	return `#${linear
		.map((channel) =>
			Math.round(linearToSrgb(channel / peak) * 255)
				.toString(16)
				.padStart(2, "0"),
		)
		.join("")}`;
}

/** Krystek (1985) Planckian locus in CIE 1960 (u, v), as the engine's white target. */
function planckianUv(kelvin: number): [number, number] {
	const t = kelvin;
	return [
		(0.860117757 + 1.54118254e-4 * t + 1.28641212e-7 * t * t) /
			(1 + 8.42420235e-4 * t + 7.08145163e-7 * t * t),
		(0.317398726 + 4.22806245e-5 * t + 4.20481691e-8 * t * t) /
			(1 - 2.89741816e-5 * t + 1.61456053e-7 * t * t),
	];
}

/** White target chromaticity at Y = 1; positive Duv lies above the locus. */
export function whiteTargetXyz(kelvin: number, duv: number): Xyz {
	const k = Math.min(20000, Math.max(1000, kelvin));
	const [u0, v0] = planckianUv(k);
	const [ua, va] = planckianUv(k * 0.999);
	const [ub, vb] = planckianUv(k * 1.001);
	const length = Math.max(Math.hypot(ub - ua, vb - va), Number.MIN_VALUE);
	let [nu, nv] = [-(vb - va) / length, (ub - ua) / length];
	if (nv < 0) [nu, nv] = [-nu, -nv];
	const [u, v] = [u0 + duv * nu, v0 + duv * nv];
	const denominator = 2 * u - 8 * v + 4;
	const cx = (3 * u) / denominator;
	const cy = (2 * v) / denominator;
	return { x: cx / cy, y: 1, z: (1 - cx - cy) / cy };
}

/** The visible colour an intent requests: `relativeOutput × (colored × base + white × target)`. */
export function requestedVisibleXyz(intent: ColorIntent): Xyz {
	const white = whiteTargetXyz(intent.white_target.kelvin, intent.white_target.duv);
	const colored = Math.min(1, 2 * (1 - intent.white_blend));
	const whiteShare = Math.min(1, 2 * intent.white_blend);
	const mix = (base: number, target: number) =>
		intent.relative_output * (colored * base + whiteShare * target);
	return {
		x: mix(intent.base_xyz.x, white.x),
		y: mix(intent.base_xyz.y, white.y),
		z: mix(intent.base_xyz.z, white.z),
	};
}

function rgbToHsv(red: number, green: number, blue: number) {
	const maximum = Math.max(red, green, blue);
	const delta = maximum - Math.min(red, green, blue);
	const saturation = maximum === 0 ? 0 : delta / maximum;
	let hue = 0;
	if (delta !== 0) {
		if (maximum === red) hue = ((((green - blue) / delta) % 6) + 6) % 6;
		else if (maximum === green) hue = (blue - red) / delta + 2;
		else hue = (red - green) / delta + 4;
		hue /= 6;
	}
	return { hue, saturation, brightness: maximum };
}

function hsvToRgb(hue: number, saturation: number, brightness: number) {
	const i = Math.floor(hue * 6);
	const f = hue * 6 - i;
	const p = brightness * (1 - saturation);
	const q = brightness * (1 - f * saturation);
	const t = brightness * (1 - (1 - f) * saturation);
	switch (((i % 6) + 6) % 6) {
		case 0:
			return [brightness, t, p] as const;
		case 1:
			return [q, brightness, p] as const;
		case 2:
			return [p, brightness, t] as const;
		case 3:
			return [p, q, brightness] as const;
		case 4:
			return [t, p, brightness] as const;
		default:
			return [brightness, p, q] as const;
	}
}

const AMBER_XYZ = srgbToXyz(1, 0.5, 0);

/** Applies one sampled component the way the virtual authoring engine edits its base. */
function applyComponent(intent: ColorIntent, component: ColorComponent, value: number): ColorIntent {
	const recipe = { ...intent.recipe, rgb: [...intent.recipe.rgb] as [number, number, number] };
	const rebase = () => {
		const rgb = srgbToXyz(...recipe.rgb);
		return {
			...intent,
			recipe,
			base_xyz: {
				x: rgb.x + recipe.amber * AMBER_XYZ.x,
				y: rgb.y + recipe.amber * AMBER_XYZ.y,
				z: rgb.z + recipe.amber * AMBER_XYZ.z,
			},
		};
	};
	switch (component) {
		case "red":
		case "green":
		case "blue":
			recipe.rgb[["red", "green", "blue"].indexOf(component)] = value;
			return rebase();
		case "amber":
			recipe.amber = value;
			return rebase();
		case "hue":
		case "saturation": {
			const hsv = rgbToHsv(...recipe.rgb);
			recipe.rgb = [
				...hsvToRgb(
					component === "hue" ? value / 360 : hsv.hue,
					component === "saturation" ? value : hsv.saturation,
					hsv.brightness,
				),
			];
			return rebase();
		}
		case "white_blend":
			return { ...intent, white_blend: value };
		case "temperature":
			return { ...intent, white_target: { ...intent.white_target, kelvin: value } };
		case "duv":
			return { ...intent, white_target: { ...intent.white_target, duv: value } };
		case "relative_output":
			return { ...intent, relative_output: value };
		case "uv":
			return { ...intent, uv: { amount: value } };
	}
}

/** Linear interpolation over equally spaced control points; hue takes the shortest arc. */
export function sampleSpread(points: readonly number[], t: number, circular = false) {
	if (points.length === 0) return 0;
	if (points.length === 1) return points[0];
	const position = Math.min(1, Math.max(0, t)) * (points.length - 1);
	const left = Math.min(points.length - 2, Math.floor(position));
	const fraction = position - left;
	const from = points[left];
	let to = points[left + 1];
	if (circular) {
		let delta = (((to - from) % 360) + 360) % 360;
		// An exact 180° tie resolves clockwise, as the engine does.
		if (delta > 180) delta -= 360;
		to = from + delta;
		return (((from + (to - from) * fraction) % 360) + 360) % 360;
	}
	return from + (to - from) * fraction;
}

/** Sample positions that include every control point and the midpoint between neighbours. */
export function spreadSamplePositions(pointCount: number) {
	const steps = Math.max(1, (Math.max(2, pointCount) - 1) * 2);
	return Array.from({ length: steps + 1 }, (_, index) => index / steps);
}

function semanticIntentColor(intent: ColorIntent): PreviewColor {
	const visible = requestedVisibleXyz(intent);
	const black = !(Math.max(visible.x, visible.y, visible.z) > 1e-6);
	if (black && intent.uv.amount > 0) return { hex: UV_PREVIEW_HEX, uv: true };
	return { hex: xyzDisplayHex(visible) };
}

/**
 * The local intents a semantic program resolves to along its spread. Saturation applies before
 * hue so an achromatic base can still take the requested hue, as the engine orders it.
 */
export function semanticSpreadIntents(intent: ColorIntent, positions?: readonly number[]) {
	const spreads = [...(intent.spreads ?? [])].sort(
		(left, right) => Number(left.component === "hue") - Number(right.component === "hue"),
	);
	if (spreads.length === 0) return [intent];
	const samples =
		positions ?? spreadSamplePositions(Math.max(...spreads.map((spread) => spread.points.length)));
	const seed: ColorIntent = { ...intent, spreads: [] };
	return samples.map((t) =>
		spreads.reduce(
			(local, spread) =>
				applyComponent(local, spread.component, sampleSpread(spread.points, t, spread.component === "hue")),
			seed,
		),
	);
}

/**
 * Every colour one stored colour value shows. `positions` places the samples of a spread, for
 * example one per Group member; without it a spread is sampled at its control points and between
 * them.
 */
export function colorValueColors(
	value: AttributeValue,
	positions?: readonly number[],
): PreviewColor[] {
	switch (value.kind) {
		case "color_xyz":
			return [{ hex: xyzDisplayHex(value.value) }];
		case "color_program": {
			const program = value.value;
			if (program.kind === "semantic")
				return semanticSpreadIntents(program.intent, positions).map(semanticIntentColor);
			const visible = program.portable.visible;
			// A Direct spread's per-rank appearance needs the source model the desk keeps server-side.
			if (!visible || (program.recipe.spreads?.length ?? 0) > 0) return [{ hex: null }];
			const xyz = {
				x: visible.xyz.x * visible.relative_output,
				y: visible.xyz.y * visible.relative_output,
				z: visible.xyz.z * visible.relative_output,
			};
			const black = !(Math.max(xyz.x, xyz.y, xyz.z) > 1e-6);
			if (black && (program.portable.uv?.amount ?? 0) > 0) return [{ hex: UV_PREVIEW_HEX, uv: true }];
			return [{ hex: xyzDisplayHex(xyz) }];
		}
		default:
			return [];
	}
}
