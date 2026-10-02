import type {
	FamilyEncoderPagesSnapshot,
	ProgrammingAttributeValue,
	ProgrammingColorComponent,
	ProgrammingComponentDescriptor,
} from "../../../../../api/familyEncoderModels";
import {
	type ColorComponentChange,
	scalarSet,
} from "../../../../../features/programmerValues/familyGestureFamilies";
import type { ValueRange } from "../HorizontalRangeFader";

/**
 * Pure model of the semantic Color Special Dialog (TL-550).
 *
 * The dialog only presents requested values and turns operator gestures into Color component
 * edits; the server owns every per-fixture result (API rule 4). Values travel in descriptor
 * units on the wire and in display units (descriptor × `display_scale`) on the controls.
 */

/** The components the dialog's pickers and faders author. */
export type ColorDialogControl =
	| "hue"
	| "saturation"
	| "white_blend"
	| "temperature"
	| "duv";

export const COLOR_DIALOG_CONTROLS: readonly ColorDialogControl[] = [
	"hue",
	"saturation",
	"white_blend",
	"temperature",
	"duv",
];

/**
 * The compiled core descriptors (`ProgrammingComponent::descriptor`), used only for components
 * the published family pages do not carry (Hue and Saturation have no encoder slot).
 */
const unit = { kind: "bounded", bounds: { min: 0, max: 1 } } as const;
const percent = (
	role: ProgrammingComponentDescriptor["role"],
): ProgrammingComponentDescriptor => ({
	owner: "color",
	role,
	unit: "percent",
	domain: unit,
	step: 0.01,
	fine_step: 0.001,
	display_scale: 100,
	interpolation: "linear",
	capability: "semantic_intent",
	spread: true,
	align: true,
	dynamics: true,
});
export const CORE_COLOR_DESCRIPTORS: Readonly<
	Record<ColorDialogControl, ProgrammingComponentDescriptor>
> = {
	hue: {
		...percent("color_coordinate"),
		unit: "degrees",
		domain: { kind: "cyclic", bounds: { min: 0, max: 360 } },
		step: 1,
		fine_step: 0.1,
		display_scale: 1,
		interpolation: "shortest_arc",
	},
	saturation: percent("color_coordinate"),
	white_blend: percent("color_orthogonal"),
	temperature: {
		...percent("color_orthogonal"),
		unit: "kelvin",
		domain: { kind: "bounded", bounds: { min: 1000, max: 20000 } },
		step: 100,
		fine_step: 10,
		display_scale: 1,
		interpolation: "reciprocal",
	},
	duv: {
		...percent("color_orthogonal"),
		unit: "duv",
		domain: { kind: "bounded", bounds: { min: -0.03, max: 0.03 } },
		step: 0.001,
		fine_step: 0.0001,
		display_scale: 1,
	},
};

export type ColorDescriptors = Readonly<
	Record<ColorDialogControl, ProgrammingComponentDescriptor>
>;

/** Descriptors from the server's Color family pages, completed by the core table. */
export function colorDescriptors(
	snapshot: FamilyEncoderPagesSnapshot | null,
): ColorDescriptors {
	const published = new Map<string, ProgrammingComponentDescriptor>();
	const group = snapshot?.families.find((entry) => entry.family === "color");
	for (const page of group?.pages ?? [])
		for (const slot of page.slots)
			if (slot?.kind === "component" && slot.component.kind === "color")
				published.set(slot.component.component, slot.descriptor);
	const result = { ...CORE_COLOR_DESCRIPTORS };
	for (const control of COLOR_DIALOG_CONTROLS) {
		const descriptor = published.get(control);
		if (descriptor) result[control] = descriptor;
	}
	return result;
}

export interface ColorControlLimits {
	min: number;
	max: number;
	step: number;
}

/** Control limits in display units. A cyclic domain stops one step short of its wrap. */
export function colorControlLimits(
	descriptor: ProgrammingComponentDescriptor,
): ColorControlLimits {
	const scale = descriptor.display_scale || 1;
	const step = descriptor.step * scale;
	const domain = descriptor.domain;
	if (!domain || domain.kind === "finite") return { min: 0, max: 100, step };
	const max = domain.bounds.max * scale;
	return {
		min: domain.bounds.min * scale,
		max: domain.kind === "cyclic" ? max - step : max,
		step,
	};
}

