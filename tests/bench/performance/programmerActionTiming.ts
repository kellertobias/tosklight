import { expect, type Page } from "@playwright/test";
import type {
	DynamicInstanceActionOutcome,
	LiveAction,
} from "../../../apps/light-desktop/src/api/generated/light-wire";
import type { ApiDriver } from "../core/api";
import type { LightBench } from "../core/lightBench";
import type { SimulatedHardware } from "../hardware/hardwareScenario";

export interface ProgrammerActionTimingRecord {
	action_id: number;
	source: string;
	action: string;
	request_id: string;
	received_output_tick: number;
	acknowledged_output_tick: number;
	first_output_tick: number | null;
	acknowledgement_wall_micros: number;
	first_output_wall_micros: number | null;
	output_frame_hz: number;
	budget_ticks: number;
	requires_output_frame: boolean;
	acknowledgement_within_budget: boolean;
	output_within_budget: boolean | null;
	succeeded: boolean;
}

export interface ProgrammerActionTimingIntent {
	source: "http" | "websocket" | "osc";
	route:
		| "software"
		| "keyboard"
		| "http"
		| "websocket"
		| "osc"
		| "attached-hardware";
	action: string;
	requiresOutputFrame: boolean;
}

interface PerformanceDiagnostics {
	programmer_action_timing: ProgrammerActionTimingRecord[];
}

/** Gates a real action on receipt, authoritative ack, and first output render. */
export class BrowserProgrammerActionTiming {
	private readonly observations: Array<
		ProgrammerActionTimingIntent & { record: ProgrammerActionTimingRecord }
	> = [];

	constructor(
		private readonly api: ApiDriver,
		private readonly bench: LightBench,
		private readonly page: Page,
	) {}

	async mark(): Promise<number> {
		return latestActionId((await this.snapshot()).programmer_action_timing);
	}

	async expectAction(
		intent: ProgrammerActionTimingIntent,
		operation: () => Promise<unknown>,
	): Promise<ProgrammerActionTimingRecord> {
		const baseline = await this.mark();
		// Keep the production scheduler running while the physical/browser interaction is in
		// flight. A Stage render may delay Playwright's input promise, but it must never freeze
		// authoritative output as the explicit test clock otherwise would.
		const scheduler = intent.requiresOutputFrame
			? this.bench.freeRunClock(1_000)
			: undefined;
		await operation();
		let matched: ProgrammerActionTimingRecord | undefined;
		await scheduler;
		await expect
			.poll(async () => {
				matched = (await this.snapshot()).programmer_action_timing.find(
					(record) =>
						record.action_id > baseline &&
						record.source === intent.source &&
						record.action === intent.action,
				);
				if (!matched) return "missing";
				return matched.requires_output_frame &&
					matched.output_within_budget == null
					? "pending-output"
					: "settled";
			})
			.toBe("settled");
		// The timing record deliberately settles on the first affected frame. Advance once more
		// after measuring so a configured Programmer fade cannot leak a partial value into the
		// next independent route assertion.
		if (intent.requiresOutputFrame) await this.bench.freeRunClock(4_000);
		expect(matched).toBeDefined();
		expect(matched?.succeeded).toBe(true);
		expect(matched?.requires_output_frame).toBe(intent.requiresOutputFrame);
		expect(matched?.budget_ticks).toBe(
			(matched?.output_frame_hz ?? 0) <= 60 ? 2 : 4,
		);
		expect(matched?.acknowledged_output_tick).toBeGreaterThanOrEqual(
			matched?.received_output_tick ?? 0,
		);
		expect(
			matched?.acknowledgement_within_budget,
			`Programmer acknowledgement exceeded its tick budget: ${JSON.stringify(matched)}`,
		).toBe(true);
		if (intent.requiresOutputFrame) {
			expect(matched?.first_output_tick).not.toBeNull();
			expect(matched?.first_output_wall_micros).not.toBeNull();
			expect(
				matched?.output_within_budget,
				`Programmer output exceeded its tick budget: ${JSON.stringify(matched)}`,
			).toBe(true);
		} else {
			expect(matched?.output_within_budget).toBeNull();
		}
		this.observations.push({
			...intent,
			record: matched as ProgrammerActionTimingRecord,
		});
		return matched as ProgrammerActionTimingRecord;
	}

