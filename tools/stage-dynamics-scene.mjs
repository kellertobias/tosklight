import { LARGE_STAGE_DYNAMIC_INSTANCES } from "./stage-large-scene.mjs";

/**
 * Profile channel attributes that carry an animated Large Stage lane, mapped to the lane the
 * contract-1 desk accepts (TL-552). Intensity stays a scalar lane. Color and Position are
 * semantic families since the cutover, so their channels map to one component of the owner:
 * subtractive CMY flags drive the matching recipe primary, White drives White Blend, and Pan and
 * Tilt drive Angles in degrees. One lane per (target, component) keeps the workload's address
 * count and motion of the legacy scalar scene.
 */
const DYNAMIC_ATTRIBUTES = new Map([
	["intensity", "intensity"],
	["color.red", "color.red"],
	["color.cyan", "color.red"],
	["color.green", "color.green"],
	["color.magenta", "color.green"],
	["color.blue", "color.blue"],
	["color.yellow", "color.blue"],
	["color.amber", "color.amber"],
	["color.white", "color.white_blend"],
	["color.uv", "color.uv"],
	["pan", "position.pan"],
	["tilt", "position.tilt"],
]);

/** Nominal centred travel used to express the legacy 20–80 % Pan/Tilt sweep in degrees. */
export const LARGE_STAGE_PAN_TRAVEL_DEGREES = 540;
export const LARGE_STAGE_TILT_TRAVEL_DEGREES = 270;

/** The family a lane belongs to; one Dynamic animates one family, as an operator would. */
const laneFamily = (lane) =>
	lane === "intensity" ? "intensity" : lane.startsWith("color.") ? "color" : "position";

export function createLargeStageDynamicsPlan(patch, largeScene) {
	const dynamicRoots = new Set(largeScene.dynamicFixtureIds);
	const profiles = new Map(
		patch.profile_revisions.map((profile) => [
			`${profile.profile_id}:${profile.profile_revision}`,
			profile,
		]),
	);
	const descriptors = patch.fixtures
		.filter((fixture) => dynamicRoots.has(fixture.fixture_id))
		.flatMap((fixture) => fixtureTargetDescriptors(fixture, profiles))
		.sort(
			(left, right) =>
				left.signature.localeCompare(right.signature) ||
				left.target.localeCompare(right.target),
		);
	if (descriptors.length < LARGE_STAGE_DYNAMIC_INSTANCES)
		throw new Error(
			`Large Stage has only ${descriptors.length} Dynamic targets for ${LARGE_STAGE_DYNAMIC_INSTANCES} instances`,
		);
	const addresses = descriptors.flatMap((descriptor) =>
		descriptor.attributes.map((attribute) => ({
			target: descriptor.target,
			attribute,
		})),
	);
	// A Color or Position owner is one family value per target, so the components of one target
	// share one Dynamic (two Dynamics on one owner would replace each other, not add up). Each
	// partition unit is one target's family with its component lanes.
	const units = [
		...Map.groupBy(
			addresses,
			(address) =>
				`${laneFamily(address.attribute)}\u0000${address.target}`,
		).values(),
	].map((group) => ({
		target: group[0].target,
		family: laneFamily(group[0].attribute),
		lanes: group.map((address) => address.attribute).sort(),
	}));
	const buckets = partitionUnits(units, LARGE_STAGE_DYNAMIC_INSTANCES);
	const activations = buckets.map((bucket, index) => ({
		definition: dynamicDefinition(bucket, index),
		targets: bucket.map((unit) => unit.target),
	}));
	const identities = buckets.flatMap((bucket) =>
		bucket.flatMap((unit) => unit.lanes.map((lane) => `${unit.target}:${lane}`)),
	);
	if (identities.length !== addresses.length)
		throw new Error("Large Stage Dynamic partitions lost an address");
	if (new Set(identities).size !== identities.length)
		throw new Error("Large Stage Dynamic address partitions overlap");
	return {
		definitions: activations.map((activation) => activation.definition),
		activations,
		targetDescriptors: descriptors,
		dynamicTargetCount: addresses.length,
		staticControlFixtureIds: [...largeScene.staticControlFixtureIds],
		laneCoverage: Object.fromEntries(
			[...new Set(DYNAMIC_ATTRIBUTES.values())]
				.map((lane) => [
					lane,
					addresses.filter((address) => address.attribute === lane).length,
				])
				.filter(([, count]) => count > 0),
		),
	};
}

