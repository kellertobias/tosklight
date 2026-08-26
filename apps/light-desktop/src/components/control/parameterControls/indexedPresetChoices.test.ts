import { describe, expect, it } from "vitest";
import type { PatchedFixture } from "../../../api/types";
import {
	indexedFunctionLabel,
	indexedPresetChoices,
} from "./indexedPresetChoices";

describe("indexedPresetChoices", () => {
	it("merges compatible semantics despite different authored raw values", () => {
		const first = fixture("fixture-1", "profile-1", "gobo.dots", "Dots", 93);
		const second = fixture("fixture-2", "profile-2", "gobo.dots", "Dots", 41);

		const choices = indexedPresetChoices(
			[first, second],
			["fixture-1", "fixture-2"],
			"gobo.1",
		);

		expect(choices).toEqual([
			expect.objectContaining({
				label: "Dots",
				semanticId: "gobo.dots",
				description: "All selected fixtures",
				targets: [
					{
						fixtureId: "fixture-1",
						functionId: "profile-1-function",
						profileRevision: 1,
					},
					{
						fixtureId: "fixture-2",
						functionId: "profile-2-function",
						profileRevision: 1,
					},
				],
			}),
		]);
	});

	it("keeps same-name incompatible semantics and partial support visibly scoped", () => {
		const first = fixture("fixture-1", "profile-1", "reset.soft", "Reset", 93);
		const second = fixture("fixture-2", "profile-2", "reset.hard", "Reset", 41);

		const choices = indexedPresetChoices(
			[first, second],
			["fixture-1", "fixture-2"],
			"gobo.1",
		);

		expect(choices).toHaveLength(2);
		expect(choices.map((choice) => choice.semanticId).sort()).toEqual([
			"reset.hard",
			"reset.soft",
		]);
		expect(
			choices.every((choice) => choice.description !== "All selected fixtures"),
		).toBe(true);
	});
});

function fixture(
	fixtureId: string,
	profileId: string,
	semanticId: string,
	label: string,
	rawValue: number,
) {
	const headId = `${profileId}-head`;
	const modeId = `${profileId}-mode`;
	return {
		fixture_id: fixtureId,
		fixture_number: Number(fixtureId.slice(-1)),
		name: `Fixture ${fixtureId.slice(-1)}`,
		logical_heads: [],
		definition: {
			name: `Model ${fixtureId.slice(-1)}`,
			mode_id: modeId,
			profile_snapshot: {
				revision: 1,
				modes: [
					{
						id: modeId,
						heads: [{ id: headId, master_shared: true }],
						channels: [
							{
								id: `${profileId}-channel`,
								head_id: headId,
								attribute: "gobo.1",
								functions: [
									{
										id: `${profileId}-function`,
										name: label,
										dmx_from: rawValue,
										dmx_to: rawValue,
										attribute: "gobo.1",
										priority: 0,
										behavior: {
											type: "indexed",
											semantic_id: semanticId,
											label,
											raw_value: rawValue,
										},
									},
								],
							},
						],
						control_actions: [],
					},
				],
			},
		},
	} as unknown as PatchedFixture;
}

describe("indexedFunctionLabel", () => {
	/** A shutter shaped like the shipped Robe profile, whose default raw 32 is Shutter open. */
	function shutterFixture(fixtureId: string) {
		const headId = `${fixtureId}-head`;
		const modeId = `${fixtureId}-mode`;
		const ranges: Array<[number, number, string, "fixed" | "continuous"]> = [
			[0, 31, "Shutter closed", "fixed"],
			[32, 63, "Shutter open", "fixed"],
			[64, 95, "Strobe effect from slow to fast", "continuous"],
		];
		return {
			fixture_id: fixtureId,
			fixture_number: 1,
			name: fixtureId,
			logical_heads: [],
			definition: {
				name: "Moving light",
				mode_id: modeId,
				profile_snapshot: {
					revision: 1,
					modes: [
						{
							id: modeId,
							heads: [{ id: headId, master_shared: true }],
							channels: [
								{
									id: `${fixtureId}-shutter`,
									head_id: headId,
									attribute: "shutter",
									functions: ranges.map(([from, to, label, type]) => ({
										id: `${fixtureId}-${from}`,
										name: label,
										dmx_from: from,
										dmx_to: to,
										attribute: "shutter",
										priority: 0,
										behavior:
											type === "fixed"
												? {
														type: "fixed",
														semantic_id: label
															.toLowerCase()
															.replace(/\s+/gu, "-"),
														label,
														raw_value: from,
													}
												: {
														type: "continuous",
														physical_min: 0,
														physical_max: 1,
														unit: null,
													},
									})),
								},
							],
							control_actions: [],
						},
					],
				},
			},
		} as unknown as PatchedFixture;
	}

	it("names the position a stepped value sits in instead of the percentage it happens to be", () => {
		const fixtures = [shutterFixture("fixture-1")];
		// Raw 32 is the shipped shutter default, which reads as 13% of the channel.
		expect(
			indexedFunctionLabel(fixtures, ["fixture-1"], "shutter", () => 32 / 255),
		).toBe("Shutter open");
		expect(
			indexedFunctionLabel(fixtures, ["fixture-1"], "shutter", () => 0),
		).toBe("Shutter closed");
	});

	it("leaves a continuous range reading as a proportion", () => {
		const fixtures = [shutterFixture("fixture-1")];
		// Inside the strobe sweep there is no position to name.
		expect(
			indexedFunctionLabel(fixtures, ["fixture-1"], "shutter", () => 80 / 255),
		).toBeUndefined();
	});

	it("reports a split selection rather than naming one fixture's position for all", () => {
		const fixtures = [shutterFixture("fixture-1"), shutterFixture("fixture-2")];
		expect(
			indexedFunctionLabel(fixtures, ["fixture-1", "fixture-2"], "shutter", (id) =>
				id === "fixture-1" ? 32 / 255 : 0,
			),
		).toBe("Mixed");
	});

	it("says nothing about an attribute the selection does not carry", () => {
		const fixtures = [shutterFixture("fixture-1")];
		expect(
			indexedFunctionLabel(fixtures, ["fixture-1"], "intensity", () => 0.5),
		).toBeUndefined();
		expect(
			indexedFunctionLabel(fixtures, ["fixture-9"], "shutter", () => 0.5),
		).toBeUndefined();
	});
});
