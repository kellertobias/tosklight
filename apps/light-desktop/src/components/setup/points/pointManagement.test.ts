import { describe, expect, it } from "vitest";
import type { FixtureDefinition, PatchedFixture } from "../../../api/types";
import {
	movedPointLocation,
	newPointCandidate,
	nextPointName,
	nextPointNumber,
	pointDefinition,
} from "./pointManagement";

/** TL-651: Points are unpatched 3D Point fixtures; the helpers name, number and place them. */

function definition(mode: string, name = "3D Point") {
	return {
		manufacturer: "ToskLight",
		name,
		mode,
		mode_id: mode,
		profile_id: "point-profile",
		profile_revision: 1,
		profile_snapshot: null,
		heads: [
			{ parameters: [{ attribute: name === "3D Point" ? "point.position.x" : "dimmer" }] },
		],
		channels: [],
		splits: [],
	} as unknown as FixtureDefinition;
}

function fixture(number: number | null, name: string, point = false) {
	return {
		fixture_id: `fixture-${number}-${name}`,
		fixture_number: number,
		virtual_fixture_number: null,
		name,
		definition: definition("Full 24 bit", point ? "3D Point" : "Spot"),
		location: { x: 1000, y: 2000, z: 3000 },
	} as unknown as PatchedFixture;
}

describe("Point management helpers", () => {
	it("finds the shipped 3D Point in Full 24 bit, or its first mode", () => {
		const library = [
			definition("Position 16 bit"),
			definition("Full 24 bit"),
			definition("Full 24 bit", "Other"),
		];
		expect(pointDefinition(library)?.mode).toBe("Full 24 bit");
		expect(pointDefinition([definition("Position 16 bit")])?.mode).toBe("Position 16 bit");
		expect(pointDefinition([definition("Full 24 bit", "Spot")])).toBeNull();
	});

	it("numbers a new Point after the highest fixture ID and names it after the Point count", () => {
		const fixtures = [fixture(1, "Spot 1"), fixture(12, "Spot 12"), fixture(901, "Point 1", true)];
		expect(nextPointNumber(fixtures)).toBe(902);
		expect(nextPointNumber([])).toBe(1);
		expect(nextPointName(fixtures)).toBe("Point 2");
		expect(nextPointName([fixture(5, "Point 1")])).toBe("Point 2");
	});

	it("creates an unpatched Point at the given location in millimetres", () => {
		const candidate = newPointCandidate(definition("Full 24 bit"), [fixture(3, "Spot 3")], {
			name: "  Singer ",
			locationMetres: { x: 1.25, y: -2, z: 0.5 },
		});
		expect(candidate?.fixture).toMatchObject({
			name: "Singer",
			fixture_number: 4,
			universe: null,
			address: null,
			location: { x: 1250, y: -2000, z: 500 },
		});
		expect(candidate?.input.splitPatches).toEqual([
			expect.objectContaining({ universe: null, address: null }),
		]);
	});

	it("moves one axis of a Point's location", () => {
		expect(movedPointLocation(fixture(9, "Point 1", true), "z", -1.5)).toEqual({
			x: 1000,
			y: 2000,
			z: -1500,
		});
	});
});
