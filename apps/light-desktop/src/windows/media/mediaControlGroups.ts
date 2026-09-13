/**
 * Controls that exist in only some Media Server channel layouts: the effect-bank layout's Playback
 * BPM and Master Flip/mirror, and the mapping layout's parameters, trimming, blend, visualizer, and
 * 3D model controls.
 */
export const LAYOUT_SPECIFIC_ATTRIBUTES = new Set<string>([
	"media.playback_bpm",
	"media.flip_mirror",
	"media.in_point",
	"media.out_point",
	"media.blend_mode",
	"media.model",
	"media.model.pan",
	"media.model.tilt",
	...[1, 2].flatMap((bank) =>
		[1, 2, 3, 4].map(
			(parameter) => `media.effect.bank.${bank}.parameter.${parameter}`,
		),
	),
	...[1, 2, 3, 4].map((parameter) => `media.visualizer.parameter.${parameter}`),
]);

export const MEDIA_CONTROL_GROUPS = [
	{
		id: "playback",
		label: "Playback",
		attributes: [
			"media.play_mode",
			"intensity",
			"volume",
			"media.playback_speed",
			"media.playback_bpm",
			"media.in_point",
			"media.out_point",
		],
	},
	{
		id: "frame",
		label: "Frame",
		attributes: [
			"media.scale.x",
			"media.scale.y",
			"media.scaling_mode",
			"media.position.x",
			"media.position.y",
			"position.rotation",
		],
	},
	{
		id: "model",
		label: "3D model",
		attributes: ["media.model", "media.model.pan", "media.model.tilt"],
	},
	{
		id: "colour",
		label: "Colour",
		attributes: ["color.tint", "media.grayscale", "media.blend_mode"],
	},
	{
		id: "mask-controls",
		label: "Mask",
		attributes: [
			"media.mask.position.x",
			"media.mask.position.y",
			"media.mask.scale.x",
			"media.mask.scale.y",
			"media.mask.invert",
			"media.mask.opacity",
		],
	},
	{
		id: "visualizer",
		label: "Visualizer",
		attributes: [
			"media.visualizer.parameter.1",
			"media.visualizer.parameter.2",
			"media.visualizer.parameter.3",
			"media.visualizer.parameter.4",
		],
	},
	{
		id: "effects",
		label: "Effects",
		attributes: [
			"media.effect.bank.1.select",
			"media.effect.bank.1.strength",
			"media.effect.bank.1.parameter.1",
			"media.effect.bank.1.parameter.2",
			"media.effect.bank.1.parameter.3",
			"media.effect.bank.1.parameter.4",
			"media.effect.bank.2.select",
			"media.effect.bank.2.strength",
			"media.effect.bank.2.parameter.1",
			"media.effect.bank.2.parameter.2",
			"media.effect.bank.2.parameter.3",
			"media.effect.bank.2.parameter.4",
		],
	},
] as const;

export const MASTER_CONTROL_GROUPS = [
	{
		id: "playback",
		label: "Output",
		attributes: ["intensity", "volume"],
	},
	{
		id: "frame",
		label: "Geometry",
		attributes: [
			"media.scale.x",
			"media.scale.y",
			"media.scaling_mode",
			"media.position.x",
			"media.position.y",
			"position.rotation",
			"media.flip_mirror",
		],
	},
	{
		id: "mask-controls",
		label: "Mask position",
		attributes: ["media.mask.position.x", "media.mask.position.y"],
	},
	{
		id: "shapers",
		label: "Shapers",
		attributes: [
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
	},
	{
		id: "colour",
		label: "Colour",
		attributes: ["color.tint"],
	},
	{
		id: "effects",
		label: "Effects",
		attributes: ["media.master.effect.opacity_cycle"],
	},
] as const;

const MEDIA_CONTROL_LABELS: Record<string, string> = {
	intensity: "Dimmer",
	volume: "Volume",
	"media.play_mode": "Play mode",
	"media.playback_speed": "Speed",
	"media.playback_bpm": "Playback BPM",
	"media.scale.x": "Scale X",
	"media.scale.y": "Scale Y",
	"media.scaling_mode": "Scaling mode",
	"media.position.x": "Position X",
	"media.position.y": "Position Y",
	"position.rotation": "Rotation",
	"color.tint": "Colour",
	"media.grayscale": "Grayscale",
	"media.mask.scale.x": "Mask scale X",
	"media.mask.scale.y": "Mask scale Y",
	"media.mask.position.x": "Mask position X",
	"media.mask.position.y": "Mask position Y",
	"media.mask.invert": "Invert",
	"media.mask.opacity": "Mask opacity",
	"media.effect.bank.1.select": "Effect Select",
	"media.effect.bank.1.strength": "Effect Strength",
	"media.effect.bank.2.select": "Effect Select",
	"media.effect.bank.2.strength": "Effect Strength",
	"media.master.effect.opacity_cycle": "Multiplier / Divider",
	"media.in_point": "In point",
	"media.out_point": "Out point",
	"media.blend_mode": "Blend mode",
	"media.model": "3D model",
	"media.model.pan": "Model pan",
	"media.model.tilt": "Model tilt",
	"media.flip_mirror": "Flip / Mirror",
	"shaper.blade.1.position": "Left",
	"shaper.blade.1.angle": "Left rotation",
	"shaper.blade.2.position": "Right",
	"shaper.blade.2.angle": "Right rotation",
	"shaper.blade.3.position": "Top",
	"shaper.blade.3.angle": "Top rotation",
	"shaper.blade.4.position": "Bottom",
	"shaper.blade.4.angle": "Bottom rotation",
	"shaper.rotation": "Module rotation",
	"media.layer.play.mode": "Play mode",
	"media.layer.dimmer": "Dimmer",
	"media.layer.volume": "Volume",
	"media.layer.speed.multiplier": "Speed",
	"media.layer.playback.bpm": "Playback BPM",
	"media.layer.scale.x": "Scale X",
	"media.layer.scale.y": "Scale Y",
	"media.layer.scaling.mode": "Scaling mode",
	"media.layer.position.x": "Position X",
	"media.layer.position.y": "Position Y",
	"media.layer.rotation": "Rotation",
	"media.layer.tint": "Colour",
	"media.layer.grayscale": "Grayscale",
	"media.layer.mask.scale.x": "Mask scale X",
	"media.layer.mask.scale.y": "Mask scale Y",
	"media.layer.mask.position.x": "Mask position X",
	"media.layer.mask.position.y": "Mask position Y",
	"media.layer.mask.invert": "Invert",
	"media.layer.mask.opacity": "Mask opacity",
	"media.layer.effect.1": "Effect 1",
	"media.layer.effect.2": "Effect 2",
	"media.layer.effect.3": "Effect 3",
	"media.layer.effect.4": "Effect 4",
};

/** The operator label for a control, falling back to a readable form of its attribute. */
export function mediaControlLabel(attribute: string) {
	return MEDIA_CONTROL_LABELS[attribute] ?? readableAttribute(attribute);
}

function readableAttribute(attribute: string) {
	const words = attribute.split(".").slice(2);
	return words
		.map((word, index) =>
			index === 0 ? `${word.charAt(0).toUpperCase()}${word.slice(1)}` : word,
		)
		.join(" ");
}
