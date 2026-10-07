#!/usr/bin/env node
// Deterministic semantic performance workloads for TL-548 scheduler verification and TL-553
// performance acceptance (TL-564). The builder consumes a PatchSnapshot-shaped patch (the live
// `/api/v2/patch` snapshot, or the synthetic rig from semantic-performance-fixtures.mjs) and
// returns schema-valid Dynamic definitions, static programmer bases, deterministic tracking
// inputs and an explicit requested-versus-realized manifest. It never starts, measures or
// accepts anything; runner integration and acceptance belong to the TL-548 runtime owner.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
	SEMANTIC_CONTRACT_SCHEMAS,
	angleCurrentPartnerLaneId,
	canonicalJson,
	contractIdentity,
	createSeededRandom,
	normalizeSeed,
	quantize,
	semanticWorkloadDirectory,
	sha256,
	uuidV5,
	validateSemanticDefinition,
	validateWireDefinition,
	workloadNamespace,
} from "./semantic-performance-contract.mjs";
import {
	DEFAULT_MOUNT_PLAN,
	DEFAULT_SEMANTIC_RIG,
	classifyModeHeads,
	createSyntheticSemanticPatch,
	isPointProfile,
	loadFixtureLibrary,
} from "./semantic-performance-fixtures.mjs";

export const SEMANTIC_WORKLOAD_VERSION = "tosklight.semantic-performance-workload/1";
export const SEMANTIC_TRACKING_RATES_HZ = Object.freeze([30, 60, 120]);
export const DEFAULT_OUTPUT_RATES_HZ = Object.freeze([44, 60, 125]);
export const SEMANTIC_DYNAMIC_POOL_BASE = 9_500;
const COLOR_CLASSES = Object.freeze(["rgb", "rgbw", "cmy", "wheel"]);

export const DEFAULT_SEMANTIC_REQUEST = Object.freeze({
	angle: 24,
	position: Object.freeze({
		fixed: 6,
		referenced: 6,
		sharedMountGroups: DEFAULT_MOUNT_PLAN.sharedGroups,
		sharedMountGroupSize: DEFAULT_MOUNT_PLAN.sharedGroupSize,
		separateMounts: DEFAULT_MOUNT_PLAN.separateMounts,
	}),
	color: Object.freeze({ rgb: 24, rgbw: 24, cmy: 12, wheel: 12 }),
	uv: 16,
	focus: 24,
	zoom: 12,
	dirtySubsetPoints: 1,
	trackingRatesHz: SEMANTIC_TRACKING_RATES_HZ,
	outputRatesHz: DEFAULT_OUTPUT_RATES_HZ,
	trackingDurationSeconds: 10,
});

// ---------------------------------------------------------------------------------------------
// Request normalization

function count(value, label) {
	if (!Number.isSafeInteger(value) || value < 0)
		throw new Error(`${label} must be a nonnegative integer`);
	return value;
}

function rates(values, label) {
	if (!Array.isArray(values) || values.length === 0)
		throw new Error(`${label} must list at least one rate`);
	const normalized = values.map((value) => {
		if (!Number.isFinite(value) || value <= 0 || value > 1_000)
			throw new Error(`${label} rates must be within (0, 1000] Hz`);
		return quantize(value, 3);
	});
	return [...new Set(normalized)].sort((left, right) => left - right);
}

export function normalizeSemanticRequest(request = {}) {
	const merged = {
		...DEFAULT_SEMANTIC_REQUEST,
		...request,
		position: { ...DEFAULT_SEMANTIC_REQUEST.position, ...request.position },
		color: { ...DEFAULT_SEMANTIC_REQUEST.color, ...request.color },
	};
	const duration = merged.trackingDurationSeconds;
	if (!Number.isFinite(duration) || duration <= 0 || duration > 3_600)
		throw new Error("trackingDurationSeconds must be within (0, 3600]");
	return {
		angle: count(merged.angle, "angle"),
		position: Object.fromEntries(
			Object.entries(merged.position).map(([key, value]) => [key, count(value, `position.${key}`)]),
		),
		color: Object.fromEntries(
			COLOR_CLASSES.map((key) => [key, count(merged.color[key] ?? 0, `color.${key}`)]),
		),
		uv: count(merged.uv, "uv"),
		focus: count(merged.focus, "focus"),
		zoom: count(merged.zoom, "zoom"),
		dirtySubsetPoints: count(merged.dirtySubsetPoints, "dirtySubsetPoints"),
		trackingRatesHz: rates(merged.trackingRatesHz, "tracking"),
		outputRatesHz: rates(merged.outputRatesHz, "output"),
		trackingDurationSeconds: quantize(duration, 3),
	};
}

// ---------------------------------------------------------------------------------------------
// Patch resolution: actual root and logical-head identities

