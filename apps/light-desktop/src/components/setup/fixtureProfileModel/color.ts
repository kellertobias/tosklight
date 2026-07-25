import type {
	ChannelResolution,
	ColorSystem,
	FixtureMode,
	HeadColorSystem,
	XyzValue,
} from "../../../api/types";
import { maxRaw } from "./rawValues";

export interface XyyValue {
	x: number;
	y: number;
	luminance: number;
}

export interface HighlightRawChoice {
	semantic_id?: string;
	label?: string;
	name?: string;
	raw_value?: number;
	dmx_from?: number;
	dmx_to?: number;
}

const SEMANTIC_WHITE: XyzValue = { x: 0.95047, y: 1, z: 1.08883 };

export function xyzToXyy(value: XyzValue): XyyValue {
	const sum = value.x + value.y + value.z;
	return sum > 0
		? { x: value.x / sum, y: value.y / sum, luminance: value.y }
		: { x: 0, y: 0, luminance: 0 };
}

export function xyyToXyz(value: XyyValue): XyzValue {
	if (value.y <= 0 || value.luminance <= 0) {
		return { x: 0, y: Math.max(0, value.luminance), z: 0 };
	}
	return {
		x: Math.max(0, (value.x * value.luminance) / value.y),
		y: Math.max(0, value.luminance),
		z: Math.max(0, ((1 - value.x - value.y) * value.luminance) / value.y),
	};
}

function identifiesOpenOrWhite(value: string | undefined) {
	const normalized = (value ?? "")
		.trim()
		.toLowerCase()
		.replaceAll(/[^a-z0-9]+/g, " ")
		.trim();
	return [
		"open",
		"white",
		"clear",
		"color open",
		"colour open",
		"color white",
		"colour white",
		"open white",
		"no color",
		"no colour",
	].includes(normalized);
}

/**
 * Derive a physical raw Highlight default for a newly authored/imported channel. Existing profile
 * values are not normalized through this helper, so an explicitly authored raw look stays exact.
 */
export function semanticHighlightRaw(
	attribute: string,
	resolution: ChannelResolution,
	defaultRaw: number,
	invert = false,
	choices: HighlightRawChoice[] = [],
) {
	const maximum = maxRaw(resolution);
	const endpoint = (full: boolean) => (full !== invert ? maximum : 0);
	if (attribute === "intensity") return endpoint(true);
	if (
		[
			"color.red",
			"color.green",
			"color.blue",
			"color.white",
			"color.cold_white",
			"color.warm_white",
		].includes(attribute)
	) {
		return endpoint(true);
	}
	if (["color.cyan", "color.magenta", "color.yellow"].includes(attribute)) {
		return endpoint(false);
	}
	if (
		/^color\.emitter\.(red|green|blue|white|cold_white|warm_white)$/.test(
			attribute,
		)
	) {
		return endpoint(true);
	}
	if (attribute.startsWith("color.wheel")) {
		const choice = choices.find(
			(candidate) =>
				identifiesOpenOrWhite(candidate.semantic_id) ||
				identifiesOpenOrWhite(candidate.label) ||
				identifiesOpenOrWhite(candidate.name),
		);
		if (choice?.raw_value != null) {
			return Math.max(0, Math.min(maximum, Math.round(choice.raw_value)));
		}
		if (choice?.dmx_from != null) {
			const midpoint =
				choice.dmx_from +
				Math.floor(
					((choice.dmx_to ?? choice.dmx_from) - choice.dmx_from) / 2,
				);
			return Math.round((Math.max(0, Math.min(255, midpoint)) * maximum) / 255);
		}
	}
	return Math.max(0, Math.min(maximum, Math.round(defaultRaw)));
}

function correctedWhite(
	matrix: HeadColorSystem["correction_matrix"],
): XyzValue {
	return {
		x:
			matrix[0][0] * SEMANTIC_WHITE.x +
			matrix[0][1] * SEMANTIC_WHITE.y +
			matrix[0][2] * SEMANTIC_WHITE.z,
		y:
			matrix[1][0] * SEMANTIC_WHITE.x +
			matrix[1][1] * SEMANTIC_WHITE.y +
			matrix[1][2] * SEMANTIC_WHITE.z,
		z:
			matrix[2][0] * SEMANTIC_WHITE.x +
			matrix[2][1] * SEMANTIC_WHITE.y +
			matrix[2][2] * SEMANTIC_WHITE.z,
	};
}

function additiveWhiteLevels(
	system: Extract<ColorSystem, { type: "additive" }>,
	matrix: HeadColorSystem["correction_matrix"],
) {
	const visible = system.emitters.filter((emitter) => emitter.visible);
	if (visible.length < 3) {
		return visible.map((emitter) =>
			/red|green|blue|white/i.test(emitter.name) ? 1 : 0,
		);
	}
	const target = correctedWhite(matrix);
	const opticalLimits = visible.map((emitter) =>
		Math.pow(emitter.maximum_level, emitter.response_curve),
	);
	const levels = visible.map(() => 0);
	const norm = Math.max(
		0.001,
		visible.reduce(
			(sum, emitter) =>
				sum + emitter.xyz.x ** 2 + emitter.xyz.y ** 2 + emitter.xyz.z ** 2,
			0,
		),
	);
	const rate = 0.8 / norm;
	for (let iteration = 0; iteration < 256; iteration += 1) {
		const produced = visible.reduce(
			(sum, emitter, index) => ({
				x: sum.x + emitter.xyz.x * levels[index],
				y: sum.y + emitter.xyz.y * levels[index],
				z: sum.z + emitter.xyz.z * levels[index],
			}),
			{ x: 0, y: 0, z: 0 },
		);
		const error = {
			x: produced.x - target.x,
			y: produced.y - target.y,
			z: produced.z - target.z,
		};
		visible.forEach((emitter, index) => {
			const gradient =
				2 *
				(error.x * emitter.xyz.x +
					error.y * emitter.xyz.y +
					error.z * emitter.xyz.z);
			levels[index] = Math.max(
				0,
				Math.min(opticalLimits[index], levels[index] - rate * gradient),
			);
		});
	}
	return levels;
}

