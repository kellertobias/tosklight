import type { CadEntity } from "./types";
import type { PatchFixtureProjection } from "@tosklight/patch";
import { describe, expect, it } from "vitest";
import { duplicateFixtures, withDuplicatePreview } from "./cadDuplicate";

function fixture(
	fixtureId: string,
	fixtureNumber: number | null,
	virtualFixtureNumber: number | null,
	extra: Partial<PatchFixtureProjection> = {},
): PatchFixtureProjection {
	return {
		fixtureId,
		fixtureNumber,
		virtualFixtureNumber,
		name: `Element ${fixtureId}`,
		profileId: "profile",
		profileRevision: 3,
		modeId: "mode",
		splitPatches: [{ split: 1, universe: 1, address: 1 }],
		layerId: "default",
		directControl: null,
		location: { x: 1000, y: 2000, z: 3000 },
		rotation: { x: 0, y: 0, z: 90 },
		multipatch: [],
		moveInBlackEnabled: true,
		moveInBlackDelayMillis: 0,
		highlightOverrides: [],
		fixtureRevision: 7,
		logicalHeads: [],
		...extra,
	} as PatchFixtureProjection;
}

describe("duplicating CAD elements", () => {
	it("gives each copy its own ID and the next free numbers, unpatched and a step along the view", () => {
		const all = [
			fixture("lamp", 101, null),
			fixture("taken", 102, null),
			fixture("truss", null, 4, { splitPatches: [] }),
			fixture("other", null, 5),
		];
		let counter = 0;
		const copies = duplicateFixtures(all, ["lamp", "truss", "missing"], [500, 0, -20], () => `new-${++counter}`);
		expect(copies).toHaveLength(2);
		const [lamp, truss] = copies;
		expect(lamp).toMatchObject({
			fixtureId: "new-1",
			fixtureNumber: 103,
			virtualFixtureNumber: null,
			name: "Element lamp",
			profileId: "profile",
			modeId: "mode",
			rotation: { x: 0, y: 0, z: 90 },
			splitPatches: [{ split: 1, universe: null, address: null }],
			location: { x: 1500, y: 2000, z: 2980 },
			fixtureRevision: 0,
		});
		expect(truss).toMatchObject({ fixtureId: "new-2", fixtureNumber: null, virtualFixtureNumber: 6 });
		// The originals are untouched.
		expect(all[0].splitPatches).toEqual([{ split: 1, universe: 1, address: 1 }]);
	});

	it("gives two copies of one element two different numbers", () => {
		const all = [fixture("a", 1, null), fixture("b", 2, null)];
		const copies = duplicateFixtures(all, ["a", "b"], [0, 0, 0]);
		expect(copies.map((copy) => copy.fixtureNumber)).toEqual([3, 4]);
		expect(new Set(copies.map((copy) => copy.fixtureId)).size).toBe(2);
	});

	it("copies multi-patch placements with new IDs, unpatched, at the same step", () => {
		const all = [
			fixture("lamp", 1, null, {
				multipatch: [
					{
						id: "copy",
						name: "",
						splitPatches: [{ split: 1, universe: 2, address: 10 }],
						location: { x: 0, y: 0, z: 0 },
						rotation: { x: 0, y: 0, z: 0 },
					},
				],
			}),
		];
		const [copy] = duplicateFixtures(all, ["lamp"], [0, 500, 0], () => "id");
		expect(copy.multipatch).toEqual([
			expect.objectContaining({
				id: "id",
				splitPatches: [{ split: 1, universe: null, address: null }],
				location: { x: 0, y: 500, z: 0 },
			}),
		]);
	});
});

describe("a duplicating move's preview", () => {
	it("leaves every placement of the copied fixtures and moves a copy of each", () => {
		const base = {
			selectable: true,
			rotationDegrees: [0, 0, 0],
			sizeMillimetres: [400, 400, 400],
		};
		const lamp = { ...base, id: "a", logicalFixtureId: "a", positionMillimetres: [0, 0, 4000] };
		const lampCopy = { ...base, id: "a-2", logicalFixtureId: "a", positionMillimetres: [1000, 0, 4000] };
		const other = { ...base, id: "b", logicalFixtureId: "b", positionMillimetres: [0, 2000, 0] };
		const entities = [lamp, lampCopy, other] as unknown as CadEntity[];
		const preview = {
			entityIds: ["a"],
			deltaMillimetres: [500, 0, 0] as [number, number, number],
			spread: false,
			duplicate: true,
		};
		const shown = withDuplicatePreview(entities, preview);
		expect(shown.entities.slice(0, 3)).toEqual(entities);
		expect(shown.entities.slice(3).map((entity) => [entity.id, entity.positionMillimetres])).toEqual([
			["a:copy", [500, 0, 4000]],
			["a-2:copy", [1500, 0, 4000]],
		]);
		// The originals no longer follow the preview; the gizmo still does through its delta.
		expect(shown.preview).toEqual({ ...preview, entityIds: [] });
		// An ordinary move is left as it is.
		const move = { ...preview, duplicate: undefined };
		expect(withDuplicatePreview(entities, move)).toEqual({ entities, preview: move });
	});
});
