import type {
	DynamicLaneProjection,
	DynamicProgrammingLaneConfigurationProjection,
	DynamicRandomGroupProjection,
	DynamicScalarSourceProjection,
	DynamicValueAddressProjection,
	DynamicValueSourceProjection,
} from "../../api/types";
import {
	type ProgrammingDynamicLane,
	type ScalarDynamicLane,
	isScalarDynamicLane,
} from "./laneModel";
import {
	type DynamicLaneDomain,
	laneDomainForAddress,
	laneDomainForKey,
} from "./laneDomain";

/**
 * One lane as the curve editor composes it (TL-648): the scalar lane shape, with values in the
 * lane's descriptor units and `attribute` holding its lane-chooser key. A scalar lane is its own
 * editable form. A typed lane is projected into it and committed back to the typed Programming
 * body, which keeps only the active method, so nothing percentage-shaped is ever stored for it.
 */
export type EditableDynamicLane = ScalarDynamicLane;

const pwm = () => ({
	attack: 0,
	on: 0.5,
	decay: 0,
	off: 0.5,
	attack_interpolation: "linear" as const,
	decay_interpolation: "linear" as const,
});

const valueSource = (value: number): DynamicScalarSourceProjection => ({
	type: "value",
	value,
});

/** The lane's descriptor: its typed family component, or a scalar 0–1 attribute. */
export function dynamicLaneDomain(
	lane: DynamicLaneProjection,
): DynamicLaneDomain | null {
	return isScalarDynamicLane(lane)
		? laneDomainForKey(lane.attribute)
		: laneDomainForAddress(lane.programming.address);
}

/** Every method's configuration for a new lane of `domain`. */
function defaultConfigurations(domain: DynamicLaneDomain) {
	const { minimum, maximum, middle, amplitude } = domain.defaults;
	return {
		keyframes: {
			points: [
				{
					position: 0,
					source: valueSource(minimum),
					interpolation: "ease_in_out" as const,
				},
				{
					position: 0.5,
					source: valueSource(maximum),
					interpolation: "ease_in_out" as const,
				},
			],
			size: 1,
		},
		max_min: {
			minimum: valueSource(minimum),
			maximum: valueSource(maximum),
			function: "sinus" as const,
			size: 1,
			pwm: pwm(),
		},
		middle_amplitude: {
			middle:
				middle === "current"
					? ({ type: "current" } as const)
					: valueSource(middle),
			amplitude,
			function: "sinus" as const,
			size: 1,
			pwm: pwm(),
			invert_waveform: false,
		},
	};
}

function editableSource(
	source: DynamicValueSourceProjection,
	key: string,
): DynamicScalarSourceProjection | null {
	switch (source.kind) {
		case "current":
			return { type: "current" };
		case "value":
			return source.value.kind === "scalar"
				? valueSource(source.value.value)
				: null;
		case "preset":
			return {
				type: "preset",
				preset_id: source.preset_id,
				attribute: key,
				last_valid_by_target: [],
			};
	}
}

function typedSource(
	source: DynamicScalarSourceProjection,
	address: DynamicValueAddressProjection,
	original: ProgrammingDynamicLane | null,
): DynamicValueSourceProjection {
	switch (source.type) {
		case "current":
			return { kind: "current" };
		case "value":
			return { kind: "value", value: { kind: "scalar", value: source.value } };
		case "preset":
			return (
				(original && retainedPresetSource(original, source.preset_id)) ?? {
					kind: "preset",
					preset_id: source.preset_id,
					address,
					last_valid_by_target: [],
				}
			);
	}
}

/** A Preset source already stored on the lane keeps its fallbacks and retained template. */
function retainedPresetSource(lane: ProgrammingDynamicLane, presetId: string) {
	const configuration = lane.programming.configuration;
	const sources: DynamicValueSourceProjection[] =
		configuration.mode === "keyframes"
			? configuration.configuration.points.map((point) => point.source)
			: configuration.mode === "max_min"
				? [
						configuration.configuration.minimum,
						configuration.configuration.maximum,
					]
				: configuration.mode === "middle_amplitude"
					? [configuration.configuration.middle]
					: [];
	return (
		sources.find(
			(source) => source.kind === "preset" && source.preset_id === presetId,
		) ?? null
	);
}

