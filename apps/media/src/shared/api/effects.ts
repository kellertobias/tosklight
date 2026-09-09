export const EFFECT_TYPES = [
	{ value: "analog-tv", label: "TV/CRT/VHS Simulation" },
	{ value: "digital-tv", label: "Digital Video/ Glitch Simulation" },
	{ value: "blur", label: "Blur" },
	{ value: "feedback", label: "Feedback" },
	{ value: "beat-move", label: "Beat Move" },
	{ value: "beat-scan", label: "Beat Scan" },
	{ value: "beat-scale-turn", label: "Beat Scale & Turn" },
	{ value: "beat-form-flash", label: "Beat form Flash" },
	{ value: "kaleidoscope", label: "Kaleidoscope" },
	{ value: "rasterize-bw", label: "B/W Rasterize" },
	{ value: "rasterize-cmyk", label: "CMYK Rasterize" },
	{ value: "drawn-image", label: "Drawn Image Style" },
] as const;

export type EffectType = (typeof EFFECT_TYPES)[number]["value"];
export type EffectLibrarySlot = EffectPresetView;

export interface UpdateEffectLibrarySlot {
	requestId: string;
	name?: string;
	effectType?: string;
	parameters?: number[];
	clear?: boolean;
}
import type { EffectPresetView } from "./generated/media-wire";
