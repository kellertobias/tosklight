import type { FixtureProfile } from "../../wire";

/**
 * The unit and the resolution each authored physical figure is kept in.
 *
 * A manufacturer prints a body in whole millimetres, a weight to ten grams and a power draw in
 * whole watts; more digits than that are transcription noise, not information. The editor rejects
 * them rather than rounding, so an operator sees exactly what the profile will store.
 */
export const PHYSICAL_PRECISION = [
	{ key: "width_millimetres", label: "Width", unit: "mm", decimals: 0 },
	{ key: "height_millimetres", label: "Height", unit: "mm", decimals: 0 },
	{ key: "depth_millimetres", label: "Depth", unit: "mm", decimals: 0 },
	{ key: "weight_kilograms", label: "Weight", unit: "kg", decimals: 2 },
	{
		key: "power_watts",
		label: "Power consumption",
		unit: "W",
		decimals: 0,
	},
] as const;

/** Sharpness and uniformity are stored as `0..1` and authored as a percentage to one decimal. */
export const OPTICS_PERCENT_PRECISION = [
	{ key: "sharpness", label: "Sharpness" },
	{ key: "uniformity", label: "Uniformity" },
] as const;

export const OPTICS_PERCENT_DECIMALS = 1;

/** A stored fraction at one percent decimal: tested on the stored figure, where float noise is. */
function fractionHasPercentPrecision(value: number) {
	return hasAtMostDecimals(value, OPTICS_PERCENT_DECIMALS + 2);
}

/**
 * Whether `value` carries no more than `decimals` places, tolerating binary float noise.
 *
 * The desk stores these figures as 32-bit floats, so a value widened from one (`1.98` read as
 * `1.9800000190734863`) still counts as the figure that was authored.
 */
export function hasAtMostDecimals(value: number, decimals: number) {
	const scale = 10 ** decimals;
	const scaled = value * scale;
	const rounded = Math.round(scaled);
	return (
		Math.abs(scaled - rounded) < 1e-6 ||
		Math.fround(value) === Math.fround(rounded / scale)
	);
}

/** The operator-facing reason a figure is not stored at its precision, or `null` when it is. */
export function precisionMessage(
	label: string,
	unit: string,
	decimals: number,
	value: number | null | undefined,
) {
	if (value === null || value === undefined || !Number.isFinite(value))
		return null;
	if (hasAtMostDecimals(value, decimals)) return null;
	return decimals === 0
		? `${label} must be a whole number of ${unitName(unit)}`
		: `${label} allows at most ${decimals} decimal ${decimals === 1 ? "place" : "places"} (${unit})`;
}

function unitName(unit: string) {
	if (unit === "mm") return "millimetres";
	if (unit === "W") return "watts";
	return unit;
}

/**
 * A stored `0..1` figure as the percentage an operator reads: always one decimal place.
 *
 * A figure stored beyond that precision is shown as it is, so the value named as wrong is the
 * value on screen rather than a rounded one that would look acceptable.
 */
export function percentText(value: number | null | undefined) {
	if (value === null || value === undefined) return "";
	const percent = value * 100;
	return fractionHasPercentPrecision(value)
		? percent.toFixed(OPTICS_PERCENT_DECIMALS)
		: String(Number(percent.toPrecision(12)));
}

/**
 * A typed percentage as the stored `0..1` figure, clamped to the field's bounds.
 *
 * A figure typed within precision is snapped to it, so `28.9` is kept as `0.289` and not the
 * binary `0.28900000000000003`. A figure typed beyond it is kept unrounded, so validation can
 * name it instead of the editor silently changing it.
 */
export function percentFraction(typed: string) {
	if (typed === "") return null;
	const percent = Math.min(Math.max(Number(typed), 0), 100);
	if (!Number.isFinite(percent)) return null;
	if (!hasAtMostDecimals(percent, OPTICS_PERCENT_DECIMALS)) return percent / 100;
	const scale = 10 ** OPTICS_PERCENT_DECIMALS;
	return Math.round(percent * scale) / (100 * scale);
}

export function opticsPercentMessage(
	label: string,
	value: number | null | undefined,
) {
	if (value === null || value === undefined) return null;
	return fractionHasPercentPrecision(value)
		? null
		: `${label} must be a percentage with one decimal place`;
}

/** Every precision problem in a profile's authored physical and optical figures. */
export function validatePrecision(profile: FixtureProfile, errors: string[]) {
	for (const { key, label, unit, decimals } of PHYSICAL_PRECISION) {
		const message = precisionMessage(
			label,
			unit,
			decimals,
			profile.physical[key],
		);
		if (message) errors.push(message);
	}
	for (const { key, label } of OPTICS_PERCENT_PRECISION) {
		const message = opticsPercentMessage(label, profile.optics?.[key]);
		if (message) errors.push(message);
	}
}