	expectLiveAction(
		actionName: string,
		requiresOutputFrame: boolean,
		action: LiveAction,
		route: ProgrammerActionTimingIntent["route"] = "websocket",
	): Promise<ProgrammerActionTimingRecord> {
		return this.expectAction(
			{
				source: "websocket",
				route,
				action: actionName,
				requiresOutputFrame,
			},
			() => this.api.liveAction(action),
		);
	}

	expectHttpCommandKey(): Promise<ProgrammerActionTimingRecord> {
		return this.expectAction(
			{
				source: "http",
				route: "software",
				action: "command_key",
				requiresOutputFrame: true,
			},
			() => this.api.sendCommandKey("7"),
		);
	}

	async expectOscProgrammerKey(
		hardware: SimulatedHardware,
		key: string,
		requiresOutputFrame: boolean,
	): Promise<ProgrammerActionTimingRecord> {
		const alias = "desk";
		if (!alias) throw new Error("OSC timing requires an authenticated desk");
		const timing = await this.expectAction(
			{
				source: "osc",
				route: "attached-hardware",
				action: "programmer_key",
				requiresOutputFrame,
			},
			() =>
				hardware.send(`/light/${alias}/programmer/${key}`, [
					true,
					`latency-${crypto.randomUUID()}`,
				]),
		);
		const digit = key.match(/^digit-(\d)$/)?.[1];
		if (digit)
			expect((await this.api.getCommandLine()).commandLine.text).toContain(
				digit,
			);
		return timing;
	}

	async expectDirectOscCommandEdit(): Promise<ProgrammerActionTimingRecord> {
		const alias = "desk";
		if (!alias) throw new Error("OSC timing requires an authenticated desk");
		const endpoint = await this.bench.osc();
		const clientId = `latency-direct-${crypto.randomUUID()}`;
		try {
			await endpoint.subscribe(clientId, alias);
			const mark = endpoint.mark();
			const timing = await this.expectAction(
				{
					source: "osc",
					route: "osc",
					action: "programmer_key",
					requiresOutputFrame: false,
				},
				async () => {
					await endpoint.send(`/light/${alias}/programmer/digit-7`, [
						true,
						`latency-${crypto.randomUUID()}`,
					]);
					await endpoint.expectAfter(
						mark,
						`/light/${alias}/feedback/command-line`,
					);
				},
			);
			expect((await this.api.getCommandLine()).commandLine.text).toContain("7");
			return timing;
		} finally {
			await endpoint.unsubscribe(clientId).catch(() => undefined);
			await endpoint.close();
		}
	}

	async expectDynamicStart(
		showId: string,
	): Promise<ProgrammerActionTimingRecord> {
		const fixtureId = (await this.api.patch()).fixtures[0]?.fixture_id;
		if (!fixtureId)
			throw new Error("Dynamic timing requires one patched fixture");
		const created = await this.api.request<{
			object: { id: string };
		}>(
			"POST",
			"/api/v2/dynamics/create",
			{
				request_id: crypto.randomUUID(),
				definition: latencyDynamicDefinition(),
			},
			true,
			undefined,
			{ showId },
		);
		const requestId = crypto.randomUUID();
		let outcome: DynamicInstanceActionOutcome | undefined;
		const timing = await this.expectAction(
			{
				source: "websocket",
				route: "websocket",
				action: "dynamic",
				requiresOutputFrame: true,
			},
			async () => {
				const response =
					await this.api.liveAction<DynamicInstanceActionOutcome>(
					{
						type: "dynamic_start",
						request: {
							dynamic_id: created.object.id,
							request: {
								request_id: requestId,
								targets: [fixtureId],
								overrides: {
									size: 1,
									speed_multiplier: { numerator: 1, denominator: 1 },
									phase_offset_degrees: 0,
								},
								timing: {},
								undo_group: "programmer-action-latency",
							},
						},
					},
					requestId,
				);
				outcome = response.payload;
			},
		);
		if (!outcome)
			throw new Error("Dynamic timing action returned no controller outcome");
		const offRequestId = crypto.randomUUID();
		await this.api.liveAction(
			{
				type: "dynamic_off",
				request: {
					controller_id: outcome.controller_id,
					request: { request_id: offRequestId, timing: {} },
				},
			},
			offRequestId,
		);
		await this.bench.freeRunClock(1_000);
		return timing;
	}

