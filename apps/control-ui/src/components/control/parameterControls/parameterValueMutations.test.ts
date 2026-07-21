import { describe, expect, it, vi } from "vitest";
import {
	directValueMutations,
	releaseParameterMutations,
	setParameterMutations,
	setParameterRangeMutations,
	submitParameterMutations,
} from "./parameterValueMutations";
import type { ParameterProjection } from "./useParameterProjection";

function projection(
	overrides: Partial<ParameterProjection> = {},
): ParameterProjection {
	return {
		server: {} as ParameterProjection["server"],
		programmerFadeMillis: 1_250,
		state: {} as ParameterProjection["state"],
		active: true,
		selectedFixtureIds: ["fixture-3", "fixture-1", "fixture-2"],
		selectedGroupId: null,
		programmerValuesRoute: "normal",
		programmerValuesReady: true,
		programmerValues: [],
		groupProgrammerValues: [],
		normalized: new Map(),
		normalizedByFixture: new Map(),
		discrete: new Map(),
		discreteByFixture: new Map(),
		directChoices: { values: [], actions: [], fixtureIds: [] },
		encoderSlots: Array.from({ length: 6 }, () => "intensity"),
		hardwareConnected: false,
		...overrides,
	};
}

describe("parameter value mutation builders", () => {
	it("keeps ordered fixture writes in one explicitly timed mutation list", () => {
		expect(
			setParameterMutations(projection(), "intensity", {
				kind: "normalized",
				value: 0.5,
			}),
		).toEqual(
			["fixture-3", "fixture-1", "fixture-2"].map((fixtureId) => ({
				action: "set_fixture",
				fixtureId,
				attribute: "intensity",
				value: { kind: "normalized", value: 0.5 },
				timing: { fade: true, fadeMillis: 1_250, delayMillis: null },
			})),
		);
	});

	it("builds one Group spread and ordered fixture interpolation", () => {
		expect(
			setParameterRangeMutations(
				projection({ selectedGroupId: "front" }),
				"pan",
				[0, 50, 100],
			),
		).toEqual([
			{
				action: "set_group",
				groupId: "front",
				attribute: "pan",
				value: { kind: "spread", value: [0, 0.5, 1] },
				timing: { fade: true, fadeMillis: 1_250, delayMillis: null },
			},
		]);
		expect(
			setParameterRangeMutations(projection(), "pan", [0, 50]).map(
				(mutation) =>
					mutation.action === "set_fixture" ? mutation.value : null,
			),
		).toEqual([
			{ kind: "normalized", value: 0 },
			{ kind: "normalized", value: 0.25 },
			{ kind: "normalized", value: 0.5 },
		]);
	});

	it("releases only selected scoped values while preserving selection order", () => {
		const mutations = releaseParameterMutations(
			projection({
				programmerValues: [
					{
						fixtureId: "fixture-2",
						attribute: "intensity",
						value: { kind: "normalized", value: 0.2 },
						programmerOrder: 2,
						fade: true,
						fadeMillis: null,
						delayMillis: null,
					},
					{
						fixtureId: "fixture-3",
						attribute: "intensity",
						value: { kind: "normalized", value: 0.3 },
						programmerOrder: 1,
						fade: true,
						fadeMillis: null,
						delayMillis: null,
					},
				],
			}),
			"intensity",
		);
		expect(mutations).toEqual([
			{
				action: "release_fixture",
				fixtureId: "fixture-3",
				attribute: "intensity",
			},
			{
				action: "release_fixture",
				fixtureId: "fixture-2",
				attribute: "intensity",
			},
		]);
	});

	it("batches portable direct values and skips empty submissions", async () => {
		const actions = { batch: vi.fn(async () => ({ status: "changed" })) };
		const mutations = directValueMutations(projection(), {
			key: "indexed:gobo.dots",
			label: "Dots",
			semanticId: "gobo.dots",
			kind: "indexed",
			assignments: [
				{ fixtureId: "fixture-3", attribute: "gobo.1" },
				{ fixtureId: "fixture-1", attribute: "gobo.2" },
			],
		});
		await submitParameterMutations(actions, mutations, () => "request-1");
		await submitParameterMutations(actions, [], () => "request-2");
		expect(actions.batch).toHaveBeenCalledOnce();
		expect(actions.batch).toHaveBeenCalledWith({
			requestId: "request-1",
			mutations,
		});
	});
});
