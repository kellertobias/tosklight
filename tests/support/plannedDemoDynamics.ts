import type { ApiDriver } from "../bench/core/api";
import { putPlannedDemoObject } from "./plannedDemoObjects";
import {
	PAN_TRAVEL_DEGREES,
	TILT_TRAVEL_DEGREES,
} from "./plannedDemoSemantic";

/**
 * Programming contract 1 (TL-552/TL-648): Position, Color and Zoom lanes are typed family lanes,
 * authored here exactly as the Dynamics editor stores them. Pan and Tilt are Angles in degrees,
 * Color lanes semantic recipe components on 0–1. Only Intensity keeps scalar lanes.
 */
const PAN = angles("pan");
const TILT = angles("tilt");
/** A Waterfall sweeps Tilt between 15 % and 85 % of the demo movers' nominal 270° travel. */
const WATERFALL_TILT_LOW = (0.15 - 0.5) * TILT_TRAVEL_DEGREES;
const WATERFALL_TILT_HIGH = (0.85 - 0.5) * TILT_TRAVEL_DEGREES;
/** A Circle swings 35 % of the travel either side of Current. */
const CIRCLE_SHARE = 0.35;

const FAMILY_GROUPS = [
	["Beam Show", "4", "A"],
	["Beam Auxiliary Show", "5", "A"],
	["Wash Show", "11", "B"],
	["Wash Auxiliary Show", "12", "B"],
	["LED Show", "18", "C"],
	["LED Auxiliary Show", "19", "C"],
] as const;
const MOVING_GROUPS = [
	["Beam Show", "4"],
	["Beam Auxiliary Show", "5"],
	["Wash Show", "11"],
	["Wash Auxiliary Show", "12"],
] as const;

export function plannedDemoDynamicDefinitions() {
	const definitions: any[] = [];
	for (const [group, groupId, speed] of FAMILY_GROUPS) {
		definitions.push(
			definition(definitions.length + 1, `${group} PWM`, groupId, speed, [
				lane("intensity", "max_min", "pwm"),
			]),
			definition(
				definitions.length + 2,
				`${group} Random`,
				groupId,
				speed,
				[lane("intensity", "random", "sinus", "random")],
				[randomGroup("random")],
			),
			definition(definitions.length + 3, `${group} Sinus`, groupId, speed, [
				lane("intensity", "max_min", "sinus"),
			]),
		);
	}
	for (const [group, groupId] of MOVING_GROUPS) {
		definitions.push(
			definition(definitions.length + 1, `${group} Circle`, groupId, "E", [
				circleLane(PAN, "sinus", CIRCLE_SHARE * PAN_TRAVEL_DEGREES),
				circleLane(TILT, "cosinus", CIRCLE_SHARE * TILT_TRAVEL_DEGREES),
			]),
			definition(definitions.length + 2, `${group} Waterfall`, groupId, "E", [
				waterfallTilt(),
				keyframeLane("intensity", [
					[0, 0],
					[0.45, 1],
					[0.7, 1],
					[1, 0],
				]),
			]),
		);
	}
	definitions.push(
		definition(
			27,
			"Wash Row Waterfall",
			"8",
			"E",
			[
				waterfallTilt(),
				keyframeLane("intensity", [
					[0, 0],
					[0.45, 1],
					[0.7, 1],
					[1, 0],
				]),
			],
			[],
			{ type: "grid_linear", angle_degrees: 90 },
		),
		definition(
			28,
			"Sunstrip Random Color",
			"27",
			"C",
			[
				familyRandomLane(recipe("red"), "color"),
				familyRandomLane(recipe("green"), "color"),
				familyRandomLane(recipe("blue"), "color"),
			],
			[familyRandomGroup("color")],
		),
		definition(
			29,
			"Sunstrip Rain",
			"27",
			"C",
			[
				keyframeLane("intensity", [
					[0, 0],
					[0.5, 1],
					[1, 0],
				]),
				familyKeyframeLane(recipe("blue"), [
					[0, 0],
					[0.35, 1],
					[0.75, 1],
					[1, 0],
				]),
				familyKeyframeLane(recipe("white_blend"), [
					[0, 0],
					[0.65, 0],
					[0.8, 1],
					[1, 0],
				]),
			],
			[],
			{ type: "grid_linear", angle_degrees: 90 },
		),
		definition(
			30,
			"LED Show Random Strobe",
			"18",
			"C",
			[lane("intensity", "random", "pwm", "strobe")],
			[randomGroup("strobe")],
		),
	);
	return definitions;
}

