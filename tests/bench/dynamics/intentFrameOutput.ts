import { expect } from "@playwright/test";
import type { ApiDriver } from "../core/api";
import type { LightBench } from "../core/lightBench";
import type { IntentRig } from "./intentFrameScenario";

/**
 * Dynamics, Playbacks and published output frames of
 * docs/testing/32-intention-programming-frame-contract.md. Every read names the frame it came
 * from; the bench clock is manual, so a frame is published only by `bench.tick`.
 */

// ---------------------------------------------------------------------------------------------
// Dynamics

export type LaneSource = number | "current";

export interface AngleLane {
	component: "pan" | "tilt";
	keyframes: Array<[position: number, source: LaneSource]>;
}

export interface AngleDynamicIntent {
	pool: number;
	name: string;
	/** A fixed cycle in milliseconds, or Speed Group A at 120 BPM over this many beats. */
	cycle: { millis: number } | { speedGroupBeats: number };
	lanes: AngleLane[];
	/** Additional legacy scalar lanes (Intensity, 3D Point axes). */
	scalarLanes?: Array<{ attribute: string; keyframes: Array<[number, number]> }>;
	/** Frozen targets; a Dynamic assigned directly to a Playback cannot be targetless. */
	targets?: string[];
}

function angleLane(lane: AngleLane) {
	const address = { representation: { kind: "angles" }, component: { kind: lane.component } };
	return {
		id: crypto.randomUUID(),
		speed_multiplier: { numerator: 1, denominator: 1 },
		width: 1,
		random_group_id: null,
		phase: null,
		programming: {
			address,
			configuration: {
				mode: "keyframes",
				configuration: {
					points: lane.keyframes.map(([position, source]) => ({
						position,
						source:
							source === "current"
								? { kind: "current" }
								: { kind: "value", value: { kind: "scalar", value: source } },
						interpolation: "linear",
					})),
					size: 1,
				},
			},
		},
	};
}

function scalarLane(attribute: string, keyframes: Array<[number, number]>) {
	const shape = {
		function: "sinus",
		size: 1,
		pwm: {
			attack: 0,
			on: 0.5,
			decay: 0,
			off: 0.5,
			attack_interpolation: "linear",
			decay_interpolation: "linear",
		},
	};
	return {
		id: crypto.randomUUID(),
		attribute,
		mode: "keyframes",
		keyframes: {
			points: keyframes.map(([position, value]) => ({
				position,
				source: { type: "value", value },
				interpolation: "linear",
			})),
			size: 1,
		},
		max_min: { minimum: { type: "value", value: 0 }, maximum: { type: "value", value: 1 }, ...shape },
		middle_amplitude: { middle: { type: "current" }, amplitude: 0.35, ...shape },
		speed_multiplier: { numerator: 1, denominator: 1 },
		width: 1,
		random_group_id: null,
		phase: null,
	};
}

export function dynamicDefinition(intent: AngleDynamicIntent) {
	return {
		id: crypto.randomUUID(),
		pool_number: intent.pool,
		revision: 0,
		name: intent.name,
		color: "#4edcff",
		icon: "∿",
		target_binding: intent.targets
			? { type: "frozen_targets", targets: intent.targets }
			: { type: "targetless" },
		lanes: [
			...intent.lanes.map(angleLane),
			...(intent.scalarLanes ?? []).map((lane) => scalarLane(lane.attribute, lane.keyframes)),
		],
		random_groups: [],
		phase_mode: "uniform",
		phase: {
			ordering: { type: "selection" },
			offset_degrees: 0,
			span_degrees: 0,
			block_size: 1,
			repeats: 1,
			wings: false,
			anchors_degrees: [],
		},
		speed:
			"millis" in intent.cycle
				? { type: "fixed", duration_millis: intent.cycle.millis }
				: {
						type: "speed_group",
						group: "A",
						beats_per_cycle: { numerator: intent.cycle.speedGroupBeats, denominator: 1 },
					},
		overall_speed_multiplier: { numerator: 1, denominator: 1 },
		run_mode: "loop",
		default_activation: "start_now",
		activation_boundary: "beat",
	};
}

