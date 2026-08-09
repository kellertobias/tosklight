import type {
	ChannelFunction,
	ChannelResolution,
	FixtureChannel,
	FixtureDefinition,
	FixtureHead,
	FixtureMode,
	FixtureProfile,
	HeadColorSystem,
} from "../../../api/types";
import { derivePrimarySlots } from "./channels";
import {
	canonicalAttributeProjection,
	semanticHighlightDefaultsForMode,
	semanticHighlightRaw,
} from "./color";
import { blankFixtureProfile, cloneProfile } from "./defaults";
import { blankGeometry } from "./geometry";
import { maxRaw } from "./rawValues";
import { uuid } from "./utilities";

function channelFunctions(
	parameter: FixtureDefinition["heads"][number]["parameters"][number],
	maximum: number,
	canonicalAttribute: string,
): ChannelFunction[] {
	return parameter.capabilities.map((capability) => {
		const dmxFrom = Math.round(
			(Math.max(0, Math.min(255, capability.dmx_from)) * maximum) / 255,
		);
		const dmxTo = Math.round(
			(Math.max(0, Math.min(255, capability.dmx_to)) * maximum) / 255,
		);
		const label = capability.name.trim() || parameter.attribute;
		const semanticId = label
			.toLowerCase()
			.replaceAll(/[^a-z0-9]+/g, ".")
			.replace(/^\.+|\.+$/g, "");
		const indexed =
			capability.preset_family === "color" ||
			capability.preset_family === "gobo" ||
			/color\.wheel|gobo/.test(parameter.attribute);
		return {
			id: uuid(),
			name: label,
			dmx_from: dmxFrom,
			dmx_to: dmxTo,
			attribute: canonicalAttribute,
			priority: 100,
			behavior: indexed
				? {
						type: "indexed",
						semantic_id: semanticId,
						label,
						raw_value: dmxFrom + Math.floor((dmxTo - dmxFrom) / 2),
					}
				: {
						type: "fixed",
						semantic_id: semanticId,
						label,
						raw_value: dmxFrom + Math.floor((dmxTo - dmxFrom) / 2),
					},
		};
	});
}

function channelsFromDefinition(
	definition: FixtureDefinition,
	heads: FixtureHead[],
): FixtureChannel[] {
	const indexed = definition.heads
		.flatMap((head, headIndex) =>
			head.parameters.map((parameter) => ({ headIndex, parameter })),
		)
		.sort(
			(left, right) =>
				(left.parameter.components[0]?.offset ?? 0) -
				(right.parameter.components[0]?.offset ?? 0),
		);
	return indexed.map(({ headIndex, parameter }) => {
		const projection = canonicalAttributeProjection(parameter.attribute);
		const resolution =
			`u${Math.max(1, parameter.components.length) * 8}` as ChannelResolution;
		const maximum = maxRaw(resolution);
		const defaultRaw = Math.round(parameter.default * maximum);
		const invert = parameter.metadata?.invert ?? false;
		return {
			id: uuid(),
			head_id: heads[headIndex].id,
			split: 1,
			fixture_attribute: parameter.source_attribute ?? parameter.attribute,
			attribute: projection.attribute,
			canonical_transform: projection.canonicalTransform,
			resolution,
			secondary_slots: parameter.components
				.slice(1)
				.map((component) => component.offset + 1),
			default_raw: defaultRaw,
			highlight_raw: semanticHighlightRaw(
				parameter.attribute,
				resolution,
				defaultRaw,
				invert,
				parameter.capabilities,
			),
			physical_min: parameter.metadata?.physical_min ?? 0,
			physical_max: parameter.metadata?.physical_max ?? 1,
			unit: parameter.metadata?.unit ?? null,
			invert,
			snap: false,
			reacts_to_virtual_intensity: parameter.virtual_dimmer,
			reacts_to_sequence_master: /intensity/.test(parameter.attribute),
			reacts_to_group_master: /intensity/.test(parameter.attribute),
			reacts_to_grand_master: /intensity/.test(parameter.attribute),
			behavior: "controlled",
			functions: channelFunctions(parameter, maximum, projection.attribute),
		};
	});
}