// ---------------------------------------------------------------------------------------------
// Hue travel: shortest arc, an exact half turn goes clockwise (increasing hue)
// ---------------------------------------------------------------------------------------------

/** Signed hue travel from `start` to `end`: the shortest arc, +180 for an exact half turn. */
export function hueTravel(start: number, end: number) {
	let delta = (((end - start) % 360) + 360) % 360;
	if (delta > 180) delta -= 360;
	return delta;
}

/** The hue at `fraction` (0..1) of an ordered hue range, along its shortest arc. */
export function hueAlong(range: ValueRange, fraction: number) {
	const value = range[0] + hueTravel(range[0], range[1]) * fraction;
	return ((value % 360) + 360) % 360;
}

/** Evenly spaced hues along the range's arc, endpoints included (presentation only). */
export function hueRangeSamples(range: ValueRange, count: number) {
	const steps = Math.max(2, Math.floor(count));
	return Array.from({ length: steps }, (_, index) =>
		hueAlong(range, index / (steps - 1)),
	);
}

// ---------------------------------------------------------------------------------------------
// Requested values (read from the Programmer projection; never refitted)
// ---------------------------------------------------------------------------------------------

export interface ColorValueEntry {
	fixtureId: string;
	attribute: string;
	value: ProgrammingAttributeValue;
}

export interface ColorDialogValues {
	/** Display units: degrees, percent, kelvin, Duv. */
	hue: number;
	saturation: number;
	white_blend: number;
	temperature: number;
	duv: number;
	/** Requested UV in percent, `null` when nothing requests UV. */
	uv: number | null;
	ranges: Partial<Record<ColorDialogControl, ValueRange>>;
	/** Requested base colour with White Blend applied, as CSS (presentation only). */
	preview: string;
	/** The selection's requested colours differ; the first is shown. */
	mixed: boolean;
	/** At least one selected owner carries a semantic Color request. */
	programmed: boolean;
}

export const DEFAULT_COLOR_VALUES: ColorDialogValues = {
	hue: 0,
	saturation: 0,
	white_blend: 0,
	temperature: 6500,
	duv: 0,
	uv: null,
	ranges: {},
	preview: "rgb(255, 255, 255)",
	mixed: false,
	programmed: false,
};

type SemanticIntent = Extract<
	Extract<ProgrammingAttributeValue, { kind: "color_program" }>["value"],
	{ kind: "semantic" }
>["intent"];

function rgbHueSaturation(rgb: readonly number[], fallbackHue: number) {
	const max = Math.max(...rgb);
	const min = Math.min(...rgb);
	const delta = max - min;
	if (delta <= 0 || max <= 0) return { hue: fallbackHue, saturation: 0 };
	const [red, green, blue] = rgb;
	const sector =
		max === red
			? (green - blue) / delta
			: max === green
				? (blue - red) / delta + 2
				: (red - green) / delta + 4;
	return { hue: (sector * 60 + 360) % 360, saturation: (delta / max) * 100 };
}

const clamp01 = (value: number) => Math.max(0, Math.min(1, value));
const byte = (value: number) => Math.round(clamp01(value) * 255);

/** The requested base colour, normalised to full level, desaturated by White Blend. */
export function requestedPreview(rgb: readonly number[], whiteBlend: number) {
	const max = Math.max(...rgb, 0);
	const base = max > 0 ? rgb.map((value) => value / max) : [1, 1, 1];
	const blend = clamp01(whiteBlend);
	const [red, green, blue] = base.map((value) => value * (1 - blend) + blend);
	return `rgb(${byte(red)}, ${byte(green)}, ${byte(blue)})`;
}

function semanticIntents(
	entries: readonly ColorValueEntry[],
	fixtureIds: readonly string[],
) {
	const wanted = new Set(fixtureIds);
	return entries.flatMap((entry) =>
		entry.attribute === "color" &&
		wanted.has(entry.fixtureId) &&
		entry.value.kind === "color_program" &&
		entry.value.value.kind === "semantic"
			? [entry.value.value.intent]
			: [],
	);
}

function spreadRange(
	intent: SemanticIntent,
	component: ProgrammingColorComponent,
	scale: number,
): ValueRange | undefined {
	const points = intent.spreads?.find(
		(spread) => spread.component === component,
	)?.points;
	if (!points || points.length < 2) return undefined;
	return [points[0] * scale, points[points.length - 1] * scale];
}