function fixtureTargetDescriptors(fixture, profiles) {
	const revision = profiles.get(
		`${fixture.profile_id}:${fixture.profile_revision}`,
	);
	const snapshot = revision?.profile_snapshot;
	const mode = snapshot?.modes?.find(
		(candidate) => candidate.id === fixture.mode_id,
	);
	if (!mode)
		throw new Error(
			`Large Stage cannot resolve ${fixture.name} profile mode ${fixture.mode_id}`,
		);
	const descriptors = [];
	for (const [headIndex, head] of mode.heads.entries()) {
		const channels = mode.channels.filter(
			(channel) => channel.head_id === head.id,
		);
		const attributes = new Set(
			channels
				.map((channel) => DYNAMIC_ATTRIBUTES.get(channel.attribute))
				.filter((lane) => lane !== undefined),
		);
		// Angles are a pair: a head with either axis animates both.
		if (attributes.has("position.pan") || attributes.has("position.tilt")) {
			attributes.add("position.pan");
			attributes.add("position.tilt");
		}
		if (
			!attributes.has("intensity") &&
			channels.some((channel) => channel.reacts_to_virtual_intensity)
		)
			attributes.add("intensity");
		if (attributes.size === 0) continue;
		const target = head.master_shared
			? fixture.fixture_id
			: fixture.logical_heads.find(
					(logical) =>
						logical.profile_head_id === head.id ||
						(logical.profile_head_id === null &&
							logical.head_index === headIndex),
				)?.fixture_id;
		if (!target)
			throw new Error(
				`Large Stage cannot resolve logical head ${head.name} on ${fixture.name}`,
			);
		const sortedAttributes = [...attributes].sort();
		descriptors.push({
			target,
			fixtureId: fixture.fixture_id,
			headId: head.id,
			attributes: sortedAttributes,
			signature: sortedAttributes.join("|"),
		});
	}
	return descriptors;
}

/**
 * Splits the units into `count` Dynamics. Units group by family and lane signature (one Dynamic
 * holds one lane set); instances are allotted to the heaviest groups by address load, and each
 * group's units are dealt round-robin over its instances.
 */
function partitionUnits(units, count) {
	const groups = new Map();
	for (const unit of units) {
		const signature = `${unit.family}:${unit.lanes.join("|")}`;
		const group = groups.get(signature) ?? [];
		group.push(unit);
		groups.set(signature, group);
	}
	if (groups.size > count)
		throw new Error(
			`Large Stage has ${groups.size} lane sets but only ${count} instances`,
		);
	const load = (signature) =>
		groups.get(signature).reduce((sum, unit) => sum + unit.lanes.length, 0);
	const allocations = new Map(
		[...groups.keys()].map((signature) => [signature, 1]),
	);
	while (
		[...allocations.values()].reduce((sum, value) => sum + value, 0) < count
	) {
		const signature = [...groups.keys()]
			.filter((key) => allocations.get(key) < groups.get(key).length)
			.sort((left, right) => {
				const leftLoad = load(left) / allocations.get(left);
				const rightLoad = load(right) / allocations.get(right);
				return rightLoad - leftLoad || left.localeCompare(right);
			})[0];
		if (!signature)
			throw new Error(`Large Stage has too few targets for ${count} instances`);
		allocations.set(signature, allocations.get(signature) + 1);
	}
	const buckets = [];
	for (const [signature, group] of [...groups.entries()].sort(
		([left], [right]) => left.localeCompare(right),
	)) {
		const allocated = Array.from(
			{ length: allocations.get(signature) },
			() => [],
		);
		group.forEach((unit, index) => {
			allocated[index % allocated.length].push(unit);
		});
		buckets.push(...allocated);
	}
	if (buckets.some((bucket) => bucket.length === 0))
		throw new Error("Large Stage Dynamic partition contains an empty instance");
	return buckets;
}

