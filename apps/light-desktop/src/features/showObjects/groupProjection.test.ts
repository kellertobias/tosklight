import { describe, expect, it } from "vitest";
import type { StoredGroup } from "../../api/types";
import type { ShowObject } from "./contracts";
import {
	groupReferenceIds,
	projectLiveGroupMembership,
	resolveGroupMembership,
} from "./groupProjection";

function group(id: string, body: Partial<StoredGroup>): ShowObject<"group"> {
	return {
		kind: "group",
		id,
		revision: 1,
		updated_at: "2026-08-02T00:00:00Z",
		body: { fixtures: [], ...body },
	};
}

function bodyAt(groups: readonly ShowObject<"group">[], index: number) {
	const candidate = groups[index];
	if (!candidate) throw new Error(`Missing Group at index ${index}`);
	return candidate.body;
}

describe("canonical desktop Group projection", () => {
	it("uses a source-only explicit source with first-occurrence deduplication", () => {
		const mapping = {
			projection: {
				anchor: { x: 0, y: 0, z: 0 },
				view_direction: { x: 0, y: 0, z: -1 },
				rotation_degrees: 0,
			},
			shape: { type: "grid", angle_degrees: 0, direction: "ascending" },
		} as const;
		const projected = projectLiveGroupMembership([
			group("1", {
				fixtures: ["stale-cache"],
				source: {
					type: "explicit",
					fixture_ids: ["a", "b", "a", "c"],
				},
				mapping,
				derived_from: {
					source_group_id: "legacy",
					rule: { type: "all" },
				},
			}),
		]);

		expect(projected[0]?.body.fixtures).toEqual(["a", "b", "c"]);
		expect(projected[0]?.body.source).toEqual({
			type: "explicit",
			fixture_ids: ["a", "b", "a", "c"],
		});
		expect(projected[0]?.body.mapping).toBe(mapping);
		expect(groupReferenceIds(bodyAt(projected, 0))).toEqual([]);
	});

	it("resolves nested and multiple references in stored order", () => {
		const groups = [
			group("base", {
				source: {
					type: "explicit",
					fixture_ids: ["a", "b", "c", "d"],
				},
			}),
			group("nested", {
				source: {
					type: "references",
					references: [{ group_id: "base", rule: { type: "odd" } }],
				},
			}),
			group("other", {
				source: {
					type: "explicit",
					fixture_ids: ["c", "e", "a"],
				},
			}),
			group("combined", {
				source: {
					type: "references",
					references: [
						{ group_id: "nested", rule: { type: "all" } },
						{ group_id: "other", rule: { type: "even" } },
					],
				},
			}),
		];

		expect(resolveGroupMembership(groups).get("combined")).toEqual([
			"a",
			"c",
			"e",
		]);
		expect(groupReferenceIds(bodyAt(groups, 3))).toEqual(["nested", "other"]);
	});

	it("keeps empty canonical sources distinct from absent Groups", () => {
		const membership = resolveGroupMembership([
			group("empty", {
				fixtures: ["legacy-cache"],
				source: { type: "explicit", fixture_ids: [] },
			}),
		]);
		expect(membership.has("empty")).toBe(true);
		expect(membership.get("empty")).toEqual([]);
		expect(membership.has("absent")).toBe(false);
	});

	it("keeps unavailable canonical references empty without mutating their compatibility cache", () => {
		const groups = [
			group("missing", {
				fixtures: ["cached-missing"],
				source: {
					type: "references",
					references: [{ group_id: "absent", rule: { type: "all" } }],
				},
			}),
			group("cycle-a", {
				fixtures: ["cached-a"],
				source: {
					type: "references",
					references: [{ group_id: "cycle-b", rule: { type: "all" } }],
				},
			}),
			group("cycle-b", {
				fixtures: ["cached-b"],
				source: {
					type: "references",
					references: [{ group_id: "cycle-a", rule: { type: "all" } }],
				},
			}),
		];
		const membership = resolveGroupMembership(groups);
		const projected = projectLiveGroupMembership(groups);

		expect(membership.get("missing")).toEqual([]);
		expect(membership.get("cycle-a")).toEqual([]);
		expect(membership.get("cycle-b")).toEqual([]);
		expect(projected.map((candidate) => candidate.body.fixtures)).toEqual([
			[],
			[],
			[],
		]);
		expect(groups.map((candidate) => candidate.body.fixtures)).toEqual([
			["cached-missing"],
			["cached-a"],
			["cached-b"],
		]);
	});

	it("does not revive legacy authority for a malformed canonical source", () => {
		const malformed = group("malformed", {
			fixtures: ["cached"],
			derived_from: {
				source_group_id: "legacy-source",
				rule: { type: "all" },
			},
		});
		malformed.body.source = { type: "explicit" } as never;
		const membership = resolveGroupMembership([
			group("legacy-source", { fixtures: ["legacy"] }),
			malformed,
		]);

		expect(membership.get("malformed")).toEqual([]);
		expect(malformed.body.fixtures).toEqual(["cached"]);
	});

	it("falls back to legacy derived_from when canonical source is unavailable", () => {
		const groups = [
			group("base", { fixtures: ["a", "b", "c"] }),
			group("legacy", {
				fixtures: ["stale"],
				derived_from: {
					source_group_id: "base",
					rule: { type: "even" },
				},
			}),
		];

		expect(resolveGroupMembership(groups).get("legacy")).toEqual(["b"]);
		expect(groupReferenceIds(bodyAt(groups, 1))).toEqual(["base"]);
	});
});
