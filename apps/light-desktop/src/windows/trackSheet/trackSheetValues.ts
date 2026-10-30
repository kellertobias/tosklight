import type { AttributeValue, PresetValueReference } from "../../api/types/playback";
import { colorValueColors } from "../../features/presetPreview/colorDisplay";
import { fixtureSheetColorLabel } from "../fixtureSheetColorLabel";
import type { TrackCellState } from "./trackSheetModel";

export interface PresetName {
	name: string;
	family?: string;
	number: number;
}

const scalar = (value: { kind: "value"; value: number } | { kind: "spread"; value: number[] }) =>
	value.kind === "value" ? `${Math.round(value.value * 10) / 10}°` : "Spread";

/** The operator's intent for a stored value: Preset name first, then the value as programmed. */
export function trackValueText(
	value: AttributeValue | null,
	preset: PresetValueReference | null,
	presets: ReadonlyMap<string, PresetName>,
): string {
	const source = preset ? presets.get(preset.preset_instance_id) : undefined;
	if (source) return source.name || `${source.family ?? "Preset"} ${source.number}`;
	if (preset) return "Missing Preset";
	if (value === null) return "Release";
	switch (value.kind) {
		case "normalized":
			return `${Math.round(value.value * 100)}%`;
		case "discrete":
			return value.value;
		case "spread":
			return "Spread";
		case "position":
			return value.value.kind === "angles"
				? `P ${scalar(value.value.pan_degrees)} T ${scalar(value.value.tilt_degrees)}`
				: value.value.reference.kind === "point"
					? "Target point"
					: "Target origin";
		case "zoom":
			return value.value.opening_degrees.kind === "value"
				? `${Math.round(value.value.opening_degrees.value * 10) / 10}°`
				: "Spread";
		case "color_program":
		case "color_xyz":
			return fixtureSheetColorLabel(value) ?? "Color";
		case "group_family":
			return "Group values";
		case "raw_dmx":
		case "raw_dmx_exact":
			return "Raw DMX";
	}
}

/** Swatch colour for a stored colour value, or null for anything else. */
export function trackValueSwatch(value: AttributeValue | null): string | null {
	if (!value || (value.kind !== "color_program" && value.kind !== "color_xyz")) return null;
	return colorValueColors(value).find((preview) => preview.hex)?.hex ?? null;
}

export const TRACK_STATE_LABELS: Record<TrackCellState, string> = {
	hard: "New value",
	up: "Moves up",
	down: "Moves down",
	block: "Blocked",
	released: "Released",
	restore: "Cue-only restore",
	tracked: "Tracked",
	empty: "Not in this Cuelist",
};

/** A one-glyph marker so state reads without colour. */
export const TRACK_STATE_MARKS: Record<TrackCellState, string> = {
	hard: "",
	up: "▲",
	down: "▼",
	block: "■",
	released: "×",
	restore: "↺",
	tracked: "",
	empty: "",
};
