import type { FixtureDefinition, PatchFixtureProjection } from "@tosklight/patch";
import { describe, expect, it } from "vitest";
import { newerRevision, upgraded } from "./profileUpgrade";

const TRUSS = "562e7947-8284-5ec8-9750-3cd3fe6c1c6d";

const scenery = (section: number) => ({
	kind: "truss",
	chords: 4,
	pattern: "standard",
	default_size_metres: { x: 4, y: section, z: section },
	minimum_size_metres: { x: 0.25, y: section, z: section },
	maximum_size_metres: { x: 24, y: section, z: section },
	adjustable: { width: true, height: false, depth: false },
});

const definition = (
	revision: number,
	{ mode = "Default", modeId = "mode-default", section = 0.29 } = {},
) =>
	({
		id: `${TRUSS}:${modeId}`,
		revision,
		name: "Four-Point Truss",
		mode,
		mode_id: modeId,
		profile_id: TRUSS,
		profile_snapshot: {
			id: TRUSS,
			revision,
			modes: [{ id: modeId, name: mode }],
			scenery: scenery(section),
		},
	}) as unknown as FixtureDefinition;

const fixture = (revision: number, size?: { x: number; y: number; z: number }) =>
	({
		fixtureId: "fixture",
		name: "Four-Point Truss",
		profileId: TRUSS,
		profileRevision: revision,
		modeId: "mode-default",
		scenerySizeMetres: size ?? null,
		multipatch: [],
	}) as unknown as PatchFixtureProjection;

describe("the offer of a newer profile revision", () => {
	it("offers the newest revision in the library and nothing once the element is on it", () => {
		const library = [definition(3), definition(5), definition(4)];
		expect(newerRevision(library, fixture(4))).toMatchObject({
			profileRevision: 5,
			modeId: "mode-default",
		});
		expect(newerRevision(library, fixture(5))).toBeNull();
		expect(newerRevision(library, fixture(9))).toBeNull();
		expect(newerRevision([], fixture(1))).toBeNull();
	});

	it("keeps the fixture in its own mode when the new revision numbered its modes differently", () => {
		const library = [
			definition(4, { mode: "Extended", modeId: "old-extended" }),
			definition(6, { mode: "Basic", modeId: "new-basic" }),
			definition(6, { mode: "Extended", modeId: "new-extended" }),
		];
		expect(newerRevision(library, fixture(4), "Extended")).toMatchObject({
			modeId: "new-extended",
		});
		// Without a name to go by it is the revision's first mode, not a guess at another one.
		expect(newerRevision(library, fixture(4))).toMatchObject({ modeId: "new-basic" });
	});

	it("moves only the profile reference, and resets the measurements the operator cannot set", () => {
		const target = newerRevision([definition(2, { section: 0.29 })], fixture(1));
		if (!target) throw new Error("the newer revision is offered");
		// The old 340 mm truss the operator stretched to 8 m: the length it was given is kept, the
		// section it never chose comes back at the corrected 290 mm.
		const next = upgraded(fixture(1, { x: 8000, y: 340, z: 340 }), target);
		expect(next).toMatchObject({
			profileRevision: 2,
			modeId: "mode-default",
			scenerySizeMetres: { x: 8000, y: 290, z: 290 },
		});
		expect(next.name).toBe("Four-Point Truss");
		// An element at its profile's own size has nothing stored, and keeps nothing stored.
		expect(upgraded(fixture(1), target).scenerySizeMetres).toBeNull();
	});
});
