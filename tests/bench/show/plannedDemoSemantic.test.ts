import { describe, expect, it } from "vitest";
import { plannedDemoDynamicDefinitions } from "../../support/plannedDemoDynamics";
import * as semantic from "../../support/plannedDemoSemantic";

// The legacy family addresses the contract-1 validator rejects. Kept in step with
// `light_show::legacy_programming_family` (percentage `zoom` included since TL-552).
const LEGACY = /"(?:attribute":"(?:pan|tilt|zoom|color\.(?:red|green|blue|white))"|(?:pan|tilt|color\.(?:red|green|blue|white))":\{"kind"|zoom":\{"kind":"(?:normalized|spread)")/;

describe("Plan 76 semantic demo generator (TL-552)", () => {
	it("has no legacy switch left: the generator only authors contract-1 intent", () => {
		expect(Object.keys(semantic)).not.toContain("PLANNED_DEMO_SEMANTIC");
		expect(JSON.stringify(plannedDemoDynamicDefinitions())).not.toMatch(LEGACY);
	});

	it("authors typed Dynamic lanes with no legacy family address", () => {
		const definitions = plannedDemoDynamicDefinitions();
		expect(definitions).toHaveLength(30);
		expect(JSON.stringify(definitions)).not.toMatch(LEGACY);
		const circle = definitions.find(
			(definition: any) => definition.name === "Beam Show Circle",
		);
		expect(circle.lanes.map((lane: any) => lane.programming.address)).toEqual([
			{ representation: { kind: "angles" }, component: { kind: "pan" } },
			{ representation: { kind: "angles" }, component: { kind: "tilt" } },
		]);
		expect(
			circle.lanes[0].programming.configuration.configuration.amplitude,
		).toEqual({ kind: "scalar", value: 0.35 * semantic.PAN_TRAVEL_DEGREES });
		const random = definitions.find(
			(definition: any) => definition.name === "Sunstrip Random Color",
		);
		expect(random.random_groups[0].programming_range).toEqual({
			low: { kind: "value", value: { kind: "scalar", value: 0 } },
			high: { kind: "value", value: { kind: "scalar", value: 1 } },
		});
		// Intensity lanes stay scalar.
		const pwm = definitions.find(
			(definition: any) => definition.name === "Beam Show PWM",
		);
		expect(pwm.lanes[0].attribute).toBe("intensity");
	});

	it("maps normalized Position to centred degrees and names exact semantic Colors", () => {
		expect(semantic.semanticAngles(0.5, 0.25)).toEqual({
			kind: "position",
			value: {
				kind: "angles",
				pan_degrees: { kind: "value", value: 0 },
				tilt_degrees: { kind: "value", value: -67.5 },
			},
		});
		const red = semantic.semanticColor("Red") as any;
		expect(red.kind).toBe("color_program");
		expect(red.value.intent.recipe.rgb).toEqual([1, 0, 0]);
		expect(() => semantic.semanticColor("Chartreuse")).toThrow();
	});
});
