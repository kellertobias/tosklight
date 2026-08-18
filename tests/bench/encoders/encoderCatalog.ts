import type { AttributeValue } from "../../../apps/light-desktop/src/api/types/playback";

export enum EncoderGroup {
	Intensity = "intensity",
	Color = "color",
	Position = "position",
	Beam = "beam",
	Shapers = "shapers",
	Focus = "focus",
	Control = "control",
	Media = "media",
}

export enum IntensityAttribute {
	Dimmer = "dimmer",
	Shutter = "shutter",
	Strobe = "strobe",
	Master = "master",
}

export enum ColorAttribute {
	Red = "red",
	Green = "green",
	Blue = "blue",
	White = "white",
	Amber = "amber",
	Uv = "uv",
}

export enum PositionAttribute {
	Pan = "pan",
	Tilt = "tilt",
}

export enum BeamAttribute {
	Gobo1 = "gobo1",
	Gobo2 = "gobo2",
	GoboRotation = "goboRotation",
	Prism1 = "prism1",
	Prism2 = "prism2",
	Iris = "iris",
}

export enum ShapersAttribute {
	Blade1 = "blade1",
	Blade2 = "blade2",
	Blade3 = "blade3",
	Blade4 = "blade4",
	Rotation = "rotation",
}

export enum FocusAttribute {
	Focus = "focus",
	Zoom = "zoom",
	Frost = "frost",
	Edge = "edge",
}

export enum ControlAttribute {
	Control = "control",
	PlayMode = "playMode",
	PlaybackSpeed = "playbackSpeed",
	PlaybackBpm = "playbackBpm",
	ScalingMode = "scalingMode",
}

export enum MediaAttribute {
	Folder = "folder",
	File = "file",
	MaskFolder = "maskFolder",
	MaskFile = "maskFile",
	MaskInvert = "maskInvert",
}

export enum ProgrammerToken {
	Thru = "THRU",
}

export type ProgrammerExpression = readonly (number | ProgrammerToken)[];

export interface EncoderCatalogEntry {
	group: EncoderGroup;
	familyLabel: string;
	key: string;
	attribute: string;
	label: string;
	normalized: boolean;
}