/** Resolve every head of the patch to its programmable target and semantic capabilities. */
export function resolveSemanticPatchTargets(patch) {
	const revisions = new Map(
		patch.profile_revisions.map((revision) => [`${revision.profile_id}:${revision.profile_revision}`, revision]),
	);
	const roots = [];
	const points = [];
	const unresolved = [];
	for (const fixture of patch.fixtures) {
		const revision = revisions.get(`${fixture.profile_id}:${fixture.profile_revision}`);
		const profile = revision?.profile_snapshot;
		if (isPointProfile(profile) || isPointProfile(revision)) {
			points.push(fixture.fixture_id);
			continue;
		}
		if (!profile?.modes) {
			unresolved.push({ fixtureId: fixture.fixture_id, reason: "profile snapshot unavailable" });
			continue;
		}
		if (classifyModeHeads(profile, fixture.mode_id).some((head) => head.point)) {
			points.push(fixture.fixture_id);
			continue;
		}
		const heads = classifyModeHeads(profile, fixture.mode_id).map((head) => {
			const logical = head.masterShared
				? null
				: fixture.logical_heads.find(
						(candidate) =>
							candidate.profile_head_id === head.headId ||
							(candidate.profile_head_id == null && candidate.head_index === head.headIndex),
					);
			if (!head.masterShared && !logical)
				throw new Error(
					`${fixture.name ?? fixture.fixture_id} has no logical head for ${head.headName}`,
				);
			return {
				...head,
				target: head.masterShared ? fixture.fixture_id : logical.fixture_id,
				rootFixtureId: fixture.fixture_id,
				logical: !head.masterShared,
			};
		});
		roots.push({
			fixtureId: fixture.fixture_id,
			fixtureNumber: fixture.fixture_number ?? null,
			profileKey: `${fixture.profile_id}:${fixture.profile_revision}`,
			profileName: profile.name,
			positionMaster: fixture.position_master ?? null,
			heads,
		});
	}
	const pointIds = new Set(points);
	const mountDependents = new Map();
	for (const root of roots) {
		if (!root.positionMaster) continue;
		if (!pointIds.has(root.positionMaster)) {
			unresolved.push({ fixtureId: root.fixtureId, reason: "position_master is not a Point in this patch" });
			root.positionMaster = null;
			continue;
		}
		mountDependents.set(root.positionMaster, [
			...(mountDependents.get(root.positionMaster) ?? []),
			root.fixtureId,
		]);
	}
	return {
		roots,
		points: [...points].sort(),
		mountDependents,
		heads: roots.flatMap((root) => root.heads),
		unresolved,
	};
}

// ---------------------------------------------------------------------------------------------
// Definition builders

const PWM = Object.freeze({
	attack: 0,
	on: 0.5,
	decay: 0,
	off: 0.5,
	attack_interpolation: "linear",
	decay_interpolation: "linear",
});
const value = (number) => ({ kind: "value", value: { kind: "scalar", value: quantize(number) } });

function lane(id, representation, component, configuration) {
	return {
		id,
		speed_multiplier: { numerator: 1, denominator: 1 },
		width: 1,
		random_group_id: null,
		phase: null,
		programming: {
			address: { representation, component },
			configuration,
		},
	};
}

const maxMin = (minimum, maximum, fn = "sinus") => ({
	mode: "max_min",
	configuration: { minimum: value(minimum), maximum: value(maximum), function: fn, size: 1, pwm: { ...PWM } },
});
const middleAmplitude = (amplitude, fn = "sinus") => ({
	mode: "middle_amplitude",
	configuration: {
		middle: { kind: "current" },
		amplitude: { kind: "scalar", value: quantize(amplitude) },
		function: fn,
		size: 1,
		pwm: { ...PWM },
		invert_waveform: false,
	},
});
const keyframes = (points, interpolation = "linear") => ({
	mode: "keyframes",
	configuration: {
		points: points.map(([position, source]) => ({ position, source, interpolation })),
		size: 1,
	},
});

/** The saved automatic partner of an Angle Dynamic: a two-point static `Current` lane. */
export function angleCurrentPartnerLane(definitionId, axis) {
	return lane(
		angleCurrentPartnerLaneId(definitionId, axis),
		{ kind: "angles" },
		{ kind: axis },
		keyframes([
			[0, { kind: "current" }],
			[0.5, { kind: "current" }],
		]),
	);
}

function definition(namespace, key, index, name, lanes) {
	const id = uuidV5(namespace, `dynamic:${key}`);
	return {
		id,
		pool_number: SEMANTIC_DYNAMIC_POOL_BASE + index + 1,
		revision: 1,
		name,
		color: null,
		icon: null,
		target_binding: { type: "targetless" },
		lanes: lanes(id),
		random_groups: [],
		phase_mode: "uniform",
		spatial_mapping: { projection: { type: "inherit" }, shape: { type: "inherit" } },
		phase: {
			ordering: { type: "selection" },
			offset_degrees: (index * 23) % 360,
			span_degrees: 360,
			block_size: 1,
			repeats: 1,
			wings: false,
			anchors_degrees: [],
		},
		speed: { type: "fixed", duration_millis: 2_000 + (index % 7) * 300 },
		overall_speed_multiplier: { numerator: 1, denominator: 1 },
		run_mode: "loop",
		default_activation: "start_now",
		activation_boundary: "beat",
	};
}

const authored = (id, n) => uuidV5(id, `lane:${n}`);