/** The lane as the curve editor composes it, or null for a lane it can only inspect. */
export function editableLane(
	lane: DynamicLaneProjection,
): EditableDynamicLane | null {
	if (isScalarDynamicLane(lane)) return lane;
	const domain = laneDomainForAddress(lane.programming.address);
	if (!domain) return null;
	const defaults = defaultConfigurations(domain);
	const base = {
		id: lane.id,
		speed_multiplier: lane.speed_multiplier,
		width: lane.width,
		random_group_id: lane.random_group_id ?? null,
		phase: lane.phase ?? null,
		attribute: domain.key,
		...defaults,
	};
	const configuration = lane.programming.configuration;
	const source = (value: DynamicValueSourceProjection) =>
		editableSource(value, domain.key);
	switch (configuration.mode) {
		case "random":
			return { ...base, mode: "random" };
		case "keyframes": {
			const points = configuration.configuration.points.map((point) => {
				const converted = source(point.source);
				return converted ? { ...point, source: converted } : null;
			});
			if (points.some((point) => point === null)) return null;
			return {
				...base,
				mode: "keyframes",
				keyframes: {
					points: points as EditableDynamicLane["keyframes"]["points"],
					size: configuration.configuration.size,
				},
			};
		}
		case "max_min": {
			const { minimum, maximum, ...rest } = configuration.configuration;
			const low = source(minimum);
			const high = source(maximum);
			if (!low || !high) return null;
			return {
				...base,
				mode: "max_min",
				max_min: { ...rest, minimum: low, maximum: high },
			};
		}
		case "middle_amplitude": {
			const { middle, amplitude, ...rest } = configuration.configuration;
			const center = source(middle);
			if (!center || amplitude.kind !== "scalar") return null;
			return {
				...base,
				mode: "middle_amplitude",
				middle_amplitude: { ...rest, middle: center, amplitude: amplitude.value },
			};
		}
	}
}

function typedConfiguration(
	editable: EditableDynamicLane,
	address: DynamicValueAddressProjection,
	original: ProgrammingDynamicLane | null,
): DynamicProgrammingLaneConfigurationProjection {
	const source = (value: DynamicScalarSourceProjection) =>
		typedSource(value, address, original);
	switch (editable.mode) {
		case "random":
			return { mode: "random" };
		case "keyframes":
			return {
				mode: "keyframes",
				configuration: {
					points: editable.keyframes.points.map((point) => ({
						...point,
						source: source(point.source),
					})),
					size: editable.keyframes.size,
				},
			};
		case "max_min":
			return {
				mode: "max_min",
				configuration: {
					...editable.max_min,
					minimum: source(editable.max_min.minimum),
					maximum: source(editable.max_min.maximum),
				},
			};
		case "middle_amplitude":
			return {
				mode: "middle_amplitude",
				configuration: {
					...editable.middle_amplitude,
					middle: source(editable.middle_amplitude.middle),
					amplitude: {
						kind: "scalar",
						value: Math.max(0, editable.middle_amplitude.amplitude),
					},
					invert_waveform: editable.middle_amplitude.invert_waveform ?? false,
				},
			};
	}
}

function typedLane(
	editable: EditableDynamicLane,
	address: DynamicValueAddressProjection,
	original: ProgrammingDynamicLane | null,
): ProgrammingDynamicLane {
	return {
		id: editable.id,
		speed_multiplier: editable.speed_multiplier,
		width: editable.width,
		random_group_id: editable.random_group_id ?? null,
		phase: editable.phase ?? null,
		programming: {
			address,
			configuration: typedConfiguration(editable, address, original),
		},
	};
}

/** Writes an edited lane back in the stored shape of `original`. */
export function commitEditableLane(
	editable: EditableDynamicLane,
	original: DynamicLaneProjection,
): DynamicLaneProjection {
	if (isScalarDynamicLane(original)) return editable;
	return typedLane(editable, original.programming.address, original);
}

