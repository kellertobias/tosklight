import { describe, expect, it } from "vitest";
import { parameterSemanticDisplay } from "./parameterProgrammerState";
import type { ParameterProjection } from "./useParameterProjection";

function projection(
	values: ParameterProjection["dynamicProgrammerValues"],
): ParameterProjection {
	return { dynamicProgrammerValues: values } as ParameterProjection;
}

describe("parameter Programmer semantic display", () => {
	it("shows retained Release and both numeric and Preset FixAT values", () => {
		expect(
			parameterSemanticDisplay(
				projection([
					{
						fixtureId: "fixture-1",
						attribute: "intensity",
						value: { type: "release" },
						programmerOrder: 1,
						changedAtMillis: 1,
					},
				]),
				"intensity",
			),
		).toBe("Release");

		for (const value of [
			{ type: "fix_at", value: 0.5, timing: {} },
			{
				type: "static",
				value: { kind: "normalized", value: 0.65 },
				timing: {},
			},
		] as const) {
			expect(
				parameterSemanticDisplay(
					projection([
						{
							fixtureId: "fixture-1",
							attribute: "intensity",
							value: value as never,
							programmerOrder: 1,
							changedAtMillis: 1,
						},
					]),
					"intensity",
				),
			).toBe("FixAT");
		}
	});
});