/** Lane templates per workload family. Every Angle template yields a complete Pan/Tilt pair. */
export const SEMANTIC_DYNAMIC_TEMPLATES = Object.freeze({
	"angle-pan": (id) => [
		lane(authored(id, 0), { kind: "angles" }, { kind: "pan" }, maxMin(-90, 90)),
		angleCurrentPartnerLane(id, "tilt"),
	],
	"angle-tilt": (id) => [
		lane(authored(id, 0), { kind: "angles" }, { kind: "tilt" }, maxMin(-45, 45, "cosinus")),
		angleCurrentPartnerLane(id, "pan"),
	],
	"angle-pair": (id) => [
		lane(authored(id, 0), { kind: "angles" }, { kind: "pan" }, maxMin(-60, 60)),
		lane(authored(id, 1), { kind: "angles" }, { kind: "tilt" }, maxMin(-30, 30, "cosinus")),
	],
	target: (reference, amplitude, component = "target_x") => (id) => [
		lane(authored(id, 0), { kind: "target", reference }, { kind: component }, middleAmplitude(amplitude)),
	],
	"color-rgb": (id) => [
		lane(authored(id, 0), { kind: "semantic_color", basis: "recipe" }, { kind: "color", component: "red" }, maxMin(0.1, 1)),
		lane(authored(id, 1), { kind: "semantic_color", basis: "recipe" }, { kind: "color", component: "blue" }, maxMin(0, 0.8, "cosinus")),
	],
	"color-rgbw": (id) => [
		lane(authored(id, 0), { kind: "semantic_color", basis: "recipe" }, { kind: "color", component: "green" }, maxMin(0.2, 1)),
		lane(authored(id, 1), { kind: "semantic_color", basis: "recipe" }, { kind: "color", component: "white_blend" }, maxMin(0, 0.6, "cosinus")),
	],
	"color-cmy": (id) => [
		lane(authored(id, 0), { kind: "semantic_color", basis: "hue_saturation" }, { kind: "color", component: "hue" }, maxMin(20, 340, "linear_up")),
		lane(authored(id, 1), { kind: "semantic_color", basis: "hue_saturation" }, { kind: "color", component: "saturation" }, maxMin(0.5, 1)),
	],
	"color-wheel": (id) => [
		lane(
			authored(id, 0),
			{ kind: "semantic_color", basis: "hue_saturation" },
			{ kind: "color", component: "hue" },
			keyframes([[0, value(0)], [0.25, value(90)], [0.5, value(180)], [0.75, value(270)]], "hold"),
		),
	],
	uv: (id) => [
		lane(authored(id, 0), { kind: "semantic_color", basis: "retain" }, { kind: "color", component: "uv" }, maxMin(0, 1, "pwm")),
	],
	focus: (id) => [lane(authored(id, 0), { kind: "focus" }, { kind: "focus" }, maxMin(0.2, 0.8))],
	zoom: (convention, minimum, maximum) => (id) => [
		lane(authored(id, 0), { kind: "zoom", convention }, { kind: "zoom" }, maxMin(minimum, maximum)),
	],
});

// ---------------------------------------------------------------------------------------------
// Static programmer bases (the `Current` sources the Dynamics animate around)

const timing = Object.freeze({ fade: false, fade_millis: null, delay_millis: null });
const scalar = (number) => ({ kind: "value", value: quantize(number) });

function srgbLinear(channel) {
	return channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4;
}

/** Semantic Color static value with exact base XYZ (mirrors VirtualColorAuthoringV1). */
export function semanticColorValue([red, green, blue]) {
	const [r, g, b] = [red, green, blue].map(srgbLinear);
	return {
		kind: "color_program",
		value: {
			kind: "semantic",
			intent: {
				base_xyz: {
					x: quantize(0.4124564 * r + 0.3575761 * g + 0.1804375 * b),
					y: quantize(0.2126729 * r + 0.7151522 * g + 0.072175 * b),
					z: quantize(0.0193339 * r + 0.119192 * g + 0.9503041 * b),
				},
				recipe: { version: 1, rgb: [red, green, blue], amber: 0, approximate: false },
				white_blend: 0,
				white_target: { kelvin: 6500, duv: 0 },
				uv: { amount: 0 },
				relative_output: 1,
				allocation: "preserve_recipe",
			},
		},
	};
}

const COLOR_PALETTE = Object.freeze([
	[1, 0, 0],
	[0, 1, 0],
	[0, 0, 1],
	[1, 1, 0],
	[0, 1, 1],
	[1, 0, 1],
	[1, 1, 1],
]);

function staticMutation(target, attribute, attributeValue) {
	return { type: "set_fixture", fixture_id: target, attribute, value: attributeValue, timing: { ...timing } };
}

// ---------------------------------------------------------------------------------------------
// Workload assembly

/** Seeded choice of `requested` heads; heads matching `prefer` are chosen first. */
function pick(random, candidates, requested, prefer = () => false) {
	const ordered = [...candidates].sort((left, right) => left.target.localeCompare(right.target));
	const shuffled = random.shuffle(ordered);
	return [...shuffled.filter(prefer), ...shuffled.filter((head) => !prefer(head))]
		.slice(0, requested)
		.sort((left, right) => left.target.localeCompare(right.target));
}

function coverageStatus(requested, realized) {
	if (requested === 0) return "not-requested";
	if (realized === 0) return "missing";
	return realized < requested ? "partial" : "covered";
}