export async function installPlannedDemoDynamics(
	api: ApiDriver,
	showId: string,
	options: { assignVirtualPlaybacks?: boolean } = {},
) {
	const requestedDefinitions = plannedDemoDynamicDefinitions();
	const existing = await api.showObjects<any>(showId, "dynamic");
	const definitions = [];
	for (const dynamicDefinition of requestedDefinitions) {
		const adopted = existing.find(
			(candidate) =>
				candidate.body.pool_number === dynamicDefinition.pool_number,
		);
		if (adopted) {
			const definition = {
				...dynamicDefinition,
				id: adopted.id,
				revision: adopted.body.revision,
			};
			await putPlannedDemoObject(
				api,
				showId,
				"dynamic",
				adopted.id,
				definition,
			);
			definitions.push(definition);
			continue;
		}
		const created = await api.request<{ object: { body: any } }>(
			"POST",
			"/api/v2/dynamics/create",
			{ request_id: crypto.randomUUID(), definition: dynamicDefinition },
			true,
			undefined,
			{ showId },
		);
		definitions.push(created.object.body);
	}
	if (options.assignVirtualPlaybacks !== false)
		await installPlannedDemoDynamicPlaybacks(api, showId, definitions);
	return definitions;
}

export async function installPlannedDemoDynamicPlaybacks(
	api: ApiDriver,
	showId: string,
	definitions: readonly any[],
) {
	const [page] = await api.showObjects<any>(showId, "playback_page");
	if (!page) throw new Error("Plan 76 Busking Playback page is missing");
	const virtual_playbacks = Object.fromEntries(
		definitions.map((dynamicDefinition, index) => {
			const number = 1001 + index;
			return [String(number), dynamicPlayback(number, dynamicDefinition)];
		}),
	);
	await api.seedShowObject(
		showId,
		"playback_page",
		page.id,
		{
			...page.body,
			virtual_playbacks,
		},
		page.revision,
	);
	return virtual_playbacks;
}

function definition(
	poolNumber: number,
	name: string,
	groupId: string,
	speedGroup: string,
	lanes: any[],
	randomGroups: any[] = [],
	ordering: any = { type: "selection" },
) {
	const boundRandomGroups = randomGroups.map((item, index) => ({
		...item,
		id: stableUuid(7, poolNumber * 100 + index + 1),
	}));
	return {
		id: stableUuid(5, poolNumber),
		pool_number: poolNumber,
		revision: 0,
		name,
		color: "#4edcff",
		icon: "∿",
		target_binding: { type: "live_group", group_id: groupId },
		lanes: lanes.map((item, index) => ({
			...item,
			id: stableUuid(6, poolNumber * 100 + index + 1),
			random_group_id: item.random_group_id
				? (boundRandomGroups[0]?.id ?? null)
				: null,
		})),
		random_groups: boundRandomGroups,
		phase_mode: "uniform",
		phase: {
			ordering,
			offset_degrees: 0,
			span_degrees: 360,
			block_size: 1,
			repeats: 1,
			wings: false,
			anchors_degrees: [],
		},
		speed: {
			type: "speed_group",
			group: speedGroup,
			beats_per_cycle: { numerator: 4, denominator: 1 },
		},
		overall_speed_multiplier: { numerator: 1, denominator: 1 },
		run_mode: "loop",
		default_activation: "start_now",
		activation_boundary: "beat",
	};
}

function angles(component: "pan" | "tilt") {
	return { representation: { kind: "angles" }, component: { kind: component } };
}

/** A semantic Color component; White Blend is orthogonal to the RGB recipe base. */
function recipe(component: "red" | "green" | "blue" | "white_blend") {
	return {
		representation: { kind: "semantic_color", basis: "recipe" },
		component: { kind: "color", component },
	};
}

const scalarValue = (value: number) => ({
	kind: "value",
	value: { kind: "scalar", value },
});

function familyLane(address: object, configuration: object, randomGroupId?: string) {
	return {
		speed_multiplier: { numerator: 1, denominator: 1 },
		width: 1,
		random_group_id: randomGroupId ?? null,
		phase: null,
		programming: { address, configuration },
	};
}

/** Swings `amplitude` degrees either side of Current. */
function circleLane(address: object, periodic: string, amplitude: number) {
	return familyLane(address, {
		mode: "middle_amplitude",
		configuration: {
			middle: { kind: "current" },
			amplitude: { kind: "scalar", value: amplitude },
			function: periodic,
			size: 1,
			pwm: {
				attack: 0,
				on: 0.5,
				decay: 0,
				off: 0.5,
				attack_interpolation: "linear",
				decay_interpolation: "linear",
			},
		},
	});
}