	async expectKeyboardCommand(
		command: string,
	): Promise<ProgrammerActionTimingRecord> {
		return this.expectAction(
			{
				source: "websocket",
				route: "keyboard",
				action: "command_execute",
				requiresOutputFrame: true,
			},
			async () => {
				const input = this.page.getByRole("textbox", {
					name: "Command line",
					exact: true,
				});
				await input.fill(command);
				await input.press("Enter");
			},
		);
	}

	setOutputFrameRate(frameRateHz: number): Promise<unknown> {
		return this.api.request("POST", "/api/v2/test/output/frame-rate", {
			frame_rate_hz: frameRateHz,
		});
	}

	expectCoverage(requirements: {
		routes: ProgrammerActionTimingIntent["route"][];
		actions: string[];
		frameRateBands: Array<"at-or-below-60" | "above-60">;
	}): void {
		expect(this.observations.length).toBeGreaterThan(0);
		for (const route of requirements.routes)
			expect(
				this.observations.some((observation) => observation.route === route),
				`missing Programmer timing route ${route}`,
			).toBe(true);
		for (const action of requirements.actions)
			expect(
				this.observations.some((observation) => observation.action === action),
				`missing Programmer timing action ${action}`,
			).toBe(true);
		for (const band of requirements.frameRateBands)
			expect(
				this.observations.some(({ record }) =>
					band === "at-or-below-60"
						? record.output_frame_hz <= 60 && record.budget_ticks === 2
						: record.output_frame_hz > 60 && record.budget_ticks === 4,
				),
				`missing Programmer timing frame-rate band ${band}`,
			).toBe(true);
	}

	private snapshot(): Promise<PerformanceDiagnostics> {
		return this.api.request("GET", "/api/v2/diagnostics/performance");
	}
}

function latestActionId(records: ProgrammerActionTimingRecord[]): number {
	return Math.max(0, ...records.map((record) => record.action_id));
}

function latencyDynamicDefinition() {
	const value = (normalized: number) => ({
		type: "value",
		value: normalized,
	});
	const pwm = {
		attack: 0,
		on: 0.5,
		decay: 0,
		off: 0.5,
		attack_interpolation: "linear",
		decay_interpolation: "linear",
	};
	return {
		id: crypto.randomUUID(),
		pool_number: 99,
		revision: 0,
		name: "Programmer latency Dynamic",
		color: "#4edcff",
		icon: "∿",
		target_binding: { type: "targetless" },
		lanes: [
			{
				id: crypto.randomUUID(),
				attribute: "intensity",
				mode: "max_min",
				keyframes: {
					points: [
						{ position: 0, source: value(0), interpolation: "ease_in_out" },
						{
							position: 0.5,
							source: value(1),
							interpolation: "ease_in_out",
						},
					],
					size: 1,
				},
				max_min: {
					minimum: value(0),
					maximum: value(1),
					function: "sinus",
					size: 1,
					pwm,
				},
				middle_amplitude: {
					middle: value(0.5),
					amplitude: 0.5,
					function: "sinus",
					size: 1,
					pwm,
				},
				speed_multiplier: { numerator: 1, denominator: 1 },
				width: 1,
				random_group_id: null,
				phase: null,
			},
		],
		random_groups: [],
		phase_mode: "uniform",
		phase: {
			ordering: { type: "selection" },
			offset_degrees: 0,
			span_degrees: 360,
			block_size: 1,
			repeats: 1,
			wings: false,
			anchors_degrees: [],
		},
		speed: { type: "fixed", duration_millis: 4000 },
		overall_speed_multiplier: { numerator: 1, denominator: 1 },
		run_mode: "loop",
		default_activation: "start_now",
		activation_boundary: "beat",
	};
}