function colorSystemsFromDefinition(
	definition: FixtureDefinition,
	heads: FixtureHead[],
	channels: FixtureChannel[],
): HeadColorSystem[] {
	return heads.flatMap<HeadColorSystem>((head, headIndex) => {
		const hue = channels.find(
			(channel) =>
				channel.head_id === head.id && channel.attribute === "color.hue",
		);
		const saturation = channels.find(
			(channel) =>
				channel.head_id === head.id && channel.attribute === "color.saturation",
		);
		const intensity = channels.find(
			(channel) =>
				channel.head_id === head.id && channel.attribute === "color.brightness",
		);
		if (hue && saturation) {
			return [
				{
					head_id: head.id,
					correction_matrix: [
						[1, 0, 0],
						[0, 1, 0],
						[0, 0, 1],
					] as HeadColorSystem["correction_matrix"],
					system: {
						type: "hue_saturation" as const,
						hue_channel_id: hue.id,
						saturation_channel_id: saturation.id,
						intensity_channel_id: intensity?.id ?? null,
					},
				},
			];
		}
		if (!definition.color_calibration) return [];
		const emitters = definition.color_calibration.emitters.flatMap(
			(emitter) => {
				const name = emitter.name
					.trim()
					.toLowerCase()
					.replaceAll(/[^a-z0-9]+/g, "_");
				const fixtureAttributes = [`color.emitter.${name}`, `color.${name}`];
				const fixtureChannel = channels.find(
					(candidate) =>
						candidate.head_id === head.id &&
						fixtureAttributes.includes(candidate.fixture_attribute),
				);
				const canonicalAttribute = canonicalAttributeProjection(
					`color.${name}`,
				).attribute;
				const sourceAttributes =
					definition.heads[headIndex]?.parameters
						.filter((parameter) => {
							const projection = canonicalAttributeProjection(parameter.attribute);
							return (
								projection.attribute === canonicalAttribute &&
								projection.canonicalTransform === "identity"
							);
						})
						.map(
							(parameter) => parameter.source_attribute ?? parameter.attribute,
						) ?? [];
				const sourceChannels = channels.filter(
					(candidate) =>
						candidate.head_id === head.id &&
						sourceAttributes.includes(candidate.fixture_attribute),
				);
				const canonicalChannels = channels.filter(
					(candidate) =>
						candidate.head_id === head.id &&
						candidate.attribute === canonicalAttribute &&
						candidate.canonical_transform !== "invert_normalized",
				);
				const channel =
					fixtureChannel ??
					(sourceChannels.length === 1 ? sourceChannels[0] : undefined) ??
					(canonicalChannels.length === 1 ? canonicalChannels[0] : undefined);
				return channel
					? [
							{
								channel_id: channel.id,
								name: emitter.name,
								xyz: emitter.xyz,
								maximum_level: emitter.limit,
								response_curve: 1,
								visible: !/^(uv|ir)$|ultraviolet|infrared/i.test(emitter.name),
							},
						]
					: [];
			},
		);
		return emitters.length
			? [
					{
						head_id: head.id,
						correction_matrix: definition.color_calibration
							.correction_matrix as HeadColorSystem["correction_matrix"],
						system: { type: "additive" as const, emitters },
					},
				]
			: [];
	});
}

function modeFromDefinition(
	definition: FixtureDefinition,
	heads: FixtureHead[],
	channels: FixtureChannel[],
): FixtureMode {
	const mode: FixtureMode = {
		id: definition.mode_id ?? uuid(),
		name: definition.mode || "Default",
		notes: "",
		splits: [{ number: 1, footprint: definition.footprint }],
		heads,
		channels,
		color_systems: colorSystemsFromDefinition(definition, heads, channels),
		control_actions: [],
		geometry: blankGeometry(heads.map((head) => head.id)),
	};
	const semanticDefaults = semanticHighlightDefaultsForMode(mode);
	mode.channels = mode.channels.map((channel) => ({
		...channel,
		highlight_raw: semanticDefaults.get(channel.id) ?? channel.highlight_raw,
	}));
	return mode;
}

export function fixtureProfileFromDefinition(
	definition: FixtureDefinition,
): FixtureProfile {
	if (definition.profile_snapshot) {
		return cloneProfile(definition.profile_snapshot);
	}
	const heads: FixtureHead[] = definition.heads.map((head) => ({
		id: uuid(),
		name: head.name,
		master_shared: head.shared,
	}));
	const channels = channelsFromDefinition(definition, heads);
	const mode = modeFromDefinition(definition, heads, channels);
	const defaults = blankFixtureProfile();
	return {
		...defaults,
		id: definition.profile_id ?? definition.id,
		revision: definition.revision,
		manufacturer: definition.manufacturer,
		name: definition.name,
		short_name: definition.model,
		fixture_type: definition.device_type,
		stage_icon_asset: definition.icon_asset ?? null,
		model_asset: definition.model_asset ?? null,
		physical: {
			...defaults.physical,
			width_millimetres: definition.physical.width_millimetres ?? null,
			height_millimetres: definition.physical.height_millimetres ?? null,
			depth_millimetres: definition.physical.depth_millimetres ?? null,
			weight_kilograms: definition.physical.weight_kilograms ?? null,
			power_watts: definition.physical.power_watts ?? null,
		},
		hazardous: definition.hazardous,
		direct_control_protocols: definition.direct_control_protocols,
		signal_loss_policy: definition.signal_loss_policy,
		modes: [mode],
	};
}

/** Convert the ordered per-mode result of a legacy/GDTF import into one atomic profile draft. */
export function fixtureProfileFromDefinitions(
	definitions: FixtureDefinition[],
): FixtureProfile {
	if (!definitions.length) return blankFixtureProfile();
	const converted = definitions.map(fixtureProfileFromDefinition);
	return {
		...converted[0],
		revision: 0,
		modes: converted.flatMap((profile) => profile.modes),
	};
}