/** Return the automatic physical Highlight raw for every channel in the current complete mode. */
export function semanticHighlightDefaultsForMode(mode: FixtureMode) {
	const values = new Map(
		mode.channels.map((channel) => {
			const choices = channel.functions.flatMap((fn) =>
				fn.behavior.type === "fixed" || fn.behavior.type === "indexed"
					? [
							{
								semantic_id: fn.behavior.semantic_id,
								label: fn.behavior.label,
								raw_value: fn.behavior.raw_value,
							},
						]
					: [],
			);
			return [
				channel.id,
				semanticHighlightRaw(
					channel.attribute,
					channel.resolution,
					channel.default_raw,
					channel.invert,
					choices,
				),
			] as const;
		}),
	);
	applyColorSystemHighlights(mode, values);
	return values;
}

function applyColorSystemHighlights(
	mode: FixtureMode,
	values: Map<string, number>,
) {
	for (const record of mode.color_systems) {
		if (record.system.type === "additive") {
			applyAdditiveHighlights(mode, record, values);
		} else if (record.system.type === "subtractive") {
			applySubtractiveHighlights(mode, record, values);
		} else {
			applyWheelHighlight(record, values);
		}
	}
}

function applyAdditiveHighlights(
	mode: FixtureMode,
	record: HeadColorSystem,
	values: Map<string, number>,
) {
	if (record.system.type !== "additive") return;
	const visible = record.system.emitters.filter((emitter) => emitter.visible);
	const levels = additiveWhiteLevels(record.system, record.correction_matrix);
	visible.forEach((emitter, index) => {
		const channel = mode.channels.find(
			(candidate) => candidate.id === emitter.channel_id,
		);
		const level = levels[index];
		if (!channel || !Number.isFinite(level) || emitter.response_curve <= 0) return;
		const drive = Math.max(
			0,
			Math.min(
				emitter.maximum_level,
				Math.pow(
					Math.max(0, Math.min(1, level)),
					1 / emitter.response_curve,
				),
			),
		);
		const maximum = maxRaw(channel.resolution);
		const raw = Math.round(drive * maximum);
		values.set(channel.id, channel.invert ? maximum - raw : raw);
	});
}

function applySubtractiveHighlights(
	mode: FixtureMode,
	record: HeadColorSystem,
	values: Map<string, number>,
) {
	if (record.system.type !== "subtractive") return;
	for (const channelId of [
		record.system.cyan_channel_id,
		record.system.magenta_channel_id,
		record.system.yellow_channel_id,
	]) {
		const channel = mode.channels.find(
			(candidate) => candidate.id === channelId,
		);
		if (channel) {
			values.set(
				channel.id,
				channel.invert ? maxRaw(channel.resolution) : 0,
			);
		}
	}
}

function applyWheelHighlight(
	record: HeadColorSystem,
	values: Map<string, number>,
) {
	if (record.system.type !== "discrete_wheel") return;
	const slot =
		record.system.slots.find(
			(candidate) =>
				identifiesOpenOrWhite(candidate.semantic_id) ||
				identifiesOpenOrWhite(candidate.label),
		) ??
		record.system.slots
			.filter((candidate) => candidate.measured_xyz)
			.sort(
				(left, right) =>
					distanceFromSemanticWhite(left.measured_xyz!) -
					distanceFromSemanticWhite(right.measured_xyz!),
			)[0];
	if (slot) {
		values.set(
			record.system.channel_id,
			slot.dmx_from + Math.floor((slot.dmx_to - slot.dmx_from) / 2),
		);
	}
}

function distanceFromSemanticWhite(value: XyzValue) {
	return (
		(value.x - SEMANTIC_WHITE.x) ** 2 +
		(value.y - SEMANTIC_WHITE.y) ** 2 +
		(value.z - SEMANTIC_WHITE.z) ** 2
	);
}

/** Track automatic defaults as a Color-system draft evolves, without touching custom raw values. */
export function reconcileColorSystemHighlightDefaults(
	mode: FixtureMode,
	colorSystems: HeadColorSystem[],
): FixtureMode {
	const previous = semanticHighlightDefaultsForMode(mode);
	const next = { ...mode, color_systems: colorSystems };
	const derived = semanticHighlightDefaultsForMode(next);
	return {
		...next,
		channels: mode.channels.map((channel) =>
			channel.highlight_raw === previous.get(channel.id)
				? {
						...channel,
						highlight_raw: derived.get(channel.id) ?? channel.highlight_raw,
					}
				: channel,
		),
	};
}
