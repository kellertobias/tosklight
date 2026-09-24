import { expect } from "@playwright/test";
import { presetStorageKey } from "../../../apps/light-desktop/src/presetFamilies";
import type { ApiDriver } from "../core/api";
import type { PresetFamily } from "../groups-presets/presetScenario";

/** A keyframe's value: a fixed level, or what a stored Preset holds for each fixture. */
export type DynamicKeyframeSource =
	| number
	| { preset: { family: PresetFamily; number: number } };

export interface DynamicLaneIntent {
	attribute: string;
	/** Positions through one cycle, 0 to 1, each with the value the lane passes through there. */
	keyframes: Array<[position: number, source: DynamicKeyframeSource]>;
}

export interface DynamicIntent {
	pool: number;
	name: string;
	/** One cycle's length on a fixed clock, so a bench clock step lands on a known phase. */
	cycleMillis: number;
	/** Grid phase spread across the targets, along this angle on the stage plan. */
	gridAngleDegrees: number;
	lanes: DynamicLaneIntent[];
}

export interface DynamicHandle {
	readonly id: string;
	readonly pool: number;
	readonly name: string;
}

/**
 * Dynamics authored as the Dynamics window stores them: a Dynamic created with nothing selected is
 * targetless, and applying it starts it on whatever the Programmer has selected at that moment.
 */
export class BrowserDynamics {
	constructor(
		private readonly api: ApiDriver,
		private readonly activeShowId: () => string,
	) {}

	/** Creates a targetless Dynamic: it names no Group and no fixtures of its own. */
	async create(intent: DynamicIntent): Promise<DynamicHandle> {
		const created = await this.api.request<{
			object: { id: string; body: { id: string } };
		}>(
			"POST",
			"/api/v2/dynamics/create",
			{ request_id: crypto.randomUUID(), definition: definition(intent) },
			true,
			undefined,
			{ showId: this.activeShowId() },
		);
		return { id: created.object.body.id, pool: intent.pool, name: intent.name };
	}

	/** Starts the Dynamic on the current selection, into the Programmer, as the pool does. */
	async apply(dynamic: DynamicHandle): Promise<void> {
		await this.api.request(
			"POST",
			`/api/v2/dynamics/${encodeURIComponent(dynamic.id)}/start`,
			{ targets: [] },
			true,
			undefined,
			{ showId: this.activeShowId() },
		);
	}

	expect(dynamic: DynamicHandle) {
		return {
			/** The stored Dynamic still names no fixtures and no Group. */
			targetless: async () => {
				await expect
					.poll(async () => {
						const stored = await this.api.showObject<{
							target_binding: { type: string };
						}>(this.activeShowId(), "dynamic", dynamic.id);
						return stored?.body.target_binding.type;
					})
					.toBe("targetless");
			},
		};
	}
}

function definition(intent: DynamicIntent) {
	return {
		id: crypto.randomUUID(),
		pool_number: intent.pool,
		revision: 0,
		name: intent.name,
		color: "#4edcff",
		icon: "∿",
		target_binding: { type: "targetless" },
		lanes: intent.lanes.map(lane),
		random_groups: [],
		phase_mode: "uniform",
		phase: {
			ordering: { type: "grid_linear", angle_degrees: intent.gridAngleDegrees },
			offset_degrees: 0,
			span_degrees: 360,
			block_size: 1,
			repeats: 1,
			wings: false,
			anchors_degrees: [],
		},
		speed: { type: "fixed", duration_millis: intent.cycleMillis },
		overall_speed_multiplier: { numerator: 1, denominator: 1 },
		run_mode: "loop",
		default_activation: "start_now",
		activation_boundary: "beat",
	};
}

function lane(intent: DynamicLaneIntent) {
	const unused = {
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
		attribute: intent.attribute,
		mode: "keyframes",
		keyframes: {
			points: intent.keyframes.map(([position, source]) => ({
				position: Math.min(position, 0.999),
				source: scalar(intent.attribute, source),
				interpolation: "linear",
			})),
			size: 1,
		},
		max_min: {
			minimum: { type: "value", value: 0 },
			maximum: { type: "value", value: 1 },
			...unused,
		},
		middle_amplitude: { middle: { type: "current" }, amplitude: 0.35, ...unused },
		speed_multiplier: { numerator: 1, denominator: 1 },
		width: 1,
		random_group_id: null,
		phase: null,
	};
}

function scalar(attribute: string, source: DynamicKeyframeSource) {
	if (typeof source === "number") return { type: "value", value: source };
	return {
		type: "preset",
		preset_id: presetStorageKey({
			family: source.preset.family as Parameters<
				typeof presetStorageKey
			>[0]["family"],
			number: source.preset.number,
		}),
		attribute,
		last_valid_by_target: [],
	};
}