export interface StoredDynamic {
	id: string;
	lanes: Array<{
		programming?: {
			address: { component: { kind: string } | null };
			configuration: { configuration: { points: Array<{ source: { kind: string } }> } };
		};
	}>;
}

export async function createDynamic(api: ApiDriver, rig: IntentRig, intent: AngleDynamicIntent) {
	const created = await api.request<{ object: { body: StoredDynamic } }>(
		"POST",
		"/api/v2/dynamics/create",
		{ request_id: crypto.randomUUID(), definition: dynamicDefinition(intent) },
		true,
		undefined,
		{ showId: rig.showId },
	);
	return created.object.body;
}

export async function storedDynamic(api: ApiDriver, rig: IntentRig, id: string) {
	return (await api.showObject<StoredDynamic>(rig.showId, "dynamic", id))?.body ?? null;
}

/** The source kinds of the stored lane of one Angle component, keyframe by keyframe. */
export function laneSources(dynamic: StoredDynamic | null, component: "pan" | "tilt") {
	return dynamic?.lanes
		.find((lane) => lane.programming?.address.component?.kind === component)
		?.programming?.configuration.configuration.points.map((point) => point.source.kind);
}

/** Starts the Dynamic on the current selection, into the Programmer, as the pool does. */
export async function startDynamic(api: ApiDriver, rig: IntentRig, id: string) {
	const outcome = await api.request<{ started: boolean }>(
		"POST",
		`/api/v2/dynamics/${encodeURIComponent(id)}/start`,
		{
			request_id: crypto.randomUUID(),
			targets: [],
			overrides: {
				size: 1,
				speed_multiplier: { numerator: 1, denominator: 1 },
				phase_offset_degrees: 0,
			},
			timing: {},
		},
		true,
		undefined,
		{ showId: rig.showId },
	);
	expect(outcome.started).toBe(true);
}

export interface DynamicStatus {
	dynamic_id: string;
	lane_count: number;
	supported_address_count: number;
	skipped_address_count: number;
	warning?: string | null;
}

export async function dynamicStatus(api: ApiDriver, rig: IntentRig, id: string) {
	const runtime = await api.request<{ definitions: DynamicStatus[] }>(
		"GET",
		"/api/v2/dynamics/runtime",
		undefined,
		true,
		undefined,
		{ showId: rig.showId },
	);
	return runtime.definitions.find((definition) => definition.dynamic_id === id);
}

/** Pauses or resumes Speed Group A, which freezes the phase of every Dynamic it drives. */
export async function toggleSpeedGroupPause(api: ApiDriver) {
	return api.request<{ snapshot: { paused: boolean } }>("POST", "/api/v2/speed-groups/A/actions", {
		action: "pause",
	});
}

// ---------------------------------------------------------------------------------------------
// Published frames

export interface FrameIdentity {
	generation: number;
	sequence: number;
	sampled_at: string;
}

export interface Pose {
	available: boolean;
	pan: number;
	tilt: number;
	requested: { kind: string; value: { kind: string } } | null;
}

export interface PositionReadouts {
	frame: FrameIdentity | null;
	unavailable: string | null;
	poses: Record<string, Pose>;
}

/** The typed Position readouts of one accepted source (Live, or the Pending Preload lane). */
export async function readouts(
	api: ApiDriver,
	fixtureIds: readonly string[],
	lane: "normal" | "preload" = "normal",
): Promise<PositionReadouts> {
	const snapshot = await api.request<{
		frame?: FrameIdentity | null;
		unavailable?: string | null;
		owners: Array<{
			fixture_id: string;
			requested?: Pose["requested"];
			position: { available: boolean; common?: { pan_degrees: number; tilt_degrees: number } | null };
		}>;
	}>("GET", `/api/v2/output/readouts?lane=${lane}&fixture_ids=${fixtureIds.join(",")}`);
	return {
		frame: snapshot.frame ?? null,
		unavailable: snapshot.unavailable ?? null,
		poses: Object.fromEntries(
			snapshot.owners.map((owner) => [
				owner.fixture_id,
				{
					available: owner.position.available,
					pan: owner.position.common?.pan_degrees ?? Number.NaN,
					tilt: owner.position.common?.tilt_degrees ?? Number.NaN,
					requested: owner.requested ?? null,
				},
			]),
		),
	};
}

