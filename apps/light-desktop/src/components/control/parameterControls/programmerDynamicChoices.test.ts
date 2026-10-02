import { describe, expect, it } from "vitest";
import type { DynamicRuntimeSnapshotProjection } from "../../../api/types";
import type { ProgrammerDynamicValue } from "../../../features/programmerValues/contracts";
import { createDefaultDynamicDefinition } from "../../../windows/dynamics/DynamicsEditor";
import { dynamicChoices } from "./programmerDynamicChoices";

const definition = createDefaultDynamicDefinition(1, "intensity", {
	definition: "definition-1",
	lane: "lane-1",
});

function running(
	controllerId: string,
	authoredLink?: string,
): DynamicRuntimeSnapshotProjection {
	return {
		programmer_id: "local-programmer",
		global_paused: false,
		definitions: [],
		speed_groups: [],
		instances: [
			{
				instance_id: "runtime-instance",
				dynamic_id: definition.id,
				pool_number: definition.pool_number,
				name: definition.name,
				targets: ["fixture-1"],
				pending: false,
				pending_until_millis: null,
				paused: false,
				speed_source: "Fixed",
				activation_boundary: "beat",
				effective_cycle_millis: 1000n,
				effective_bpm: null,
				beat_phase: null,
				phase_advancing: true,
				aliasing_warning: null,
				controllers: [
					{
						controller_id: controllerId,
						programmer_id: "local-programmer",
						...(authoredLink === undefined
							? {}
							: { programmer_instance_link: authoredLink }),
						source: "Programmer",
						priority: 1,
						size: 1,
						speed_multiplier: 1,
						phase_offset_degrees: 0,
						paused: false,
						winning: true,
						releasing: false,
						activation_mix: 1,
					},
				],
			},
		],
	};
}

function pending(instanceLink: string): ProgrammerDynamicValue {
	return {
		fixtureId: "fixture-1",
		attribute: "intensity",
		programmerOrder: 2,
		changedAtMillis: 100,
		value: {
			type: "dynamic_on",
			instance_link: instanceLink,
			lane_id: "lane-1",
			dynamic: {
				dynamic_id: definition.id,
				last_known_pool_number: definition.pool_number,
				embedded_fallback_id: definition.id,
				embedded_fallback_revision: definition.revision,
				embedded_fallback: definition,
			},
			overrides: {
				size: 0.4,
				speed_multiplier: { numerator: 1, denominator: 1 },
				phase_offset_degrees: 0,
			},
			timing: { fade_millis: null, delay_millis: null },
		},
	};
}

describe("dynamicChoices scoped Programmer identity", () => {
	it("deduplicates the pending authored link while retaining the actual runtime action target", () => {
		const runtime = running("scoped-controller", "authored-link");
		const choices = dynamicChoices(runtime, [definition], [], [
			pending("authored-link"),
		]);
		expect(choices).toHaveLength(1);
		expect(choices[0].controller).toBe(runtime.instances[0].controllers[0]);
		expect(choices[0].controller.controller_id).toBe("scoped-controller");
		expect(choices[0].instance.instance_id).toBe("runtime-instance");
	});

	it("keeps a different authored link independently editable without deriving an ID in the client", () => {
		const choices = dynamicChoices(
			running("scoped-controller", "authored-link"),
			[definition],
			["fixture-1"],
			[pending("other-link")],
		);
		expect(choices).toHaveLength(2);
		expect(choices[0].controller.controller_id).toBe("scoped-controller");
		const staged = choices.find((choice) => choice.instance.pending);
		expect(staged?.controller.controller_id).toBe("other-link");
		expect(staged?.controller.programmer_instance_link).toBe("other-link");
	});

	it("uses an exact runtime ID match only within the confirmed local Programmer", () => {
		const runtime = running("legacy-link");
		const choices = dynamicChoices(runtime, [definition], [], [
			pending("legacy-link"),
		]);
		expect(choices).toHaveLength(1);
		expect(choices[0].controller).toBe(runtime.instances[0].controllers[0]);
		expect(choices[0].controller.controller_id).toBe("legacy-link");
		expect(choices[0].controller.programmer_instance_link).toBeUndefined();
	});

	it("keeps a local pending link when another Programmer uses the same imported link", () => {
		const runtime = running("foreign-controller", "authored-link");
		runtime.instances[0].controllers[0].programmer_id = "other-programmer";
		const choices = dynamicChoices(runtime, [definition], [], [
			pending("authored-link"),
		]);
		expect(choices).toHaveLength(2);
		expect(choices.find((choice) => choice.instance.pending)?.controller.programmer_id)
			.toBe("local-programmer");
	});

	it("does not infer Programmer ownership from an unscoped or non-Programmer runtime ID", () => {
		const runtime = running("authored-link");
		delete runtime.instances[0].controllers[0].programmer_id;
		expect(dynamicChoices(runtime, [definition], [], [pending("authored-link")]))
			.toHaveLength(2);
		delete runtime.programmer_id;
		expect(dynamicChoices(runtime, [definition], [], [pending("authored-link")]))
			.toHaveLength(2);
	});
});