/** Resolve every ordered mode into the portable definition shape embedded by a patched show. */
export function fixtureDefinitionsFromProfiles(profiles: FixtureProfile[]) {
	return profiles.flatMap((profile) =>
		profile.modes.map((mode) =>
			fixtureDefinitionFromProfileMode(profile, mode),
		),
	);
}

export function fixtureDefinitionFromProfileMode(
	profile: FixtureProfile,
	mode: FixtureMode,
): FixtureDefinition {
	const primary = derivePrimarySlots(mode).slots;
	const footprint =
		mode.splits.find((split) => split.number === 1)?.footprint ??
		mode.splits[0]?.footprint ??
		1;
	const additive = mode.color_systems.find(
		(record) => record.system.type === "additive",
	);
	return {
		schema_version: 2,
		id: profile.id,
		revision: profile.revision,
		manufacturer: profile.manufacturer,
		device_type: profile.fixture_type,
		name: profile.name,
		model: profile.short_name,
		mode: mode.name,
		footprint,
		heads: mode.heads.map((head, index) => ({
			index,
			name: head.name,
			shared: head.master_shared,
			parameters: headParameters(mode, head.id, primary),
		})),
		color_calibration:
			additive?.system.type === "additive"
				? {
						emitters: additive.system.emitters.map((emitter) => ({
							name: emitter.name,
							xyz: emitter.xyz,
							limit: emitter.maximum_level,
						})),
						correction_matrix: additive.correction_matrix,
					}
				: null,
		physical: {
			width_millimetres: profile.physical.width_millimetres,
			height_millimetres: profile.physical.height_millimetres,
			depth_millimetres: profile.physical.depth_millimetres,
			weight_kilograms: profile.physical.weight_kilograms,
			power_watts: profile.physical.power_watts,
		},
		model_asset: profile.model_asset,
		icon_asset: profile.stage_icon_asset,
		hazardous: profile.hazardous,
		direct_control_protocols: profile.direct_control_protocols,
		signal_loss_policy: profile.signal_loss_policy,
		safe_values: {},
		profile_id: profile.id,
		mode_id: mode.id,
		// Library definitions are immutable view models. Reuse the profile here so a large
		// multi-mode catalog does not deep-clone the complete profile once per mode. The selected
		// definition is serialized by the patch request (creating its portable value snapshot), and
		// fixtureProfileFromDefinition deep-clones before an editor can mutate the profile.
		profile_snapshot: profile,
	};
}

/**
 * A head whose channels react to virtual intensity but that has no physical intensity channel
 * (e.g. an RGB head whose intensity is derived) exposes the abstract virtual-dimmer intensity
 * parameter the operator controls, matching the server's resolved definition projection.
 */
function headParameters(
	mode: FixtureMode,
	headId: string,
	primary: Map<string, number>,
): FixtureDefinition["heads"][number]["parameters"] {
	const channels = mode.channels.filter(
		(channel) => channel.head_id === headId,
	);
	const parameters = channels.map((channel) =>
		channelDefinition(channel, primary),
	);
	const reactsToVirtual = channels.some(
		(channel) => channel.reacts_to_virtual_intensity,
	);
	const hasIntensity = parameters.some(
		(parameter) => parameter.attribute === "intensity",
	);
	if (reactsToVirtual && !hasIntensity) {
		parameters.unshift({
			attribute: "intensity",
			components: [],
			default: 1,
			virtual_dimmer: true,
			metadata: {
				physical_min: 0,
				physical_max: 1,
				unit: null,
				invert: false,
				wrap: false,
				curve: "linear",
			},
			capabilities: [],
		});
	}
	return parameters;
}

function channelDefinition(
	channel: FixtureChannel,
	primary: Map<string, number>,
): FixtureDefinition["heads"][number]["parameters"][number] {
	return {
		attribute: channel.attribute,
		components: [primary.get(channel.id) ?? 1, ...channel.secondary_slots].map(
			(slot) => ({ offset: slot - 1, byte_order: "msb_first" as const }),
		),
		default: channel.default_raw / maxRaw(channel.resolution),
		virtual_dimmer: channel.reacts_to_virtual_intensity,
		metadata: {
			physical_min: channel.physical_min ?? 0,
			physical_max: channel.physical_max ?? 1,
			unit: channel.unit,
			invert: channel.invert,
			wrap: false,
			curve: "linear",
		},
		capabilities: channel.functions.flatMap((fn) =>
			fn.behavior.type === "fixed" || fn.behavior.type === "indexed"
				? [
						{
							name: fn.behavior.label,
							dmx_from: fn.dmx_from,
							dmx_to: fn.dmx_to,
							preset_family:
								fn.behavior.type === "indexed"
									? fn.attribute.includes("gobo")
										? "gobo"
										: "color"
									: null,
						},
					]
				: [],
		),
	};
}
