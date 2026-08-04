export const parameterFamilies = {
	Intensity: ["intensity", "shutter", "master"],
	Color: [
		"color.red",
		"color.green",
		"color.blue",
		"color.white",
		"color.amber",
		"color.uv",
	],
	Position: ["pan", "tilt"],
	Beam: ["gobo", "gobo.2", "gobo.rotation", "prism", "prism.2", "iris"],
	Shapers: [
		"shaper.blade.1",
		"shaper.blade.2",
		"shaper.blade.3",
		"shaper.blade.4",
		"shaper.rotation",
	],
	Focus: ["focus", "zoom", "frost", "edge"],
	Control: [
		"control",
		"media.play_mode",
		"media.playback_speed",
		"media.playback_bpm",
		"media.scaling_mode",
	],
	Media: [
		"media.folder",
		"media.file",
		"media.mask.folder",
		"media.mask.file",
		"media.mask.invert",
	],
} as const;

export type ParameterFamily = keyof typeof parameterFamilies;
export const parameterFamilyOrder = Object.keys(
	parameterFamilies,
) as ParameterFamily[];
export type SpecialParameterFamily =
	| "Color"
	| "Position"
	| "Beam"
	| "Shapers"
	| "Control";
export const alignModes = ["out", "center", "left", "right"] as const;
export type AlignMode = (typeof alignModes)[number];

export const compactFamilyLabels: Record<ParameterFamily, string> = {
	Intensity: "Int",
	Color: "Col",
	Position: "Pos",
	Beam: "Beam",
	Shapers: "Shapr",
	Focus: "Focus",
	Control: "Ctrl",
	Media: "Media",
};

export const parameterLabels: Record<string, string> = {
	intensity: "Dimmer",
	shutter: "Shutter / Strobe",
	master: "Master",
	pan: "Pan",
	tilt: "Tilt",
	gobo: "Gobo 1",
	"gobo.2": "Gobo 2",
	"gobo.rotation": "Gobo rotation",
	prism: "Prism 1",
	"prism.2": "Prism 2",
	iris: "Iris",
	focus: "Focus",
	zoom: "Zoom",
	frost: "Frost",
	edge: "Edge",
	control: "Control",
	"media.play_mode": "Play Mode",
	"media.playback_speed": "Playback Speed",
	"media.playback_bpm": "Playback BPM",
	"media.scaling_mode": "Scaling Mode",
	"media.folder": "Media Folder",
	"media.file": "Media File",
	"media.mask.folder": "Mask Folder",
	"media.mask.file": "Mask File",
	"media.mask.invert": "Invert Mask",
};

export const specialParameterFamilies = new Set<SpecialParameterFamily>([
	"Color",
	"Position",
	"Beam",
	"Shapers",
	"Control",
]);

export function normalizedProgrammerTarget(value: unknown): number | undefined {
	if (typeof value === "number") return value;
	if (!value || typeof value !== "object") return undefined;
	const record = value as Record<string, unknown>;
	if (record.kind === "normalized" && typeof record.value === "number")
		return record.value;
	return record.value === value
		? undefined
		: normalizedProgrammerTarget(record.value);
}

export function discreteProgrammerTarget(value: unknown): string | undefined {
	if (!value || typeof value !== "object") return undefined;
	const record = value as Record<string, unknown>;
	if (record.kind === "discrete" && typeof record.value === "string")
		return record.value;
	return record.value === value
		? undefined
		: discreteProgrammerTarget(record.value);
}

export function formatNormalizedValue(value: number): string {
	return `${Math.round(value * 100)}%`;
}

export function formatNormalizedRange(values: number[]): string | undefined {
	if (!values.length) return undefined;
	const rounded = values.map((value) => Math.round(value * 100));
	const minimum = Math.min(...rounded);
	const maximum = Math.max(...rounded);
	return minimum === maximum ? `${minimum}%` : `${minimum}%...${maximum}%`;
}

export function formatDiscreteValues(values: string[]): string | undefined {
	if (!values.length) return undefined;
	const unique = [...new Set(values)];
	return unique.length === 1 ? unique[0] : "Mixed";
}