const entries: readonly EncoderCatalogEntry[] = [
	entry(
		EncoderGroup.Intensity,
		"Intensity",
		IntensityAttribute.Dimmer,
		"intensity",
		"Dimmer",
	),
	entry(
		EncoderGroup.Intensity,
		"Intensity",
		IntensityAttribute.Shutter,
		"shutter",
		"Shutter",
	),
	entry(
		EncoderGroup.Intensity,
		"Intensity",
		IntensityAttribute.Strobe,
		"strobe",
		"Strobe",
	),
	entry(
		EncoderGroup.Intensity,
		"Intensity",
		IntensityAttribute.Master,
		"master",
		"Master",
	),
	entry(EncoderGroup.Color, "Color", ColorAttribute.Red, "color.red", "Red"),
	entry(
		EncoderGroup.Color,
		"Color",
		ColorAttribute.Green,
		"color.green",
		"Green",
	),
	entry(EncoderGroup.Color, "Color", ColorAttribute.Blue, "color.blue", "Blue"),
	entry(
		EncoderGroup.Color,
		"Color",
		ColorAttribute.White,
		"color.white",
		"White",
	),
	entry(
		EncoderGroup.Color,
		"Color",
		ColorAttribute.Amber,
		"color.amber",
		"Amber",
	),
	entry(EncoderGroup.Color, "Color", ColorAttribute.Uv, "color.uv", "UV"),
	entry(EncoderGroup.Position, "Position", PositionAttribute.Pan, "pan", "Pan"),
	entry(
		EncoderGroup.Position,
		"Position",
		PositionAttribute.Tilt,
		"tilt",
		"Tilt",
	),
	entry(EncoderGroup.Beam, "Beam", BeamAttribute.Gobo1, "gobo", "Gobo 1"),
	entry(EncoderGroup.Beam, "Beam", BeamAttribute.Gobo2, "gobo.2", "Gobo 2"),
	entry(
		EncoderGroup.Beam,
		"Beam",
		BeamAttribute.GoboRotation,
		"gobo.rotation",
		"Gobo rotation",
	),
	entry(EncoderGroup.Beam, "Beam", BeamAttribute.Prism1, "prism", "Prism 1"),
	entry(EncoderGroup.Beam, "Beam", BeamAttribute.Prism2, "prism.2", "Prism 2"),
	entry(EncoderGroup.Beam, "Beam", BeamAttribute.Iris, "iris", "Iris"),
	entry(
		EncoderGroup.Shapers,
		"Shapers",
		ShapersAttribute.Blade1,
		"shaper.blade.1.position",
		"Blade 1",
	),
	entry(
		EncoderGroup.Shapers,
		"Shapers",
		ShapersAttribute.Blade2,
		"shaper.blade.2",
		"Blade 2",
	),
	entry(
		EncoderGroup.Shapers,
		"Shapers",
		ShapersAttribute.Blade3,
		"shaper.blade.3",
		"Blade 3",
	),
	entry(
		EncoderGroup.Shapers,
		"Shapers",
		ShapersAttribute.Blade4,
		"shaper.blade.4",
		"Blade 4",
	),
	entry(
		EncoderGroup.Shapers,
		"Shapers",
		ShapersAttribute.Rotation,
		"shaper.rotation",
		"Rotation",
	),
	entry(EncoderGroup.Focus, "Focus", FocusAttribute.Focus, "focus", "Focus"),
	entry(EncoderGroup.Focus, "Focus", FocusAttribute.Zoom, "zoom", "Zoom"),
	entry(EncoderGroup.Focus, "Focus", FocusAttribute.Frost, "frost", "Frost"),
	entry(EncoderGroup.Focus, "Focus", FocusAttribute.Edge, "edge", "Edge"),
	entry(
		EncoderGroup.Control,
		"Control",
		ControlAttribute.Control,
		"control",
		"Control",
		false,
	),
	entry(
		EncoderGroup.Control,
		"Control",
		ControlAttribute.PlayMode,
		"media.play_mode",
		"Play Mode",
		false,
	),
	entry(
		EncoderGroup.Control,
		"Control",
		ControlAttribute.PlaybackSpeed,
		"media.playback_speed",
		"Playback Speed",
	),
	entry(
		EncoderGroup.Control,
		"Control",
		ControlAttribute.PlaybackBpm,
		"media.playback_bpm",
		"Playback BPM",
	),
	entry(
		EncoderGroup.Control,
		"Control",
		ControlAttribute.ScalingMode,
		"media.scaling_mode",
		"Scaling Mode",
		false,
	),
	entry(
		EncoderGroup.Media,
		"Media",
		MediaAttribute.Folder,
		"media.folder",
		"Folder",
		false,
	),
	entry(
		EncoderGroup.Media,
		"Media",
		MediaAttribute.File,
		"media.file",
		"File",
		false,
	),
	entry(
		EncoderGroup.Media,
		"Media",
		MediaAttribute.MaskFolder,
		"media.mask.folder",
		"Mask Folder",
		false,
	),
	entry(
		EncoderGroup.Media,
		"Media",
		MediaAttribute.MaskFile,
		"media.mask.file",
		"Mask File",
		false,
	),
	entry(
		EncoderGroup.Media,
		"Media",
		MediaAttribute.MaskInvert,
		"media.mask.invert",
		"Mask Invert",
		false,
	),
];

export function encoderCatalogEntry(
	group: EncoderGroup,
	key: string,
): EncoderCatalogEntry {
	const found = entries.find(
		(candidate) => candidate.group === group && candidate.key === key,
	);
	if (!found) throw new Error(`Unknown ${group} encoder attribute ${key}`);
	return found;
}

export function normalizedEncoderValue(
	value: number | ProgrammerExpression,
): AttributeValue {
	const tokens = typeof value === "number" ? [value] : [...value];
	if (tokens.length === 0)
		throw new Error("Programmer expression requires at least one value");
	const points: number[] = [];
	for (const [index, token] of tokens.entries()) {
		const expectsNumber = index % 2 === 0;
		if (expectsNumber && typeof token !== "number")
			throw new Error("Programmer expression cannot lead with or repeat THRU");
		if (!expectsNumber && token !== ProgrammerToken.Thru)
			throw new Error("Programmer expression values must be separated by THRU");
		if (typeof token === "number") points.push(normalizedPercentage(token));
	}
	if (tokens.length % 2 === 0)
		throw new Error("Programmer expression cannot end with THRU");
	return points.length === 1
		? { kind: "normalized", value: points[0] }
		: { kind: "spread", value: points };
}

function entry(
	group: EncoderGroup,
	familyLabel: string,
	key: string,
	attribute: string,
	label: string,
	normalized = true,
): EncoderCatalogEntry {
	return { group, familyLabel, key, attribute, label, normalized };
}

function normalizedPercentage(value: number): number {
	if (!Number.isFinite(value) || value < 0 || value > 100)
		throw new Error(`Programmer value ${value} must be between 0 and 100`);
	return value / 100;
}