function planPositionScenarios(resolved, request, seed) {
	const random = createSeededRandom(seed, "aim-points");
	const angleHeads = resolved.heads.filter((head) => head.angle);
	const rootOf = new Map(resolved.roots.map((root) => [root.fixtureId, root]));
	const mountOf = (head) => rootOf.get(head.rootFixtureId).positionMaster;
	const shared = [...resolved.mountDependents.entries()]
		.filter(([, dependents]) => dependents.length >= 2)
		.map(([point]) => point)
		.sort();
	const separate = [...resolved.mountDependents.entries()]
		.filter(([, dependents]) => dependents.length === 1)
		.map(([point]) => point)
		.sort();
	const mountPoints = new Set(resolved.mountDependents.keys());
	const aimPool = random.shuffle(resolved.points.filter((point) => !mountPoints.has(point)).sort());
	const takeAim = () => aimPool.shift() ?? null;
	const unmounted = angleHeads.filter((head) => !mountOf(head));
	const positionRandom = createSeededRandom(seed, "position");
	const shuffledUnmounted = pick(positionRandom, unmounted, unmounted.length);
	const fixed = shuffledUnmounted.slice(0, request.position.fixed);
	const referencedTargets = shuffledUnmounted.slice(fixed.length, fixed.length + request.position.referenced);
	const referencedAim = referencedTargets.length > 0 ? takeAim() : null;
	const scenarios = [];
	scenarios.push({ key: "target-fixed", kind: "fixed", reference: { kind: "origin" }, mountPoint: null, aimPoint: null, heads: fixed, requested: request.position.fixed });
	scenarios.push({
		key: "target-referenced",
		kind: "referenced",
		reference: referencedAim ? { kind: "point", point_id: referencedAim } : null,
		mountPoint: null,
		aimPoint: referencedAim,
		heads: referencedAim ? referencedTargets : [],
		requested: request.position.referenced,
	});
	for (let group = 0; group < request.position.sharedMountGroups; group += 1) {
		const mount = shared[group] ?? null;
		const heads = mount
			? angleHeads.filter((head) => mountOf(head) === mount).slice(0, request.position.sharedMountGroupSize)
			: [];
		const aim = heads.length > 0 ? takeAim() : null;
		scenarios.push({
			key: `target-shared-mount-${group + 1}`,
			kind: "shared-moving-mount",
			reference: aim ? { kind: "point", point_id: aim } : null,
			mountPoint: mount,
			aimPoint: aim,
			heads: aim ? heads : [],
			requested: request.position.sharedMountGroupSize,
		});
	}
	for (let index = 0; index < request.position.separateMounts; index += 1) {
		const mount = separate[index] ?? null;
		const heads = mount ? angleHeads.filter((head) => mountOf(head) === mount).slice(0, 1) : [];
		const aim = heads.length > 0 ? takeAim() : null;
		scenarios.push({
			key: `target-separate-mount-${index + 1}`,
			kind: "separate-moving-mount",
			reference: aim ? { kind: "point", point_id: aim } : null,
			mountPoint: mount,
			aimPoint: aim,
			heads: aim ? heads : [],
			requested: 1,
		});
	}
	const used = new Set(scenarios.flatMap((scenario) => scenario.heads.map((head) => head.target)));
	const angleCandidates = unmounted.filter((head) => !used.has(head.target));
	return { scenarios, angleCandidates };
}

function buildFamilies(resolved, request, seed) {
	const random = createSeededRandom(seed, "selection");
	const { scenarios, angleCandidates } = planPositionScenarios(resolved, request, seed);
	const angleHeads = pick(random, angleCandidates, request.angle);
	const families = [];
	const variants = ["angle-pan", "angle-tilt", "angle-pair"];
	variants.forEach((variant, index) => {
		const heads = angleHeads.filter((_, position) => position % variants.length === index);
		if (heads.length > 0)
			families.push({ key: variant, family: "angle", template: SEMANTIC_DYNAMIC_TEMPLATES[variant], heads });
	});
	for (const scenario of scenarios) {
		if (scenario.heads.length === 0) continue;
		const amplitude = scenario.kind === "fixed" ? 0.75 : 0.25;
		const component = scenario.kind === "shared-moving-mount" ? "target_y" : "target_x";
		families.push({
			key: scenario.key,
			family: `position-${scenario.kind}`,
			template: SEMANTIC_DYNAMIC_TEMPLATES.target(scenario.reference, amplitude, component),
			heads: scenario.heads,
			scenario,
		});
	}
	for (const colorClass of COLOR_CLASSES) {
		// Prefer UV-capable heads so the independent UV Dynamic composes with Color intent.
		const heads = pick(random, resolved.heads.filter((head) => head.colorClass === colorClass), request.color[colorClass], (head) => head.uv);
		if (heads.length > 0)
			families.push({ key: `color-${colorClass}`, family: `color-${colorClass}`, template: SEMANTIC_DYNAMIC_TEMPLATES[`color-${colorClass}`], heads });
	}
	const uvHeads = pick(random, resolved.heads.filter((head) => head.uv), request.uv);
	if (uvHeads.length > 0) families.push({ key: "uv", family: "uv", template: SEMANTIC_DYNAMIC_TEMPLATES.uv, heads: uvHeads });
	const focusHeads = pick(random, resolved.heads.filter((head) => head.focus), request.focus);
	if (focusHeads.length > 0) families.push({ key: "focus", family: "focus", template: SEMANTIC_DYNAMIC_TEMPLATES.focus, heads: focusHeads });
	const zoomHeads = pick(random, resolved.heads.filter((head) => head.zoom?.physical === "degrees"), request.zoom);
	if (zoomHeads.length > 0)
		families.push({ key: "zoom", family: "zoom", template: SEMANTIC_DYNAMIC_TEMPLATES.zoom("beam", 12, 22), heads: zoomHeads });
	return { families, scenarios, angleHeads };
}

function staticBases(families, seed) {
	const random = createSeededRandom(seed, "static-bases");
	const mutations = new Map();
	const set = (target, attribute, attributeValue) => {
		if (!mutations.has(`${target}:${attribute}`))
			mutations.set(`${target}:${attribute}`, staticMutation(target, attribute, attributeValue));
	};
	for (const family of families) {
		for (const head of family.heads) {
			if (family.family === "angle")
				set(head.target, "position", {
					kind: "position",
					value: { kind: "angles", pan_degrees: scalar(random.range(-30, 30)), tilt_degrees: scalar(random.range(0, 30)) },
				});
			else if (family.family.startsWith("position-")) {
				const offset = family.scenario.kind === "fixed"
					? [random.range(-4, 4), random.range(-4, 4), 0]
					: [0, 0, 0];
				set(head.target, "position", {
					kind: "position",
					value: { kind: "target", reference: family.scenario.reference, offset_metres: offset.map(scalar) },
				});
			} else if (family.family.startsWith("color-") || family.family === "uv")
				set(head.target, "color", semanticColorValue(COLOR_PALETTE[random.integer(0, COLOR_PALETTE.length - 1)]));
			else if (family.family === "focus") set(head.target, "focus", { kind: "normalized", value: 0.5 });
			else if (family.family === "zoom")
				set(head.target, "zoom", { kind: "zoom", value: { opening_degrees: scalar(16), convention: "beam" } });
		}
	}
	return [...mutations.values()].sort((left, right) =>
		`${left.fixture_id}:${left.attribute}`.localeCompare(`${right.fixture_id}:${right.attribute}`),
	);
}

