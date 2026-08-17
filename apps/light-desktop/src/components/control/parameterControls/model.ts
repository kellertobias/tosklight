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
		"shaper.blade.1.position",
		"shaper.blade.1.angle",
		"shaper.blade.2.position",
		"shaper.blade.2.angle",
		"shaper.blade.3.position",
		"shaper.blade.3.angle",
		"shaper.blade.4.position",
		"shaper.blade.4.angle",
		"shaper.rotation",
	],
	Focus: ["focus", "zoom", "frost", "edge"],
	Control: [
		"control",
		"media.play_mode",
		"media.playback_speed",
		"media.playback_bpm",
		"media.playback.blur",
		"media.scaling_mode",
	],
	Media: [
		"media.folder",
		"media.file",
		"audio.folder",
		"audio.file",
		"audio.transport",
		"audio.repeat",
		"audio.volume",
		"media.mask.folder",
		"media.mask.file",
		"media.mask.invert",
		"media.flip_mirror",
		"media.mask.scale.x",
		"media.mask.scale.y",
		"media.mask.position.x",
		"media.mask.position.y",
	],
} as const;

export type ParameterFamily = keyof typeof parameterFamilies;
export const parameterFamilyOrder = Object.keys(
	parameterFamilies,
) as ParameterFamily[];
export type SpecialParameterFamily =
	| "Color"
	| "Position"
	| "Shapers"
	| "Control"
	| "Media";
export const alignModes = ["left", "right", "out", "in"] as const;
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
	"media.playback.blur": "Blur",
	"media.scaling_mode": "Scaling Mode",
	"media.folder": "Media Folder",
	"media.file": "Media File",
	"audio.folder": "Audio Folder",
	"audio.file": "Audio File",
	"audio.transport": "Audio Transport",
	"audio.repeat": "Audio Repeat",
	"audio.volume": "Audio Volume",
	"media.mask.folder": "Mask Folder",
	"media.mask.file": "Mask File",
	"media.mask.invert": "Invert Mask",
	"media.flip_mirror": "Flip / Mirror",
	"media.mask.scale.x": "Mask Scale X",
	"media.mask.scale.y": "Mask Scale Y",
	"media.mask.position.x": "Mask Position X",
	"media.mask.position.y": "Mask Position Y",
	"shaper.blade.1.position": "Blade 1 Position",
	"shaper.blade.1.angle": "Blade 1 Angle",
	"shaper.blade.2.position": "Blade 2 Position",
	"shaper.blade.2.angle": "Blade 2 Angle",
	"shaper.blade.3.position": "Blade 3 Position",
	"shaper.blade.3.angle": "Blade 3 Angle",
	"shaper.blade.4.position": "Blade 4 Position",
	"shaper.blade.4.angle": "Blade 4 Angle",
	"shaper.rotation": "Shaper Rotation",
};

export const specialParameterFamilies = new Set<SpecialParameterFamily>([
	"Color",
	"Position",
	"Shapers",
	"Control",
	"Media",
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
