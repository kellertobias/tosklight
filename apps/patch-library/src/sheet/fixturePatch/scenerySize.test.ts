import { describe, expect, it } from "vitest";
import type { FixtureProfileScenery } from "../../fixtureProfile";
import type { PatchedFixture } from "../../wire";
import {
	placedSceneryMetres,
	sceneryMeasurement,
	sceneryOf,
} from "./scenerySize";

const truss: FixtureProfileScenery = {
	kind: "truss",
	chords: 4,
	default_size_metres: { x: 4, y: 0.29, z: 0.29 },
	adjustable: { width: true, height: false, depth: false },
	minimum_size_metres: { x: 0.25, y: 0.29, z: 0.29 },
	maximum_size_metres: { x: 24, y: 0.29, z: 0.29 },
};

function venueObject(
	scenery: FixtureProfileScenery | null,
	size: PatchedFixture["scenery_size_metres"] = undefined,
): PatchedFixture {
	return {
		fixture_id: "truss-1",
		universe: null,
		address: null,
		logical_heads: [],
		scenery_size_metres: size,
		definition: {
			profile_snapshot: scenery ? { scenery } : null,
		} as unknown as PatchedFixture["definition"],
	};
}

describe("generated Venue object size", () => {
	it("reads as the profile's default until a size is placed", () => {
		const fixture = venueObject(truss);
		expect(placedSceneryMetres(fixture, truss)).toEqual({ x: 4, y: 0.29, z: 0.29 });
	});

	it("reads a placed size from the millimetres the patch stores", () => {
		const fixture = venueObject(truss, { x: 6000, y: 290, z: 290 });
		expect(placedSceneryMetres(fixture, truss).x).toBe(6);
	});

	it("sets one measurement and keeps the others as placed", () => {
		const fixture = venueObject(truss, { x: 6000, y: 290, z: 290 });
		expect(sceneryMeasurement(fixture, "scenery_width", "8.5")).toEqual({
			size: { x: 8500, y: 290, z: 290 },
		});
	});

	it("refuses a measurement outside the profile's range and says the range", () => {
		const fixture = venueObject(truss);
		expect(sceneryMeasurement(fixture, "scenery_width", "40")).toEqual({
			error: "Enter a width from 0.25 to 24 metres.",
		});
		expect(sceneryMeasurement(fixture, "scenery_width", "")).toEqual({
			error: "Enter a width from 0.25 to 24 metres.",
		});
	});

	it("offers nothing for a fixture that is not generated", () => {
		const lamp = venueObject(null);
		expect(sceneryOf(lamp)).toBeNull();
		expect(sceneryMeasurement(lamp, "scenery_width", "2")).toBeNull();
	});
});
