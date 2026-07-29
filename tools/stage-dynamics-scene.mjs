import { LARGE_STAGE_DYNAMIC_INSTANCES } from "./stage-large-scene.mjs";

const DYNAMIC_ATTRIBUTES = new Set([
	"intensity",
	"color.red",
	"color.green",
	"color.blue",
	"color.cyan",
	"color.magenta",
	"color.yellow",
	"color.amber",
	"color.white",
	"color.uv",
	"pan",
	"tilt",
]);

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
	const buckets = partitionDescriptors(
		descriptors,
		LARGE_STAGE_DYNAMIC_INSTANCES,
	);
	const definitions = buckets.map((bucket, index) =>
		dynamicDefinition(bucket, index),
	);
	const allTargets = definitions.flatMap(
		(definition) => definition.target_binding.targets,
	);
	if (new Set(allTargets).size !== allTargets.length)
		throw new Error("Large Stage Dynamic target partitions overlap");
	return {
		definitions,
		targetDescriptors: descriptors,
		dynamicTargetCount: descriptors.length,
		staticControlFixtureIds: [...largeScene.staticControlFixtureIds],
		laneCoverage: Object.fromEntries(
			[...DYNAMIC_ATTRIBUTES]
				.map((attribute) => [
					attribute,
					descriptors.filter((descriptor) =>
						descriptor.attributes.includes(attribute),
					).length,
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
				.map((channel) => channel.attribute)
				.filter((attribute) => DYNAMIC_ATTRIBUTES.has(attribute)),
		);
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

function partitionDescriptors(descriptors, count) {
	const groups = new Map();
	for (const descriptor of descriptors) {
		const group = groups.get(descriptor.signature) ?? [];
		group.push(descriptor);
		groups.set(descriptor.signature, group);
	}
	if (groups.size > count)
		throw new Error(
			`Large Stage has ${groups.size} Dynamic lane signatures but only ${count} instances`,
		);
	const allocations = new Map(
		[...groups.keys()].map((signature) => [signature, 1]),
	);
	while (
		[...allocations.values()].reduce((sum, value) => sum + value, 0) < count
	) {
		const signature = [...groups.keys()].sort((left, right) => {
			const leftLoad = groups.get(left).length / allocations.get(left);
			const rightLoad = groups.get(right).length / allocations.get(right);
			return rightLoad - leftLoad || left.localeCompare(right);
		})[0];
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
		group.forEach((descriptor, index) => {
			allocated[index % allocated.length].push(descriptor);
		});
		buckets.push(...allocated);
	}
	if (buckets.some((bucket) => bucket.length === 0))
		throw new Error("Large Stage Dynamic partition contains an empty instance");
	return buckets;
}

function dynamicDefinition(bucket, index) {
	const attributes = bucket[0].attributes;
	const number = index + 1;
	return {
		id: deterministicUuid("3", number),
		pool_number: 9_000 + number,
		revision: 1,
		name: `Stage capacity Dynamic ${String(number).padStart(2, "0")}`,
		color: null,
		icon: null,
		target_binding: {
			type: "frozen_targets",
			targets: bucket.map((descriptor) => descriptor.target),
		},
		lanes: attributes.map((attribute, laneIndex) =>
			dynamicLane(attribute, number, laneIndex),
		),
		random_groups: [],
		phase_mode: "uniform",
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

function dynamicLane(attribute, dynamicNumber, laneIndex) {
	const [minimum, maximum] =
		attribute === "intensity"
			? [0.25, 0.9]
			: attribute === "pan" || attribute === "tilt"
				? [0.2, 0.8]
				: [0.1, 1];
	const pwm = {
		attack: 0,
		on: 0.5,
		decay: 0,
		off: 0.5,
		attack_interpolation: "linear",
		decay_interpolation: "linear",
	};
	return {
		id: deterministicUuid("4", dynamicNumber * 100 + laneIndex + 1),
		attribute,
		mode: "max_min",
		keyframes: {
			points: [
				{
					position: 0,
					source: { type: "value", value: minimum },
					interpolation: "linear",
				},
				{
					position: 0.5,
					source: { type: "value", value: maximum },
					interpolation: "linear",
				},
			],
			size: 1,
		},
		max_min: {
			minimum: { type: "value", value: minimum },
			maximum: { type: "value", value: maximum },
			function: laneIndex % 2 === 0 ? "sinus" : "cosinus",
			size: 1,
			pwm,
		},
		middle_amplitude: {
			middle: { type: "current" },
			amplitude: (maximum - minimum) / 2,
			function: "sinus",
			size: 1,
			pwm,
		},
		speed_multiplier: {
			numerator: laneIndex + 1,
			denominator: Math.max(1, laneIndex),
		},
		width: 1,
		random_group_id: null,
		phase: null,
	};
}

function deterministicUuid(namespace, value) {
	return `${namespace}0000000-0000-4000-8000-${value
		.toString(16)
		.padStart(12, "0")}`;
}