function laneCoverage(definitions, activations) {
	const coverage = {};
	const targetsByDefinition = new Map(activations.map((activation) => [activation.dynamicId, activation.targets.length]));
	for (const item of definitions) {
		for (const entry of item.lanes) {
			const { representation, component } = entry.programming.address;
			const configuration = entry.programming.configuration;
			const sources = configuration.mode === "keyframes"
				? [...new Set(configuration.configuration.points.map((point) => point.source.kind))]
				: configuration.mode === "middle_amplitude" ? ["current"] : ["value"];
			const key = [
				representation.kind + (representation.basis ? `.${representation.basis}` : ""),
				component ? component.component ?? component.kind : "whole",
				sources.join("+"),
			].join("/");
			coverage[key] ??= { lanes: 0, addresses: 0 };
			coverage[key].lanes += 1;
			coverage[key].addresses += targetsByDefinition.get(item.id) ?? 0;
		}
	}
	return Object.fromEntries(Object.entries(coverage).sort(([left], [right]) => left.localeCompare(right)));
}

// ---------------------------------------------------------------------------------------------
// Dirty-target scenarios and deterministic tracking inputs

function dirtyScenarios(scenarios, request, seed) {
	const dependents = new Map();
	for (const scenario of scenarios)
		for (const point of [scenario.mountPoint, scenario.aimPoint].filter(Boolean))
			for (const head of scenario.heads)
				dependents.set(point, new Set([...(dependents.get(point) ?? []), head.target]));
	const points = [...dependents.keys()].sort();
	const positionTargets = new Set(scenarios.flatMap((scenario) => scenario.heads.map((head) => head.target)));
	const dirtyFor = (moving) => new Set(moving.flatMap((point) => [...dependents.get(point)]));
	const subset = createSeededRandom(seed, "dirty-subset").shuffle(points).slice(0, request.dirtySubsetPoints).sort();
	const describe = (key, moving, requestedPoints) => {
		const dirty = dirtyFor(moving);
		return {
			key,
			requestedMovingPoints: requestedPoints,
			movingPointIds: moving,
			dirtyTargetCount: dirty.size,
			positionTargetCount: positionTargets.size,
			dirtyTargetIds: [...dirty].sort(),
		};
	};
	return {
		points,
		dependents: Object.fromEntries(points.map((point) => [point, [...dependents.get(point)].sort()])),
		scenarios: [
			describe("static-points", [], 0),
			describe("small-subset", subset, request.dirtySubsetPoints),
			describe("all-points-move", points, points.length),
		],
	};
}

function trackingMotion(points, seed) {
	const random = createSeededRandom(seed, "tracking-motion");
	return points.map((pointId) => ({
		pointId,
		center: [random.range(-6, 6), random.range(-6, 6), random.range(0, 8)].map((number) => quantize(number, 3)),
		amplitude: quantize(random.range(0.5, 2), 3),
		frequencyHz: quantize(random.range(0.05, 0.5), 4),
		phase: quantize(random.range(0, 2 * Math.PI), 6),
	}));
}

/**
 * Frames of deterministic Point tracking input. Every Point is emitted every frame (as a PSN
 * source would); only the scenario's moving Points change, static Points repeat exactly.
 */
export function* trackingFrames(workload, { scenario = "all-points-move", rateHz }) {
	const plan = workload.tracking;
	if (!plan.ratesHz.includes(rateHz)) throw new Error(`tracking rate ${rateHz} Hz is not configured`);
	const dirty = workload.dirty.scenarios.find((candidate) => candidate.key === scenario);
	if (!dirty) throw new Error(`unknown dirty scenario ${scenario}`);
	const moving = new Set(dirty.movingPointIds);
	const frames = Math.round(plan.durationSeconds * rateHz);
	for (let sequence = 0; sequence < frames; sequence += 1) {
		const seconds = sequence / rateHz;
		yield {
			sequence,
			t_micros: Math.round((sequence * 1_000_000) / rateHz),
			points: plan.motion.map((motion) => {
				const t = moving.has(motion.pointId) ? seconds : 0;
				const angle = 2 * Math.PI * motion.frequencyHz * t + motion.phase;
				return {
					point_id: motion.pointId,
					position_metres: [
						quantize(motion.center[0] + motion.amplitude * Math.sin(angle)),
						quantize(motion.center[1] + motion.amplitude * 0.5 * Math.sin(1.7 * angle)),
						quantize(motion.center[2] + 0.25 * Math.sin(0.6 * angle)),
					],
				};
			}),
		};
	}
}

function trackingDigests(workload) {
	const entries = [];
	for (const scenario of workload.dirty.scenarios)
		for (const rateHz of workload.tracking.ratesHz) {
			let frameCount = 0;
			let digest = "";
			for (const frame of trackingFrames(workload, { scenario: scenario.key, rateHz })) {
				digest = sha256(`${digest}${canonicalJson(frame)}`);
				frameCount += 1;
			}
			entries.push({
				scenario: scenario.key,
				rateHz,
				frameCount,
				sampleCount: frameCount * workload.tracking.motion.length,
				sha256: digest,
			});
		}
	return entries;
}

