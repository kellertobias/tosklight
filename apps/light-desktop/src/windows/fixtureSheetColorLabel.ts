import type { AttributeValue } from "../api/types/playback";
import { colorValueColors } from "../features/presetPreview/colorDisplay";

/** Display vocabulary, independent of show Presets and fixture color calibration. */
const KNOWN_COLORS = [
	["Black", 0, 0, 0], ["White", 255, 255, 255],
	["Red", 255, 0, 0], ["Orange", 255, 128, 0],
	["Amber", 255, 191, 0], ["Yellow", 255, 255, 0],
	["Lime", 128, 255, 0], ["Green", 0, 255, 0],
	["Cyan", 0, 255, 255], ["Blue", 0, 0, 255],
	["Violet", 128, 0, 255], ["Magenta", 255, 0, 255],
	["Pink", 255, 128, 191],
] as const;

/** A concise estimate of the requested appearance; never evidence of a Preset recall. */
function nearestKnownColor(hex: string) {
	const rgb = [1, 3, 5].map((offset) => Number.parseInt(hex.slice(offset, offset + 2), 16));
	const distance = (color: (typeof KNOWN_COLORS)[number]) =>
		rgb.reduce((sum, channel, index) => sum + (channel - (color[index + 1] as number)) ** 2, 0);
	return KNOWN_COLORS.reduce((closest, color) =>
		distance(color) < distance(closest) ? color : closest,
	)[0];
}

/** Actual provenance wins. Otherwise angle brackets explicitly mark an approximate color name. */
export function fixtureSheetColorLabel(
	value: AttributeValue,
	presetName?: string | null,
): string | null {
	if (presetName?.trim()) return presetName;
	const labels: string[] = [];
	for (const preview of colorValueColors(value)) {
		if (!preview.hex) return null;
		const label = preview.uv ? "UV-only" : nearestKnownColor(preview.hex);
		if (labels.at(-1) !== label) labels.push(label);
	}
	return labels.length ? labels.map((label) => `<${label}>`).join(" thru ") : null;
}
