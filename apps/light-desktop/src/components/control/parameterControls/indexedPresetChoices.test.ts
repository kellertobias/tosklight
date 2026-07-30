import { describe, expect, it } from "vitest";
import type { PatchedFixture } from "../../../api/types";
import { indexedPresetChoices } from "./indexedPresetChoices";

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
				targets: [{ fixtureId: "fixture-1" }, { fixtureId: "fixture-2" }],
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