// ---------------------------------------------------------------------------------------------
// Public builder

/**
 * Build the complete workload for one seed. `patch` is a PatchSnapshot-shaped value; its
 * `identity_source` defaults to `live-patch` when absent.
 */
export function buildSemanticPerformanceWorkload(patch, { seed, request: rawRequest, productionSupport } = {}) {
	const normalizedSeed = normalizeSeed(seed);
	const request = normalizeSemanticRequest(rawRequest);
	const namespace = workloadNamespace(normalizedSeed);
	const resolved = resolveSemanticPatchTargets(patch);
	const { families, scenarios } = buildFamilies(resolved, request, normalizedSeed);
	const definitions = families.map((family, index) =>
		definition(namespace, family.key, index, `Semantic ${family.key}`, family.template),
	);
	const activations = families.map((family, index) => ({
		key: family.key,
		family: family.family,
		dynamicId: definitions[index].id,
		poolNumber: definitions[index].pool_number,
		targets: family.heads.map((head) => head.target),
		start: {
			targets: family.heads.map((head) => head.target),
			overrides: { size: 1, speed_multiplier: { numerator: 1, denominator: 1 }, phase_offset_degrees: 0 },
			timing: {},
		},
	}));
	const staticValues = staticBases(families, normalizedSeed);
	const dirty = dirtyScenarios(scenarios, request, normalizedSeed);
	const workload = {
		definitions,
		activations,
		staticValues,
		positionScenarios: scenarios.map((scenario) => ({
			key: scenario.key,
			kind: scenario.kind,
			reference: scenario.reference,
			mountPoint: scenario.mountPoint,
			aimPoint: scenario.aimPoint,
			requestedTargets: scenario.requested,
			targets: scenario.heads.map((head) => head.target),
		})),
		dirty,
		tracking: {
			ratesHz: request.trackingRatesHz,
			durationSeconds: request.trackingDurationSeconds,
			motion: trackingMotion(dirty.points, normalizedSeed),
			staticPointsEmitted: true,
		},
	};
	workload.manifest = buildManifest({ patch, resolved, request, families, workload, seed: normalizedSeed, namespace, productionSupport });
	return workload;
}

function capabilityAvailability(resolved) {
	const heads = resolved.heads;
	return {
		angle: heads.filter((head) => head.angle).length,
		...Object.fromEntries(COLOR_CLASSES.map((colorClass) => [`color.${colorClass}`, heads.filter((head) => head.colorClass === colorClass).length])),
		"color.hybridWheel": heads.filter((head) => head.colorWheel && head.colorClass !== "wheel" && head.colorClass).length,
		uv: heads.filter((head) => head.uv).length,
		focus: heads.filter((head) => head.focus).length,
		"zoom.physicalDegrees": heads.filter((head) => head.zoom?.physical === "degrees").length,
		"zoom.unmapped": heads.filter((head) => head.zoom?.physical === "unmapped").length,
	};
}