function familyKeyframeLane(address: object, points: Array<[number, number]>) {
	return familyLane(address, {
		mode: "keyframes",
		configuration: {
			points: points.map(([position, value]) => ({
				position: Math.min(position, 0.999),
				source: scalarValue(value),
				interpolation: "ease_in_out",
			})),
			size: 1,
		},
	});
}

function waterfallTilt() {
	return familyKeyframeLane(TILT, [
		[0, WATERFALL_TILT_LOW],
		[0.5, WATERFALL_TILT_HIGH],
		[1, WATERFALL_TILT_LOW],
	]);
}

function familyRandomLane(address: object, randomGroupId: string) {
	return familyLane(address, { mode: "random" }, randomGroupId);
}

/** A Random group for typed lanes: its range is in the components' own 0–1 recipe domain. */
function familyRandomGroup(key: string) {
	const { low: _low, high: _high, ...shared } = randomGroup(key);
	return {
		...shared,
		programming_range: { low: scalarValue(0), high: scalarValue(1) },
	};
}

function lane(
	attribute: string,
	mode: string,
	periodic: string,
	randomGroupId?: string,
) {
	return {
		attribute,
		mode,
		keyframes: keyframes([
			[0, 0],
			[0.5, 1],
			[1, 0],
		]),
		max_min: {
			minimum: value(0),
			maximum: value(1),
			function: periodic,
			size: 1,
			pwm: {
				attack: 0,
				on: 0.25,
				decay: 0,
				off: 0.75,
				attack_interpolation: "linear",
				decay_interpolation: "linear",
			},
		},
		middle_amplitude: {
			middle: { type: "current" },
			amplitude: 0.35,
			function: periodic,
			size: 1,
			pwm: {
				attack: 0,
				on: 0.5,
				decay: 0,
				off: 0.5,
				attack_interpolation: "linear",
				decay_interpolation: "linear",
			},
		},
		speed_multiplier: { numerator: 1, denominator: 1 },
		width: 1,
		random_group_id: randomGroupId ?? null,
		phase: null,
	};
}

function keyframeLane(attribute: string, points: Array<[number, number]>) {
	return {
		...lane(attribute, "keyframes", "sinus"),
		keyframes: keyframes(points),
	};
}

function keyframes(points: Array<[number, number]>) {
	return {
		points: points.map(([position, scalar]) => ({
			position: Math.min(position, 0.999),
			source: value(scalar),
			interpolation: "ease_in_out",
		})),
		size: 1,
	};
}

function randomGroup(key: string) {
	return {
		seed: randomSeed(key),
		low: value(0),
		high: value(1),
		decision_interval_millis: 250,
		start_probability: 0.25,
		mean_duration_millis: 500,
		duration_spread_millis: 100,
		attack_ratio: 0.1,
		decay_ratio: 0.1,
	};
}

function randomSeed(key: string) {
	return [...key].reduce(
		(seed, character) => seed * 31 + character.charCodeAt(0),
		17,
	);
}

function value(scalar: number) {
	return { type: "value", value: scalar };
}

function dynamicPlayback(number: number, dynamicDefinition: any) {
	return {
		number,
		name: dynamicDefinition.name,
		target: {
			type: "dynamic",
			assignment: {
				dynamic: {
					dynamic_id: dynamicDefinition.id,
					last_known_pool_number: dynamicDefinition.pool_number,
					embedded_fallback: { definition: dynamicDefinition },
				},
				revision: 1,
				target_scope: null,
				fader_mode: "size_and_master",
				priority: 0,
				activation_override: null,
				resume_policy: "follow_dynamic",
				local_speed_multiplier: { numerator: 1, denominator: 1 },
				learned_duration_millis: null,
				crossfade_non_intensity: false,
				auto_off_at_zero: false,
				auto_off_flash_release: false,
				auto_off_full_control: false,
			},
		},
		buttons: ["off", "pause", "flash"],
		button_count: 3,
		fader: "master",
		has_fader: true,
		go_activates: true,
		auto_off: true,
		xfade_millis: 0,
		color: "#4edcff",
		flash_release: "release_all",
		protect_from_swap: false,
	};
}

function stableUuid(namespace: number, value: number) {
	return `00000000-0000-4${namespace.toString(16).padStart(3, "0")}-8300-${value.toString(16).padStart(12, "0")}`;
}
