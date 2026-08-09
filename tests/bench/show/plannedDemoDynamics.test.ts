import { describe, expect, it } from "vitest";
import { plannedDemoDynamicDefinitions } from "../../support/plannedDemoDynamics";

describe("Plan 76 Dynamic library", () => {
	it("defines the exact 30 Group-bound Dynamics and Speed Group mapping", () => {
		const definitions = plannedDemoDynamicDefinitions();
		expect(definitions).toHaveLength(30);
		expect(new Set(definitions.map((definition) => definition.id)).size).toBe(
			30,
		);
		expect(
			definitions.every(
				(definition) => definition.target_binding.type === "live_group",
			),
		).toBe(true);
		expect(
			definitions
				.slice(0, 6)
				.every((definition) => definition.speed.group === "A"),
		).toBe(true);
		expect(
			definitions
				.slice(6, 12)
				.every((definition) => definition.speed.group === "B"),
		).toBe(true);
		expect(
			definitions
				.slice(12, 18)
				.every((definition) => definition.speed.group === "C"),
		).toBe(true);
		expect(
			definitions
				.slice(18, 27)
				.every((definition) => definition.speed.group === "E"),
		).toBe(true);
		expect(
			definitions
				.slice(27)
				.every((definition) => definition.speed.group === "C"),
		).toBe(true);
		expect(definitions.map((definition) => definition.name).slice(-4)).toEqual([
			"Wash Row Waterfall",
			"Sunstrip Random Color",
			"Sunstrip Rain",
			"LED Show Random Strobe",
		]);
		expect(definitions[26].phase.ordering).toEqual({
			type: "grid_linear",
			angle_degrees: 90,
		});
		expect(definitions[29].lanes).toEqual([
			expect.objectContaining({
				attribute: "intensity",
				mode: "random",
				max_min: expect.objectContaining({ function: "pwm" }),
				random_group_id: definitions[29].random_groups[0].id,
			}),
		]);
		expect(definitions[29].random_groups).toHaveLength(1);
	});
});