function buildManifest({ patch, resolved, request, families, workload, seed, namespace, productionSupport }) {
	const realizedFamily = (name) => families.filter((family) => family.family === name).reduce((sum, family) => sum + family.heads.length, 0);
	const scenarioRealized = (kind) => workload.positionScenarios.filter((scenario) => scenario.kind === kind).reduce((sum, scenario) => sum + scenario.targets.length, 0);
	const sharedRequested = request.position.sharedMountGroups * request.position.sharedMountGroupSize;
	const capabilities = {
		angle: [request.angle, realizedFamily("angle")],
		"position.fixed": [request.position.fixed, scenarioRealized("fixed")],
		"position.referenced": [request.position.referenced, scenarioRealized("referenced")],
		"position.sharedMovingMount": [sharedRequested, scenarioRealized("shared-moving-mount")],
		"position.separateMovingMount": [request.position.separateMounts, scenarioRealized("separate-moving-mount")],
		...Object.fromEntries(COLOR_CLASSES.map((colorClass) => [`color.${colorClass}`, [request.color[colorClass], realizedFamily(`color-${colorClass}`)]])),
		uv: [request.uv, realizedFamily("uv")],
		focus: [request.focus, realizedFamily("focus")],
		"zoom.physicalDegrees": [request.zoom, realizedFamily("zoom")],
	};
	const capabilityMix = Object.fromEntries(
		Object.entries(capabilities).map(([key, [requested, realized]]) => [key, { requested, realized, status: coverageStatus(requested, realized) }]),
	);
	const uvTargets = new Set(families.find((family) => family.family === "uv")?.heads.map((head) => head.target) ?? []);
	const colorTargets = new Set(families.filter((family) => family.family.startsWith("color-")).flatMap((family) => family.heads.map((head) => head.target)));
	const requestedDynamics = Math.min(3, request.angle) + (request.position.fixed > 0) + (request.position.referenced > 0) +
		request.position.sharedMountGroups + request.position.separateMounts +
		COLOR_CLASSES.filter((colorClass) => request.color[colorClass] > 0).length + (request.uv > 0) + (request.focus > 0) + (request.zoom > 0);
	const targets = new Set(workload.activations.flatMap((activation) => activation.targets));
	const zoomFamily = families.find((family) => family.family === "zoom");
	const limitations = [];
	if (zoomFamily?.heads.some((head) => !head.zoom.convention || head.zoom.quality === "unknown"))
		limitations.push({
			kind: "zoom-physical-model",
			reason: "Zoom targets have degree functions without a verified opening convention or with unknown quality; the workload assumes the beam convention",
			targets: zoomFamily.heads.filter((head) => !head.zoom.convention || head.zoom.quality === "unknown").length,
		});
	const lanes = workload.definitions.reduce((sum, item) => sum + item.lanes.length, 0);
	const manifest = {
		workloadVersion: SEMANTIC_WORKLOAD_VERSION,
		seed,
		workloadNamespace: namespace,
		workloadId: uuidV5(namespace, canonicalJson({ version: SEMANTIC_WORKLOAD_VERSION, request })),
		identitySource: patch.identity_source ?? "live-patch",
		patchIdentity: {
			showId: patch.show_id ?? null,
			patchRevision: patch.patch_revision ?? null,
			fixtureIdsSha256: sha256(patch.fixtures.map((fixture) => fixture.fixture_id).sort()),
		},
		contract: contractIdentity(),
		fixtureLibrary: patch.rig
			? { packages: patch.rig.packages, rigEntries: patch.rig.entries, mountPlan: patch.rig.mounts, rigShortfalls: patch.rig.shortfalls }
			: { status: "unavailable", reason: "live patch: identities were read from the desk, not from packages" },
		request,
		counts: {
			rootFixtures: {
				requested: patch.rig ? patch.rig.entries.reduce((sum, entry) => sum + entry.count, 0) : { status: "unavailable", reason: "live patch has no rig request" },
				actual: resolved.roots.length,
			},
			logicalHeads: { actual: resolved.heads.filter((head) => head.logical).length },
			programmableHeads: { actual: resolved.heads.length },
			points: {
				requested: patch.rig ? patch.rig.mounts.sharedGroups + patch.rig.mounts.separateMounts + patch.rig.mounts.aimPoints : { status: "unavailable", reason: "live patch has no rig request" },
				actual: resolved.points.length,
				usedByWorkload: workload.dirty.points.length,
			},
			dynamics: { requested: requestedDynamics, actual: workload.definitions.length },
			lanes: { actual: lanes },
			targets: { actual: targets.size, rootTargets: [...targets].filter((target) => resolved.roots.some((root) => root.fixtureId === target)).length },
			staticValues: { actual: workload.staticValues.length },
			unresolvedFixtures: resolved.unresolved,
		},
		capabilityMix,
		capabilityAvailability: capabilityAvailability(resolved),
		mixedColorWithUv: { uvTargets: uvTargets.size, uvTargetsWithColorIntentDynamic: [...uvTargets].filter((target) => colorTargets.has(target)).length },
		laneCoverage: laneCoverage(workload.definitions, workload.activations),
		angleDynamics: workload.definitions
			.filter((item) => item.lanes.some((entry) => entry.programming.address.representation.kind === "angles"))
			.map((item) => ({
				dynamicId: item.id,
				axes: item.lanes.map((entry) => entry.programming.address.component.kind).sort(),
				currentPartners: item.lanes.filter((entry) => entry.id === angleCurrentPartnerLaneId(item.id, entry.programming.address.component.kind)).map((entry) => entry.programming.address.component.kind),
			})),
		dirtyScenarios: workload.dirty.scenarios.map(({ dirtyTargetIds, ...scenario }) => ({ ...scenario, dirtyTargetIdsSha256: sha256(dirtyTargetIds) })),
		rates: {
			kind: "configured-targets",
			trackingHz: request.trackingRatesHz,
			outputHz: request.outputRatesHz,
			matrix: request.trackingRatesHz.flatMap((trackingHz) => request.outputRatesHz.map((outputHz) => ({ trackingHz, outputHz }))),
			note: "Configured targets only. Observed sample, output and native Stage presentation rates belong to a report.",
		},
		tracking: { durationSeconds: request.trackingDurationSeconds, staticPointsEmitted: true, streams: trackingDigests(workload) },
		productionSupport: productionSupport ?? {
			semanticProgrammingContract: { status: "unavailable", reason: "not probed by the builder; production accepts contract 0 until TL-548 integration gates pass" },
		},
		limitations,
		shortfalls: [
			...Object.entries(capabilityMix)
				.filter(([, entry]) => entry.status === "partial" || entry.status === "missing")
				.map(([key, entry]) => ({ kind: "capability", key, requested: entry.requested, realized: entry.realized })),
			...(patch.rig?.shortfalls ?? []),
			...(resolved.unresolved.length > 0
				? [{ kind: "unresolved-fixtures", count: resolved.unresolved.length, reason: "excluded from target selection; see counts.unresolvedFixtures" }]
				: []),
		],
		digests: {
			definitions: sha256(workload.definitions),
			activations: sha256(workload.activations),
			staticValues: sha256(workload.staticValues),
		},
	};
	manifest.manifestSha256 = sha256(manifest);
	return manifest;
}

/** Validate every definition and static base against the generated contracts. */
export function validateSemanticWorkload(workload) {
	const errors = [];
	for (const item of workload.definitions)
		for (const error of validateSemanticDefinition(item)) errors.push(`${item.name}: ${error}`);
	for (const mutation of workload.staticValues)
		for (const error of validateWireDefinition(SEMANTIC_CONTRACT_SCHEMAS.programmingAttributeValue, mutation.value))
			errors.push(`static ${mutation.fixture_id}/${mutation.attribute}: ${error}`);
	return errors;
}

export { semanticWorkloadDirectory };

