import { describe, expect, it } from "vitest";
import type { CueList, PatchedFixture } from "../api/types";
import {
	activeProgrammerFixtureIds,
	compareFixtureIds,
	cueListFixtureIds,
	fixtureIsIncluded,
	fixtureSheetIncludesFixture,
} from "./fixtureSheetFilters";

const groups = [
	{
		id: "front",
		body: { fixtures: ["fixture-2", "head-3"] },
	},
];

describe("fixture sheet filters", () => {
	const fixture = ({
		fixtureId = "fixture-1",
		fixtureNumber = 1,
		virtualFixtureNumber = null,
		manufacturer = "Test",
		patchPolicy = "dmx",
	}: {
		fixtureId?: string;
		fixtureNumber?: number | null;
		virtualFixtureNumber?: number | null;
		manufacturer?: string;
		patchPolicy?: "dmx" | "visual_only";
	} = {}) =>
		({
			fixture_id: fixtureId,
			fixture_number: fixtureNumber,
			virtual_fixture_number: virtualFixtureNumber,
			universe: null,
			address: null,
			definition: {
				manufacturer,
				profile_snapshot: { patch_policy: patchPolicy },
			},
			logical_heads: [],
		}) as unknown as PatchedFixture;

	it("independently excludes visual-only, Venue, and complete 0.x identities", () => {
		expect(
			fixtureSheetIncludesFixture(
				fixture({ manufacturer: "Touring", patchPolicy: "visual_only" }),
			),
		).toBe(false);
		expect(
			fixtureSheetIncludesFixture(
				fixture({ manufacturer: "Venue", patchPolicy: "dmx" }),
			),
		).toBe(false);
		expect(
			fixtureSheetIncludesFixture(
				fixture({
					fixtureId: "legacy-scenery",
					fixtureNumber: null,
					virtualFixtureNumber: 7,
				}),
			),
		).toBe(false);
		expect(
			fixtureSheetIncludesFixture(
				fixture({ fixtureId: "0.imported", fixtureNumber: null }),
			),
		).toBe(false);
	});

	it("retains ordinary 100-series fixtures whose rendered heads contain .0", () => {
		const fixture100 = fixture({
			fixtureId: "fixture-100",
			fixtureNumber: 100,
		});
		fixture100.logical_heads = [{ fixture_id: "head-100.1", head_index: 1 }];

		expect(fixtureSheetIncludesFixture(fixture100)).toBe(true);
		expect(
			fixtureSheetIncludesFixture(
				fixture({ fixtureId: "100.0", fixtureNumber: null }),
			),
		).toBe(true);
		expect(
			fixtureSheetIncludesFixture(
				fixture({ fixtureId: "100.1", fixtureNumber: null }),
			),
		).toBe(true);
	});

	it("includes direct and group programmer fixtures", () => {
		const programmer = {
			fixtureIds: ["fixture-1"],
			groupIds: ["front"],
		};
		expect([...activeProgrammerFixtureIds(programmer, groups)].sort()).toEqual([
			"fixture-1",
			"fixture-2",
			"head-3",
		]);
	});

	it("resolves canonical multi-reference membership for filters", () => {
		const canonicalGroups = [
			{
				id: "left",
				body: {
					fixtures: ["stale-left"],
					source: {
						type: "explicit" as const,
						fixture_ids: ["fixture-1", "fixture-2"],
					},
				},
			},
			{
				id: "combined",
				body: {
					fixtures: ["stale-combined"],
					source: {
						type: "references" as const,
						references: [{ group_id: "left", rule: { type: "even" as const } }],
					},
				},
			},
		];

		expect(
			activeProgrammerFixtureIds(
				{ fixtureIds: [], groupIds: ["combined"] },
				canonicalGroups,
			),
		).toEqual(new Set(["fixture-2"]));
	});

	it("skips stored-empty, missing, and deleted group targets", () => {
		const programmer = {
			fixtureIds: [],
			groupIds: ["empty", "missing"],
		};
		expect(
			activeProgrammerFixtureIds(programmer, [
				{ id: "empty", body: { fixtures: [] } },
			]),
		).toEqual(new Set());
	});

	it("includes fixtures used directly or through groups anywhere in a cue list", () => {
		const cueList = {
			cues: [
				{
					changes: [{ fixture_id: "fixture-1" }],
					group_changes: [{ group_id: "front" }],
				},
			],
		} as unknown as CueList;
		expect([...(cueListFixtureIds(cueList, groups) ?? [])].sort()).toEqual([
			"fixture-1",
			"fixture-2",
			"head-3",
		]);
	});

	it("matches logical heads and orders missing fixture numbers last", () => {
		const fixture = {
			fixture_id: "fixture-3",
			logical_heads: [{ fixture_id: "head-3", head_index: 0 }],
		} as PatchedFixture;
		expect(fixtureIsIncluded(fixture, new Set(["head-3"]))).toBe(true);
		expect(
			compareFixtureIds(
				{ fixture_id: "missing", fixture_number: null } as PatchedFixture,
				{ fixture_id: "numbered", fixture_number: 12 } as PatchedFixture,
			),
		).toBeGreaterThan(0);
		expect(
			compareFixtureIds(
				{
					fixture_id: "visual",
					fixture_number: null,
					virtual_fixture_number: 1,
				} as PatchedFixture,
				{ fixture_id: "numbered", fixture_number: 1 } as PatchedFixture,
			),
		).toBeLessThan(0);
	});
});