/** One control of one requested intent, in display units (`null`: hue of a colourless request). */
function controlValue(
	intent: SemanticIntent,
	control: ColorDialogControl,
	scale: number,
): number | null {
	if (control === "hue" || control === "saturation") {
		const derived = rgbHueSaturation(intent.recipe.rgb, Number.NaN);
		if (control === "hue") return derived.saturation > 0 ? derived.hue : null;
		return derived.saturation;
	}
	if (control === "white_blend") return intent.white_blend * scale;
	if (control === "temperature") return intent.white_target.kelvin * scale;
	return intent.white_target.duv * scale;
}

/**
 * A fixture-addressed range is stored resolved, one value per fixture (the backend spreads it
 * by rank). When the selection's values of one control are exactly the even steps from the first
 * fixture's value to the last one's, in selection order (Hue along its shortest arc), the dialog
 * shows that ordered range again, so a Shift range reads `80% → 20%` after the Programmer reflects
 * it. Any other difference is no range (the request reads Mixed).
 */
function resolvedRange(
	ordered: readonly SemanticIntent[],
	control: ColorDialogControl,
	scale: number,
): ValueRange | undefined {
	if (ordered.length < 2) return undefined;
	const values = ordered.map((intent) => controlValue(intent, control, scale));
	if (values.some((value) => value === null || !Number.isFinite(value))) return undefined;
	const points = values as number[];
	const first = points[0];
	const last = points[points.length - 1];
	const travel = control === "hue" ? hueTravel(first, last) : last - first;
	const tolerance = Math.max(1e-6, Math.abs(travel) * 1e-3);
	if (Math.abs(travel) <= tolerance) return undefined;
	const steps = points.length - 1;
	const even = points.every((value, index) => {
		const expected = first + (travel * index) / steps;
		const difference =
			control === "hue" ? hueTravel(expected, value) : value - expected;
		return Math.abs(difference) <= tolerance;
	});
	return even ? [first, last] : undefined;
}

/** The selection's semantic intents in selection order, when every owner carries one. */
function orderedIntents(
	entries: readonly ColorValueEntry[],
	fixtureIds: readonly string[],
) {
	const byFixture = new Map<string, SemanticIntent>();
	for (const entry of entries)
		if (
			entry.attribute === "color" &&
			entry.value.kind === "color_program" &&
			entry.value.value.kind === "semantic"
		)
			byFixture.set(entry.fixtureId, entry.value.value.intent);
	const ordered = fixtureIds.map((id) => byFixture.get(id));
	return ordered.every(Boolean) ? (ordered as SemanticIntent[]) : [];
}

/** The selection's requested Color as the dialog shows it. */
export function requestedColorValues(
	entries: readonly ColorValueEntry[],
	fixtureIds: readonly string[],
	descriptors: ColorDescriptors,
	fallbackHue = 0,
): ColorDialogValues {
	const intents = semanticIntents(entries, fixtureIds);
	const intent = intents[0];
	if (!intent) return { ...DEFAULT_COLOR_VALUES, hue: fallbackHue };
	const scale = (control: ColorDialogControl) =>
		descriptors[control].display_scale || 1;
	const ranges: ColorDialogValues["ranges"] = {};
	const ordered = orderedIntents(entries, fixtureIds);
	for (const control of COLOR_DIALOG_CONTROLS) {
		const range =
			spreadRange(intent, control, scale(control)) ??
			resolvedRange(ordered, control, scale(control));
		if (range) ranges[control] = range;
	}
	const derived = rgbHueSaturation(intent.recipe.rgb, fallbackHue);
	const first = JSON.stringify(intent);
	return {
		hue: ranges.hue?.[0] ?? derived.hue,
		saturation: ranges.saturation?.[0] ?? derived.saturation,
		white_blend:
			ranges.white_blend?.[0] ?? intent.white_blend * scale("white_blend"),
		temperature:
			ranges.temperature?.[0] ??
			intent.white_target.kelvin * scale("temperature"),
		duv: ranges.duv?.[0] ?? intent.white_target.duv * scale("duv"),
		uv: intent.uv.amount > 0 ? intent.uv.amount * 100 : null,
		ranges,
		preview: requestedPreview(intent.recipe.rgb, intent.white_blend),
		mixed: intents.some((other) => JSON.stringify(other) !== first),
		programmed: true,
	};
}