function dynamicDefinition(bucket, index) {
	const number = index + 1;
	const id = deterministicUuid("3", number);
	return {
		id,
		pool_number: 9_000 + number,
		revision: 1,
		name: `Stage capacity Dynamic ${String(number).padStart(2, "0")}`,
		color: null,
		icon: null,
		target_binding: { type: "targetless" },
		lanes: bucket[0].lanes.map((lane, laneIndex) =>
			dynamicLane(lane, number, laneIndex),
		),
		random_groups: [],
		phase_mode: "uniform",
		spatial_mapping: { projection: { type: "inherit" }, shape: { type: "inherit" } },
		phase: {
			ordering: { type: "selection" },
			offset_degrees: (index * 19) % 360,
			span_degrees: 360,
			block_size: 1,
			repeats: 1,
			wings: false,
			anchors_degrees: [],
		},
		speed: {
			type: "fixed",
			duration_millis: 2_400 + (index % 5) * 350,
		},
		overall_speed_multiplier: { numerator: 1, denominator: 1 },
		run_mode: "loop",
		default_activation: "start_now",
		activation_boundary: "beat",
	};
}

const PWM = Object.freeze({
	attack: 0,
	on: 0.5,
	decay: 0,
	off: 0.5,
	attack_interpolation: "linear",
	decay_interpolation: "linear",
});

/** The legacy sweep of each lane (normalized), and how it maps onto the lane's semantic unit. */
function laneSweep(lane) {
	if (lane === "intensity") return { minimum: 0.25, maximum: 0.9, scale: (value) => value };
	if (lane === "position.pan")
		return {
			minimum: 0.2,
			maximum: 0.8,
			scale: (value) => (value - 0.5) * LARGE_STAGE_PAN_TRAVEL_DEGREES,
		};
	if (lane === "position.tilt")
		return {
			minimum: 0.2,
			maximum: 0.8,
			scale: (value) => (value - 0.5) * LARGE_STAGE_TILT_TRAVEL_DEGREES,
		};
	return { minimum: 0.1, maximum: 1, scale: (value) => value };
}

function semanticAddress(lane) {
	if (lane === "position.pan" || lane === "position.tilt")
		return {
			representation: { kind: "angles" },
			component: { kind: lane.slice("position.".length) },
		};
	const component = lane.slice("color.".length);
	return {
		// UV is orthogonal to the recipe; it keeps whatever recipe the target holds.
		representation:
			component === "uv"
				? { kind: "semantic_color", basis: "retain" }
				: { kind: "semantic_color", basis: "recipe" },
		component: { kind: "color", component },
	};
}

function dynamicLane(lane, dynamicNumber, laneIndex) {
	const { minimum, maximum, scale } = laneSweep(lane);
	const shared = {
		id: deterministicUuid("4", dynamicNumber * 100 + laneIndex + 1),
		speed_multiplier: {
			numerator: laneIndex + 1,
			denominator: Math.max(1, laneIndex),
		},
		width: 1,
		random_group_id: null,
		phase: null,
	};
	const fn = laneIndex % 2 === 0 ? "sinus" : "cosinus";
	if (lane === "intensity")
		return {
			...shared,
			attribute: lane,
			mode: "max_min",
			keyframes: {
				points: [
					{ position: 0, source: { type: "value", value: minimum }, interpolation: "linear" },
					{ position: 0.5, source: { type: "value", value: maximum }, interpolation: "linear" },
				],
				size: 1,
			},
			max_min: {
				minimum: { type: "value", value: minimum },
				maximum: { type: "value", value: maximum },
				function: fn,
				size: 1,
				pwm: { ...PWM },
			},
			middle_amplitude: {
				middle: { type: "current" },
				amplitude: (maximum - minimum) / 2,
				function: "sinus",
				size: 1,
				pwm: { ...PWM },
			},
		};
	const value = (number) => ({
		kind: "value",
		value: { kind: "scalar", value: Math.round(scale(number) * 1e6) / 1e6 },
	});
	return {
		...shared,
		programming: {
			address: semanticAddress(lane),
			configuration: {
				mode: "max_min",
				configuration: {
					minimum: value(minimum),
					maximum: value(maximum),
					function: fn,
					size: 1,
					pwm: { ...PWM },
				},
			},
		},
	};
}

function deterministicUuid(namespace, value) {
	return `${namespace}0000000-0000-4000-8000-${value
		.toString(16)
		.padStart(12, "0")}`;
}
