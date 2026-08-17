import { describe, expect, it } from "vitest";
import {
	normalizedParameterDisplay,
	parameterSemanticDisplay,
} from "./parameterProgrammerState";
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

describe("media address encoder display", () => {
	it("shows byte-addressed files and folders instead of percentages", () => {
		const mediaProjection = {
			selectedGroupId: null,
			selectedFixtureIds: ["fixture-1"],
			programmerValues: [],
			groupProgrammerValues: [],
			normalizedByFixture: new Map([
				[
					"fixture-1",
					new Map([
						["media.file", 3 / 255],
						["media.folder", 1 / 255],
					]),
				],
			]),
		} as unknown as ParameterProjection;

		expect(normalizedParameterDisplay(mediaProjection, "media.file")).toBe(
			"File 3",
		);
		expect(normalizedParameterDisplay(mediaProjection, "media.folder")).toBe(
			"Folder 1",
		);
	});

	it("calls address zero No file", () => {
		const mediaProjection = {
			selectedGroupId: null,
			selectedFixtureIds: ["fixture-1"],
			programmerValues: [],
			groupProgrammerValues: [],
			normalizedByFixture: new Map([
				["fixture-1", new Map([["media.file", 0]])],
			]),
		} as unknown as ParameterProjection;

		expect(normalizedParameterDisplay(mediaProjection, "media.file")).toBe(
			"No file",
		);
	});
});