/** Advances the manual clock by `millis`, then reads the commanded Angles of one fixture. */
export async function poseAfter(api: ApiDriver, bench: LightBench, fixtureId: string, millis = 0) {
	await bench.tick(millis);
	const pose = (await readouts(api, [fixtureId])).poses[fixtureId];
	if (!pose) throw new Error(`no Position readout for ${fixtureId}`);
	return pose;
}

/** Commanded Angles agree within this many degrees (16-bit fitting round trip). */
export const DEGREES = 0.01;

export function expectAngles(pose: Pose, pan: number, tilt: number, label = "") {
	expect(Math.abs(pose.pan - pan), `${label} Pan ${pose.pan} vs ${pan}`).toBeLessThan(DEGREES);
	expect(Math.abs(pose.tilt - tilt), `${label} Tilt ${pose.tilt} vs ${tilt}`).toBeLessThan(DEGREES);
}

export interface DmxSnapshot {
	frame: FrameIdentity | null;
	revision: number;
	universes: Array<{ universe: number; slots: number[] }>;
	points: Array<{ fixture_id: string; offset_metres: [number, number, number] }>;
	preload: { frame: FrameIdentity | null } | null;
	preload_status?: { state: string; episode?: string | null } | null;
}

export function outputDmx(api: ApiDriver, includePreload = false) {
	return api.request<DmxSnapshot>(
		"GET",
		`/api/v2/output/dmx${includePreload ? "?include_preload=true" : ""}`,
	);
}

/** One fixture's channels, from its 1-based start address on universe 1. */
export async function channels(bench: LightBench, address: number, count: number) {
	const frame = await bench.tick(0);
	const slots = frame.universes.find((entry) => entry.universe === 1)?.slots ?? [];
	return slots.slice(address - 1, address - 1 + count);
}

export interface ColorReport {
	heads: Array<{
		fixture_id: string;
		has_target: boolean;
		quality: string;
		uv?: { status: string } | null;
		direct?: { replay: string } | null;
		note?: string | null;
	}>;
	accepted_frame?: { state: string; frame?: FrameIdentity | null } | null;
}

export function colorReport(api: ApiDriver, rig: IntentRig, fixtureIds: readonly string[]) {
	return api.request<ColorReport>(
		"GET",
		`/api/v2/color-intent/report?fixtures=${fixtureIds.join(",")}`,
		undefined,
		true,
		undefined,
		{ showId: rig.showId },
	);
}

// ---------------------------------------------------------------------------------------------
// Playbacks

export async function playbackAction(
	api: ApiDriver,
	playback: number,
	action: string,
	input: Record<string, unknown> = {},
) {
	const result = await api.playbackNumberAction<{ outcome: { status: string } }>(
		playback,
		action,
		input,
	);
	return result.outcome.status;
}

/**
 * A standalone Dynamic Playback: the Dynamic (which must name its own targets) assigned directly
 * to pool Playback `number`, its fader the non-intensity master.
 */
export async function seedDynamicPlayback(
	api: ApiDriver,
	rig: IntentRig,
	number: number,
	dynamic: StoredDynamic & { pool_number?: number },
	crossfadeNonIntensity: boolean,
) {
	await api.seedShowObject(rig.showId, "playback", String(number), {
		number,
		name: `Dynamic ${number}`,
		target: {
			type: "dynamic",
			assignment: {
				dynamic: {
					dynamic_id: dynamic.id,
					last_known_pool_number: dynamic.pool_number ?? 1,
					embedded_fallback: { definition: dynamic },
				},
				revision: 1,
				target_scope: null,
				fader_mode: "master",
				priority: 1,
				activation_override: null,
				resume_policy: "follow_dynamic",
				local_speed_multiplier: { numerator: 1, denominator: 1 },
				learned_duration_millis: null,
				crossfade_non_intensity: crossfadeNonIntensity,
				auto_off_at_zero: false,
				auto_off_flash_release: false,
				auto_off_full_control: false,
			},
		},
		buttons: ["toggle", "pause", "flash"],
		button_count: 3,
		fader: "master",
		has_fader: true,
		go_activates: true,
		auto_off: false,
		xfade_millis: 0,
		color: "#20c997",
		flash_release: "release_all",
		protect_from_swap: false,
	});
}