/** Offline convenience: real packages, deterministic synthetic identities, same builder. */
export function buildSyntheticSemanticWorkload({ seed, request, rig = DEFAULT_SEMANTIC_RIG, library = loadFixtureLibrary({ packages: [...new Set([...rig.map((entry) => entry.packageName), "tosklight--3d-point"])] }) } = {}) {
	const patch = createSyntheticSemanticPatch({ seed: normalizeSeed(seed), library, rig });
	return buildSemanticPerformanceWorkload(patch, { seed, request });
}

// ---------------------------------------------------------------------------------------------
// Writing

/**
 * Write the exact inputs a report refers to. A directory already holding a different manifest,
 * or workload evidence without one, is refused unchanged; identical regeneration is allowed.
 */
export function writeSemanticWorkload(workload, directory, { emitTracking = false } = {}) {
	const expected = workload.manifest.manifestSha256;
	const existingManifest = path.join(directory, "manifest.json");
	if (fs.existsSync(existingManifest)) {
		const stored = JSON.parse(fs.readFileSync(existingManifest, "utf8")).manifestSha256;
		if (stored !== expected)
			throw new Error(`refusing to overwrite semantic workload evidence in ${directory}: it holds manifest ${stored}, not ${expected}`);
	} else if (fs.existsSync(directory) && fs.readdirSync(directory).some((name) => /^(?:workload\.json|tracking-.*\.ndjson|report-.*\.json)$/u.test(name)))
		throw new Error(`refusing to write into ${directory}: it holds workload evidence without a manifest`);
	fs.mkdirSync(directory, { recursive: true });
	const write = (name, data) => fs.writeFileSync(path.join(directory, name), `${JSON.stringify(data, null, "\t")}\n`);
	write("manifest.json", workload.manifest);
	write("workload.json", {
		workloadVersion: workload.manifest.workloadVersion,
		workloadId: workload.manifest.workloadId,
		definitions: workload.definitions,
		activations: workload.activations,
		staticValues: workload.staticValues,
		positionScenarios: workload.positionScenarios,
		dirty: workload.dirty,
		tracking: workload.tracking,
	});
	if (emitTracking)
		for (const scenario of workload.dirty.scenarios)
			for (const rateHz of workload.tracking.ratesHz) {
				const lines = [];
				for (const frame of trackingFrames(workload, { scenario: scenario.key, rateHz })) lines.push(canonicalJson(frame));
				fs.writeFileSync(path.join(directory, `tracking-${scenario.key}-${rateHz}hz.ndjson`), `${lines.join("\n")}\n`);
			}
	return directory;
}

function parseArguments(argv) {
	const options = { seed: "548", emitTracking: false };
	for (let index = 0; index < argv.length; index += 1) {
		const flag = argv[index];
		const next = () => {
			index += 1;
			if (argv[index] === undefined) throw new Error(`${flag} needs a value`);
			return argv[index];
		};
		const list = () => next().split(",").map(Number);
		if (flag === "--seed") options.seed = next();
		else if (flag === "--patch") options.patch = next();
		else if (flag === "--tracking-hz") options.trackingRatesHz = list();
		else if (flag === "--output-hz") options.outputRatesHz = list();
		else if (flag === "--duration") options.trackingDurationSeconds = Number(next());
		else if (flag === "--subset-points") options.dirtySubsetPoints = Number(next());
		else if (flag === "--out") options.out = next();
		else if (flag === "--emit-tracking") options.emitTracking = true;
		else if (flag === "--binary") options.binary = next();
		else throw new Error(`unknown argument ${flag}`);
	}
	return options;
}

async function main() {
	const options = parseArguments(process.argv.slice(2));
	const request = Object.fromEntries(
		["trackingRatesHz", "outputRatesHz", "trackingDurationSeconds", "dirtySubsetPoints"]
			.filter((key) => options[key] !== undefined)
			.map((key) => [key, options[key]]),
	);
	const workload = options.patch
		? buildSemanticPerformanceWorkload(JSON.parse(fs.readFileSync(options.patch, "utf8")), { seed: options.seed, request })
		: buildSyntheticSemanticWorkload({ seed: options.seed, request });
	const errors = validateSemanticWorkload(workload);
	if (errors.length > 0) throw new Error(`workload is not contract-valid:\n${errors.join("\n")}`);
	const { artifactPaths } = await import("./artifact-paths.mjs");
	const directory = options.out ?? semanticWorkloadDirectory(artifactPaths.performance, workload.manifest);
	writeSemanticWorkload(workload, directory, { emitTracking: options.emitTracking });
	const { manifest } = workload;
	// The builder only produces a synthetic, not-evaluated report; measured reports come from
	// the TL-548 runner through createSemanticPerformanceReport({ evidence: { source: "measured-run" } }).
	const report = await import("./semantic-performance-report.mjs");
	const reportFile = await report.writeSemanticPerformanceReport(
		report.createSemanticPerformanceReport({
			manifest,
			build: report.collectBuildIdentity({ binaryPath: options.binary }),
			host: report.collectHostIdentity(),
		}),
		directory,
	);

	console.log(`${manifest.workloadVersion} ${manifest.workloadId} (seed ${manifest.seed}, ${manifest.identitySource})`);
	console.log(`dynamics ${manifest.counts.dynamics.actual}/${manifest.counts.dynamics.requested}, targets ${manifest.counts.targets.actual}, points ${manifest.counts.points.usedByWorkload}`);
	for (const shortfall of manifest.shortfalls) console.log(`shortfall ${canonicalJson(shortfall)}`);
	console.log(`wrote ${directory}`);
	console.log(`synthetic report ${reportFile} (claims not-evaluated)`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url))
	main().catch((error) => {
		console.error(error.message);
		process.exit(1);
	});
