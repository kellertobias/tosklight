import { describe, expect, it } from "vitest";
import type { SelectionProjection } from "./contracts";
import {
	applySelectionRule,
	gestureSelectionPrediction,
	groupSelectionPrediction,
	replaceSelectionPrediction,
	ruleSelectionPrediction,
} from "./selectionPrediction";
import { FIXTURE_1, FIXTURE_2, FIXTURE_3 } from "./testFixtures";

const FIXTURE_4 = "55555555-5555-4555-8555-555555555555";

function selection(
	overrides: Partial<SelectionProjection> = {},
): SelectionProjection {
	return {
		selected: [FIXTURE_1],
		expression: { type: "static" },
		revision: 7,
		gestureOpen: false,
		...overrides,
	};
}

describe("selection predictions", () => {
	it("replaces with ordered unique fixtures and closes the gesture", () => {
		const predicted = replaceSelectionPrediction([
			FIXTURE_2,
			FIXTURE_1,
			FIXTURE_2,
		])(
			selection({
				expression: {
					type: "sources",
					items: [{ type: "fixture", fixtureId: FIXTURE_1 }],
				},
				gestureOpen: true,
			}),
		);

		expect(predicted).toEqual({
			selected: [FIXTURE_2, FIXTURE_1],
			expression: { type: "static" },
			revision: 7,
			gestureOpen: false,
		});
	});

	it("starts and extends an ordered fixture gesture", () => {
		const first = gestureSelectionPrediction(
			{ type: "fixture", fixtureId: FIXTURE_2 },
			[FIXTURE_2],
			false,
		)(selection({ selected: [FIXTURE_1] }));
		const extended = gestureSelectionPrediction(
			{ type: "fixture", fixtureId: FIXTURE_3 },
			[FIXTURE_3],
			false,
		)(first);
		const removed = gestureSelectionPrediction(
			{ type: "fixture", fixtureId: FIXTURE_2 },
			[FIXTURE_2],
			true,
		)(extended);

		expect(removed.selected).toEqual([FIXTURE_3]);
		expect(removed.expression).toEqual({
			type: "sources",
			items: [
				{ type: "fixture", fixtureId: FIXTURE_2 },
				{ type: "fixture", fixtureId: FIXTURE_3 },
				{ type: "remove_fixture", fixtureId: FIXTURE_2 },
			],
		});
		expect(removed.gestureOpen).toBe(true);
	});

	it("keeps live Groups symbolic and dereferenced Groups fixture-based", () => {
		const live = gestureSelectionPrediction(
			{ type: "live_group", groupId: "7" },
			[FIXTURE_1, FIXTURE_2],
			false,
		)(selection());
		const frozen = gestureSelectionPrediction(
			{ type: "dereferenced_group", groupId: "8" },
			[FIXTURE_2, FIXTURE_3],
			false,
		)(selection());

		expect(live.expression).toEqual({
			type: "sources",
			items: [{ type: "live_group", groupId: "7" }],
		});
		expect(frozen.expression).toEqual({
			type: "sources",
			items: [
				{ type: "fixture", fixtureId: FIXTURE_2 },
				{ type: "fixture", fixtureId: FIXTURE_3 },
			],
		});
	});

	it("projects live and frozen Group ownership explicitly", () => {
		const fixtures = [FIXTURE_1, FIXTURE_2, FIXTURE_3, FIXTURE_4];
		const live = groupSelectionPrediction(
			"12",
			fixtures,
			false,
			{ type: "even" },
			41,
		)(selection());
		const frozen = groupSelectionPrediction(
			"12",
			fixtures,
			true,
			{ type: "odd" },
			41,
		)(selection());

		expect(live).toMatchObject({
			selected: [FIXTURE_2, FIXTURE_4],
			expression: {
				type: "live_group",
				groupId: "12",
				rule: { type: "even" },
			},
			gestureOpen: false,
		});
		expect(frozen).toMatchObject({
			selected: [FIXTURE_1, FIXTURE_3],
			expression: {
				type: "frozen_group",
				groupId: "12",
				sourceRevision: 41,
			},
		});
	});

	it("recomputes a dependent rule from the current authoritative order", () => {
		const prediction = ruleSelectionPrediction({ type: "odd" });
		const original = prediction(
			selection({
				selected: [FIXTURE_1, FIXTURE_2, FIXTURE_3],
				expression: {
					type: "live_group",
					groupId: "7",
					rule: { type: "all" },
				},
			}),
		);
		const rebased = prediction(
			selection({
				selected: [FIXTURE_3, FIXTURE_2, FIXTURE_1, FIXTURE_4],
				expression: {
					type: "live_group",
					groupId: "7",
					rule: { type: "all" },
				},
			}),
		);

		expect(original.selected).toEqual([FIXTURE_1, FIXTURE_3]);
		expect(rebased.selected).toEqual([FIXTURE_3, FIXTURE_1]);
		expect(rebased.expression).toEqual({
			type: "live_group",
			groupId: "7",
			rule: { type: "odd" },
		});
	});

	it("supports every-Nth offsets and rejects invalid integer inputs", () => {
		const fixtures = [FIXTURE_1, FIXTURE_2, FIXTURE_3, FIXTURE_4];

		expect(
			applySelectionRule(fixtures, {
				type: "every_nth",
				n: 2,
				offset: 1,
			}),
		).toEqual([FIXTURE_2, FIXTURE_4]);
		expect(() =>
			applySelectionRule(fixtures, {
				type: "every_nth",
				n: 0,
				offset: 0,
			}),
		).toThrow("positive integer interval");
		expect(() =>
			applySelectionRule(fixtures, {
				type: "every_nth",
				n: 2,
				offset: -1,
			}),
		).toThrow("non-negative integer offset");
	});

	it("does not double-filter an already filtered live Group", () => {
		const predicted = ruleSelectionPrediction({ type: "even" })(
			selection({
				selected: [FIXTURE_1, FIXTURE_3],
				expression: {
					type: "live_group",
					groupId: "7",
					rule: { type: "odd" },
				},
			}),
		);

		expect(predicted.selected).toEqual([FIXTURE_1, FIXTURE_3]);
		expect(predicted.expression).toEqual({
			type: "live_group",
			groupId: "7",
			rule: { type: "odd" },
		});
	});
});
