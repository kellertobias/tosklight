import { describe, expect, it } from "vitest";
import type { Group } from "./model";
import {
	defaultSpatialMapping,
	resolveMappingPresentation,
	validateSpatialMapping,
} from "./spatialMapping";

function group(id: string, body: Record<string, unknown>): Group {
	return {
		kind: "group",
		id,
		revision: 1,
		updated_at: "",
		body: {
			name: `Group ${id}`,
			fixtures: [],
			programming: {},
			...body,
		},
	} as Group;
}

describe("Group spatial mapping presentation", () => {
	it("distinguishes none, local, inherited, and mixed source mappings", () => {
		const top = defaultSpatialMapping();
		const front = {
			...defaultSpatialMapping(),
			projection: {
				...defaultSpatialMapping().projection,
				view_direction: { x: 0, y: 1, z: 0 },
				preset: "front" as const,
			},
		};
		const localTop = group("1", { mapping: top });
		const localFront = group("2", { mapping: front });
		const inherited = group("3", {
			source: {
				type: "references",
				references: [{ group_id: "1", rule: { type: "all" } }],
			},
		});
		const mixed = group("4", {
			source: {
				type: "references",
				references: [
					{ group_id: "1", rule: { type: "all" } },
					{ group_id: "2", rule: { type: "all" } },
				],
			},
		});

		expect(resolveMappingPresentation(group("5", {}), [localTop])).toEqual({
			type: "none",
			label: "Mapping: None",
			mapping: null,
		});
		expect(resolveMappingPresentation(localTop, [localTop])).toMatchObject({
			type: "local",
			label: "Local override",
		});
		expect(
			resolveMappingPresentation(inherited, [localTop, inherited]),
		).toMatchObject({
			type: "inherited",
			label: "Inherited from Group 1",
			sourceGroupIds: ["1"],
		});
		expect(
			resolveMappingPresentation(mixed, [localTop, localFront, mixed]),
		).toEqual({
			type: "mixed",
			label: "Mixed source mappings — source order",
			mapping: null,
		});
	});

	it("rejects non-finite fields and a zero view direction before a write", () => {
		const zeroDirection = defaultSpatialMapping();
		zeroDirection.projection.view_direction = { x: 0, y: 0, z: 0 };
		expect(validateSpatialMapping(zeroDirection)).toBe(
			"View direction must not be zero.",
		);

		const nonFinite = defaultSpatialMapping();
		nonFinite.shape = {
			type: "radar",
			center_u: 0,
			center_v: Number.NaN,
			start_angle_degrees: 0,
			sweep: "clockwise",
		};
		expect(validateSpatialMapping(nonFinite)).toBe(
			"Every projection and Phase value must be a finite number.",
		);
	});
});