// ---------------------------------------------------------------------------------------------
// Gestures → Color component edits
// ---------------------------------------------------------------------------------------------

/** One control's requested value or ordered range as a Color component edit (wire units). */
export function colorComponentChange(
	control: ColorDialogControl,
	value: number,
	range: ValueRange | undefined,
	descriptors: ColorDescriptors,
): ColorComponentChange {
	const scale = descriptors[control].display_scale || 1;
	const toWire = (display: number) => display / scale;
	return {
		component: control,
		operation: range
			? {
					kind: "set",
					value: { kind: "spread", value: [toWire(range[0]), toWire(range[1])] },
				}
			: scalarSet(toWire(value)),
	};
}

/**
 * A shifted contact without an anchor only marks the pending first endpoint: it is shown, not
 * written. Writing starts with the completed `[first, last]` range.
 */
export function isPendingEndpoint(
	shifted: boolean,
	range: ValueRange | undefined,
) {
	return shifted && !range;
}

// ---------------------------------------------------------------------------------------------
// Variant and encoder-area budget
// ---------------------------------------------------------------------------------------------

/** Media heads use White Blend as greyscale over the shared tint; a mix keeps the lamp dialog. */
export function colorDialogVariant(
	selectedFixtureIds: readonly string[],
	mediaFixtureIds: readonly string[],
): "lamp" | "media" {
	if (!selectedFixtureIds.length) return "lamp";
	const media = new Set(mediaFixtureIds);
	return selectedFixtureIds.every((id) => media.has(id)) ? "media" : "lamp";
}

/** The compact Color dialog needs at least this much of the measured lower encoder area. */
export const COMPACT_COLOR_MIN_WIDTH = 680;
export const COMPACT_COLOR_MIN_HEIGHT = 210;

export function encoderAreaFits(size: { width: number; height: number }) {
	return (
		size.width >= COMPACT_COLOR_MIN_WIDTH &&
		size.height >= COMPACT_COLOR_MIN_HEIGHT
	);
}

// ---------------------------------------------------------------------------------------------
// Presentation colours (never sent; the server owns every fixture and Media result)
// ---------------------------------------------------------------------------------------------

/** Full-level RGB (0..1) of a hue in degrees and a saturation in percent. */
export function hueSaturationRgb(hue: number, saturation: number): [number, number, number] {
	const h = ((((hue % 360) + 360) % 360) / 60);
	const s = clamp01(saturation / 100);
	const sector = Math.floor(h) % 6;
	const f = h - Math.floor(h);
	const p = 1 - s;
	const q = 1 - f * s;
	const t = 1 - (1 - f) * s;
	const table: [number, number, number][] = [
		[1, t, p],
		[q, 1, p],
		[p, 1, t],
		[p, q, 1],
		[t, p, 1],
		[1, p, q],
	];
	return table[sector];
}

export function cssRgb(rgb: readonly number[]) {
	return `rgb(${byte(rgb[0])}, ${byte(rgb[1])}, ${byte(rgb[2])})`;
}

const toLinear = (value: number) =>
	value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
const toEncoded = (value: number) =>
	value <= 0.0031308 ? value * 12.92 : 1.055 * value ** (1 / 2.4) - 0.055;

/**
 * One Media source pixel under the shared Color tint and White Blend (TL-569/593 Media rule):
 * White Blend desaturates the source towards its Rec.709 luminance, then the tint multiplies
 * it. Intensity is not part of Color: layer and master Intensity stay independent.
 */
export function mediaPreviewPixel(
	source: readonly number[],
	tint: readonly number[],
	whiteBlend: number,
): [number, number, number] {
	const linear = source.map((value) => toLinear(clamp01(value)));
	const luminance = linear[0] * 0.2126 + linear[1] * 0.7152 + linear[2] * 0.0722;
	const blend = clamp01(whiteBlend);
	const out = linear.map((value, channel) =>
		toEncoded(((1 - blend) * value + blend * luminance) * toLinear(clamp01(tint[channel]))),
	);
	return [out[0], out[1], out[2]];
}
