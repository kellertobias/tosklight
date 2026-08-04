import { describe, expect, it, vi } from "vitest";
import {
	releaseParameterMutations,
	setParameterMutations,
	setParameterRangeMutations,
	submitParameterAbsoluteIntent,
	submitParameterMutations,
	submitParameterStep,
} from "./parameterValueMutations";
import type { ParameterProjection } from "./useParameterProjection";

function projection(
	overrides: Partial<ParameterProjection> = {},
): ParameterProjection {
	return {
		programmerActions: null,
		programmerFadeMillis: 1_250,
		state: {} as ParameterProjection["state"],
		active: true,
		selectedFixtureIds: ["fixture-3", "fixture-1", "fixture-2"],
		selectedFixtures: [],
		selectionRevision: 1,
		selectedGroupId: null,
		programmerValuesRoute: "normal",
		programmerValuesReady: true,
		programmerValues: [],
		groupProgrammerValues: [],
		normalized: new Map(),
		normalizedByFixture: new Map(),
		discrete: new Map(),
		discreteByFixture: new Map(),
		encoderGroups: [],
		encoderPage: 1,
		encoderPageCount: 1,
		encoderSlots: Array.from({ length: 6 }, () => "intensity"),
		encoderPushTurnSlots: Array.from({ length: 6 }, () => null),
		visibleEncoderCount: 6,
		attributeLabels: new Map(),
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
		// The non-group branch sends the ordered selection and lets the server interpolate.
		expect(setParameterRangeMutations(projection(), "pan", [0, 50])).toEqual([
			{
				action: "set_selection",
				fixtureIds: ["fixture-3", "fixture-1", "fixture-2"],
				attribute: "pan",
				value: { kind: "spread", value: [0, 0.5] },
				timing: { fade: true, fadeMillis: 1_250, delayMillis: null },
			},
		]);
		expect(
			setParameterRangeMutations(
				projection({
					programmerValuesRoute: "preload",
					selectedFixtureIds: [
						"fixture-4",
						"fixture-3",
						"fixture-2",
						"fixture-1",
					],
				}),
				"pan",
				[0, 100, 0],
			),
		).toEqual(
			["fixture-4", "fixture-3", "fixture-2", "fixture-1"].map(
				(fixtureId, index) => ({
					action: "set_fixture",
					fixtureId,
					attribute: "pan",
					value: { kind: "normalized", value: [0, 1, 1, 0][index] },
					timing: { fade: true, fadeMillis: 1_250, delayMillis: null },
				}),
			),
		);
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

	it("skips empty mutation submissions", async () => {
		const actions = { batch: vi.fn(async () => ({ status: "changed" })) };
		await submitParameterMutations(actions, [], () => "request-1");
		expect(actions.batch).not.toHaveBeenCalled();
	});

	it("submits a hardware tick as one signed server-owned intent", async () => {
		const actions = {
			batch: vi.fn(),
			applyIntent: vi.fn(async () => ({ status: "changed" })),
		};
		await submitParameterStep(
			actions,
			projection(),
			"intensity",
			-0.1,
			() => "step-1",
		);
		expect(actions.batch).not.toHaveBeenCalled();
		expect(actions.applyIntent).toHaveBeenCalledWith({
			requestId: "step-1",
			fixtureIds: ["fixture-3", "fixture-1", "fixture-2"],
			attribute: "intensity",
			operation: { type: "relative_step", delta: -0.1 },
			timing: { fade: false, fadeMillis: null, delayMillis: null },
		});
	});

	it("submits an absolute selection without expanding fixture writes locally", async () => {
		const actions = {
			batch: vi.fn(),
			applyIntent: vi.fn(async () => ({ status: "changed" })),
		};
		await submitParameterAbsoluteIntent(
			actions,
			projection(),
			"pan",
			{ kind: "spread", value: [0, 0.5, 1] },
			() => "absolute-1",
		);
		expect(actions.batch).not.toHaveBeenCalled();
		expect(actions.applyIntent).toHaveBeenCalledWith({
			requestId: "absolute-1",
			fixtureIds: ["fixture-3", "fixture-1", "fixture-2"],
			attribute: "pan",
			operation: {
				type: "absolute_set",
				value: { kind: "spread", value: [0, 0.5, 1] },
			},
			timing: { fade: false, fadeMillis: null, delayMillis: null },
		});
	});

	it("submits a live Group tick as one server-owned Group intent", async () => {
		const actions = {
			batch: vi.fn(),
			applyIntent: vi.fn(async () => ({ status: "changed" })),
		};
		await submitParameterStep(
			actions,
			projection({ selectedGroupId: "front" }),
			"intensity",
			0.01,
			() => "group-step-1",
		);
		expect(actions.batch).not.toHaveBeenCalled();
		expect(actions.applyIntent).toHaveBeenCalledWith({
			requestId: "group-step-1",
			fixtureIds: [],
			groupId: "front",
			attribute: "intensity",
			operation: { type: "relative_step", delta: 0.01 },
			timing: { fade: false, fadeMillis: null, delayMillis: null },
		});
	});
});
