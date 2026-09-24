import type {
	FixtureProfileScenery,
	PatchFixtureProjection,
	PatchProfileRevision,
} from "@tosklight/patch";
import { describe, expect, it } from "vitest";
import { sharedModel, sharedModelFields, THRU_FIELDS } from "./thruFields";

const riser = (): FixtureProfileScenery => ({
	kind: "riser",
	chords: 0,
	default_size_metres: { x: 2, y: 0.4, z: 1 },
	adjustable: { width: false, height: true, depth: false },
	minimum_size_metres: { x: 2, y: 0.1, z: 1 },
	maximum_size_metres: { x: 2, y: 1.2, z: 1 },
});

const fixture = (id: string, profileId: string, over: Partial<PatchFixtureProjection> = {}) =>
	({
		fixtureId: id,
		fixtureNumber: null,
		virtualFixtureNumber: 1,
		name: id,
		profileId,
		profileRevision: 1,
		modeId: "mode",
		splitPatches: [],
		layerId: "layer",
		directControl: null,
		location: { x: 0, y: 0, z: 0 },
		rotation: { x: 0, y: 0, z: 0 },
		multipatch: [],
		moveInBlackEnabled: false,
		moveInBlackDelayMillis: 0,
		highlightOverrides: [],
		fixtureRevision: 1,
		logicalHeads: [],
		...over,
	}) as PatchFixtureProjection;

const revision = (
	profileId: string,
	over: Partial<PatchProfileRevision> = {},
): PatchProfileRevision =>
	({
		profileId,
		profileRevision: 1,
		contentDigest: "digest",
		manufacturer: "Generic",
		name: "Stage Element 2 × 1 m",
		fixtureType: "venue",
		patchPolicy: "visual_only",
		referencedModes: [],
		profileSnapshot: { scenery: riser() },
		...over,
	}) as PatchProfileRevision;

describe("the model a selection shares", () => {
	it("is the profile every selected element was patched from", () => {
		const model = sharedModel(
			[fixture("a", "deck"), fixture("b", "deck"), fixture("c", "deck")],
			[revision("deck")],
		);
		expect(model?.profileId).toBe("deck");
		expect(model?.label).toBe("Generic Stage Element 2 × 1 m");
		expect(model?.scenery?.kind).toBe("riser");
	});

	it("is nothing when the elements were patched from different profiles", () => {
		expect(
			sharedModel([fixture("a", "deck-2x1"), fixture("b", "deck-1x1")], [revision("deck-2x1")]),
		).toBeNull();
	});

	it("is nothing for a single element, which Info shows on its own", () => {
		expect(sharedModel([fixture("a", "deck")], [revision("deck")])).toBeNull();
	});

	it("resolves the revision the first selected element stands on when they differ", () => {
		const model = sharedModel(
			[fixture("a", "deck", { profileRevision: 2 }), fixture("b", "deck")],
			[revision("deck"), revision("deck", { profileRevision: 2, name: "Stage Element, corrected" })],
		);
		expect(model?.label).toBe("Generic Stage Element, corrected");
	});

	it("still names the profile id when the patch carries no revision for it", () => {
		const model = sharedModel([fixture("a", "deck"), fixture("b", "deck")], []);
		expect(model).toEqual({
			profileId: "deck",
			label: "",
			scenery: null,
			sizing: null,
			crowd: false,
		});
	});
});

describe("the controls a shared model offers", () => {
	const model = () => sharedModel([fixture("a", "deck"), fixture("b", "deck")], [revision("deck")])!;

	it("offers only the measurements the profile lets the operator set", () => {
		expect(sharedModelFields(model(), true).map((field) => field.id)).toEqual(["size-y"]);
	});

	it("writes the measurement to an element in millimetres, keeping the fixed axes", () => {
		const [height] = sharedModelFields(model(), true);
		const written = height.write(fixture("a", "deck"), 0.8);
		expect(written.scenerySizeMetres).toEqual({ x: 2000, y: 800, z: 1000 });
	});

	it("reads the height an element is placed at, and its profile default when it stores none", () => {
		const [height] = sharedModelFields(model(), true);
		expect(height.read(fixture("a", "deck"))).toBe(0.4);
		expect(
			height.read(fixture("a", "deck", { scenerySizeMetres: { x: 2000, y: 600, z: 1000 } })),
		).toBe(0.6);
	});

	it("holds a spread inside the profile's range rather than building an impossible element", () => {
		const [height] = sharedModelFields(model(), true);
		expect(height.write(fixture("a", "deck"), 5).scenerySizeMetres?.y).toBe(1200);
		expect(height.write(fixture("a", "deck"), 0).scenerySizeMetres?.y).toBe(100);
	});

	it("offers scale instead when the shared model is a placed model with no measurements", () => {
		const placed = sharedModel(
			[fixture("a", "prop"), fixture("b", "prop")],
			[revision("prop", { profileSnapshot: { scenery: null } as never })],
		)!;
		expect(sharedModelFields(placed, true).map((field) => field.id)).toEqual(["scale"]);
		const [scale] = sharedModelFields(placed, true);
		expect(scale.write(fixture("a", "prop"), 2).modelScale).toBe(2);
		// Built size is stored as nothing at all, as the single-element panel stores it.
		expect(scale.write(fixture("a", "prop"), 1).modelScale).toBeNull();
	});

	it("offers a lamp no scale, since only a Venue object is drawn at another size", () => {
		const lamp = sharedModel(
			[fixture("a", "lamp"), fixture("b", "lamp")],
			[revision("lamp", { profileSnapshot: { scenery: null } as never })],
		)!;
		expect(sharedModelFields(lamp, false)).toEqual([]);
	});

	it("offers a crowd area its width and depth rather than a scale, within 1 to 250 m", () => {
		const crowd = sharedModel(
			[fixture("a", "crowd"), fixture("b", "crowd")],
			[
				revision("crowd", {
					profileSnapshot: {
						scenery: null,
						crowd: { default_width_metres: 5, default_depth_metres: 3, modes: [] },
						physical: { height_millimetres: 1780 },
					} as never,
				}),
			],
		)!;
		const fields = sharedModelFields(crowd, true);
		expect(fields.map((field) => field.id)).toEqual(["size-x", "size-z"]);
		const [width, depth] = fields;
		expect(width.read(fixture("a", "crowd"))).toBe(5);
		expect(depth.read(fixture("a", "crowd"))).toBe(3);
		// The height stored beside them is the people's, never set here.
		expect(width.write(fixture("a", "crowd"), 12).scenerySizeMetres).toEqual({
			x: 12000,
			y: 1780,
			z: 3000,
		});
		expect(depth.write(fixture("a", "crowd"), 400).scenerySizeMetres?.z).toBe(250_000);
		expect(depth.write(fixture("a", "crowd"), 0.2).scenerySizeMetres?.z).toBe(1000);
	});
});

describe("the placement fields every selection keeps", () => {
	it("still covers position, rotation, bracket angle and barn doors", () => {
		expect(THRU_FIELDS.map((field) => field.id)).toEqual([
			"position-x",
			"position-y",
			"position-z",
			"rotation-x",
			"rotation-y",
			"rotation-z",
			"bracket",
			"barndoors",
		]);
		expect(THRU_FIELDS.filter((field) => field.lampsOnly).map((field) => field.id)).toEqual([
			"bracket",
			"barndoors",
		]);
	});
});