/** A new lane for a lane-chooser key: typed on a family component, scalar otherwise. */
export function createDynamicLane(
	key: string,
	id: string = crypto.randomUUID(),
): DynamicLaneProjection {
	const domain = laneDomainForKey(key);
	const defaults = defaultConfigurations(domain);
	const editable: EditableDynamicLane = {
		id,
		attribute: domain.key,
		mode: domain.defaults.method,
		...defaults,
		speed_multiplier: { numerator: 1, denominator: 1 },
		width: 1,
		random_group_id: null,
		phase: null,
	};
	return domain.address ? typedLane(editable, domain.address, null) : editable;
}

/**
 * The lane re-addressed to another key. A scalar lane moving to another scalar attribute keeps
 * its curve; any change of domain starts from that domain's defaults, keeping identity, speed,
 * width and phase.
 */
export function retargetDynamicLane(
	lane: DynamicLaneProjection,
	key: string,
): DynamicLaneProjection {
	const next = laneDomainForKey(key);
	if (isScalarDynamicLane(lane) && !next.address)
		return { ...lane, attribute: key };
	return {
		...createDynamicLane(key, lane.id),
		speed_multiplier: lane.speed_multiplier,
		width: lane.width,
		phase: lane.phase ?? null,
	};
}

/** A Random group whose range lies in `domain`'s units and stored shape. */
export function createRandomGroup(
	domain: DynamicLaneDomain,
): DynamicRandomGroupProjection {
	const shared = {
		id: crypto.randomUUID(),
		seed: crypto.getRandomValues(new Uint32Array(1))[0] ?? 0,
		decision_interval_millis: 250,
		start_probability: 0.25,
		mean_duration_millis: 500,
		duration_spread_millis: 100,
		attack_ratio: 0.1,
		decay_ratio: 0.1,
	};
	const { minimum, maximum } = domain.defaults;
	return domain.address
		? {
				...shared,
				programming_range: {
					low: { kind: "value", value: { kind: "scalar", value: minimum } },
					high: { kind: "value", value: { kind: "scalar", value: maximum } },
				},
			}
		: { ...shared, low: valueSource(minimum), high: valueSource(maximum) };
}

/** Whether a Random group's stored range can carry lanes of `domain`. */
export function randomGroupFits(
	group: DynamicRandomGroupProjection,
	domain: DynamicLaneDomain,
) {
	return ("programming_range" in group) === (domain.address !== null);
}

/** A Random group's range in descriptor units. */
export function editableRandomRange(
	group: DynamicRandomGroupProjection,
	key: string,
): { low?: DynamicScalarSourceProjection; high?: DynamicScalarSourceProjection } {
	if ("low" in group) return { low: group.low, high: group.high };
	return {
		low: editableSource(group.programming_range.low, key) ?? undefined,
		high: editableSource(group.programming_range.high, key) ?? undefined,
	};
}

/** The lane on the graph's 0–1 vertical axis, for curves and the selection preview. */
export function graphLane(
	editable: EditableDynamicLane,
	domain: DynamicLaneDomain,
): EditableDynamicLane {
	if (!domain.address) return editable;
	const span = Math.max(1e-9, domain.maximum - domain.minimum);
	const fraction = (source: DynamicScalarSourceProjection) =>
		source.type === "value"
			? valueSource((source.value - domain.minimum) / span)
			: source;
	return {
		...editable,
		keyframes: {
			...editable.keyframes,
			points: editable.keyframes.points.map((point) => ({
				...point,
				source: fraction(point.source),
			})),
		},
		max_min: {
			...editable.max_min,
			minimum: fraction(editable.max_min.minimum),
			maximum: fraction(editable.max_min.maximum),
		},
		middle_amplitude: {
			...editable.middle_amplitude,
			middle: fraction(editable.middle_amplitude.middle),
			amplitude: editable.middle_amplitude.amplitude / span,
		},
	};
}
