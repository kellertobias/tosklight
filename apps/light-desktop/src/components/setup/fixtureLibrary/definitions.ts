import type { FixtureDefinition } from "../../../api/types";

export const FIXTURE_TYPES = [
	"dimmer",
	"fogger",
	"profile",
	"wash",
	"wash mover",
	"spot mover",
	"beam mover",
	"strobe",
	"media server",
	"pixel fixture",
	"other",
];

export function blankDefinition(): FixtureDefinition {
	return {
		schema_version: 1,
		id: crypto.randomUUID(),
		revision: 1,
		manufacturer: "",
		device_type: "other",
		name: "",
		model: "",
		mode: "Standard",
		footprint: 1,
		heads: [
			{
				index: 0,
				name: "Main",
				shared: true,
				parameters: [
					{
						attribute: "intensity",
						components: [{ offset: 0, byte_order: "msb_first" }],
						default: 0,
						virtual_dimmer: false,
						capabilities: [],
					},
				],
			},
		],
		color_calibration: null,
		physical: {},
		model_asset: null,
		icon_asset: null,
		hazardous: false,
		direct_control_protocols: [],
		signal_loss_policy: { type: "hold_last" },
		safe_values: {},
	};
}

export function fixtureAttributeName(value: string) {
	const normalized = value
		.replace(/([a-z])([A-Z])/g, "$1.$2")
		.replace(/\s+/g, ".")
		.toLowerCase();
	const aliases: Record<string, string> = {
		dimmer: "intensity",
		"color.add_r": "color.red",
		"color.add_g": "color.green",
		"color.add_b": "color.blue",
		"color.add_w": "color.white",
		"color.add_ww": "color.warm_white",
		"color.add_cw": "color.cold_white",
		"color.sub_c": "color.cyan",
		"color.sub_m": "color.magenta",
		"color.sub_y": "color.yellow",
	};
	const wheel = normalized.match(/^color(?:\.wheel)?_?(\d+)$/);
	return (
		aliases[normalized] ?? (wheel ? `color.wheel.${wheel[1]}` : normalized)
	);
}

interface HeadDraft {
	name: string;
	master: boolean;
	channels: string;
}

function splitChannels(value: string) {
	const result: string[] = [];
	let depth = 0;
	let start = 0;
	for (let index = 0; index < value.length; index++) {
		if (value[index] === "{" || value[index] === "[") depth++;
		if (value[index] === "}" || value[index] === "]") depth--;
		if (value[index] === "," && depth === 0) {
			result.push(value.slice(start, index));
			start = index + 1;
		}
	}
	result.push(value.slice(start));
	return result;
}

export function parseHeadDrafts(heads: HeadDraft[]) {
	let offset = 0;
	const parsedHeads = heads.map((head, headIndex) => ({
		index: headIndex,
		name: head.name.trim() || `Head ${headIndex + 1}`,
		shared: head.master,
		parameters: splitChannels(head.channels)
			.map((raw) => raw.trim())
			.filter(Boolean)
			.map((raw) => {
				const capabilitiesText = raw.match(/\{(.+)\}/)?.[1] ?? "";
				const rangeText = raw.match(/\[(-?[\d.]+),(-?[\d.]+)(?:,([^\]]+))?\]/);
				const clean = raw.replace(/\{.+\}/, "").replace(/\[.+\]/, "");
				const [attribute, resolution] = clean.split(":");
				const bytes = resolution === "16" ? 2 : 1;
				const start = offset;
				offset += bytes;
				return {
					attribute: fixtureAttributeName(attribute),
					components: Array.from({ length: bytes }, (_, component) => ({
						offset: start + component,
						byte_order: "msb_first" as const,
					})),
					default: attribute.toLowerCase().includes("shutter") ? 1 : 0,
					virtual_dimmer: false,
					metadata: {
						physical_min: rangeText ? Number(rangeText[1]) : 0,
						physical_max: rangeText ? Number(rangeText[2]) : 1,
						unit: rangeText?.[3] ?? null,
						invert: false,
						wrap: attribute.toLowerCase().includes("pan"),
						curve: "linear",
					},
					capabilities: capabilitiesText
						.split("|")
						.filter(Boolean)
						.map((entry) => {
							const [name, range = "0-255"] = entry.split("=");
							const [from, to = from] = range.split("-").map(Number);
							return {
								name: name.trim(),
								dmx_from: from,
								dmx_to: to,
								preset_family: attribute.toLowerCase().includes("gobo")
									? "gobo"
									: null,
							};
						}),
				};
			}),
	}));
	return { heads: parsedHeads, footprint: offset };
}
