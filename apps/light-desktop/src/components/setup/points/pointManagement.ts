import type { FixtureDefinition, PatchedFixture } from "../../../api/types";
import {
	newPatchFixtureCandidate,
	type PatchFixtureCandidate,
} from "../../../features/patch/model";
import { nextAvailableFixtureNumber } from "../fixturePatch/fixtureIds";
import { positionPoints } from "../fixturePatch/positionReference";

/**
 * TL-651: Points are the show's 3D Points — ordinary fixtures of the shipped **ToskLight → 3D
 * Point** profile, so a Point keeps one stable identity (its fixture UUID) whether it is patched,
 * left without an address, or bound to a PosiStageNet tracker. The Points view creates and edits
 * exactly those fixtures; there is no second target store.
 */
export const POINT_MANUFACTURER = "ToskLight";
export const POINT_PROFILE = "3D Point";
/**
 * The mode a Point is created in. Unpatched it sends nothing, so the mode only matters once the
 * Point is patched; Full 24 bit carries position and rotation, the mode older points map onto.
 */
export const POINT_MODE = "Full 24 bit";

/** The library definition a new Point uses, or null when the library does not hold it. */
export function pointDefinition(
	library: readonly FixtureDefinition[],
): FixtureDefinition | null {
	const profile = library.filter(
		(candidate) =>
			candidate.manufacturer === POINT_MANUFACTURER &&
			candidate.name === POINT_PROFILE,
	);
	return profile.find((candidate) => candidate.mode === POINT_MODE) ?? profile[0] ?? null;
}

/** The next free fixture number after the highest one the show uses. */
export function nextPointNumber(fixtures: readonly PatchedFixture[]): number | null {
	const used = new Set(
		fixtures.flatMap((fixture) =>
			fixture.fixture_number == null ? [] : [fixture.fixture_number],
		),
	);
	const highest = Math.max(0, ...used);
	return nextAvailableFixtureNumber(highest + 1, used);
}

/** `Point N`, with N one past the show's Point count and never a name another fixture uses. */
export function nextPointName(fixtures: readonly PatchedFixture[]): string {
	const names = new Set(fixtures.map((fixture) => fixture.name));
	let index = positionPoints(fixtures).length + 1;
	while (names.has(`Point ${index}`)) index++;
	return `Point ${index}`;
}

export interface NewPointInput {
	name?: string;
	/** Installed location in metres; the stage origin when absent. */
	locationMetres?: { x: number; y: number; z: number };
}

/** An aim Point without a DMX address: an unpatched 3D Point fixture. */
export function newPointCandidate(
	definition: FixtureDefinition,
	fixtures: readonly PatchedFixture[],
	input: NewPointInput = {},
): PatchFixtureCandidate | null {
	const number = nextPointNumber(fixtures);
	if (number === null) return null;
	const metres = input.locationMetres ?? { x: 0, y: 0, z: 0 };
	return newPatchFixtureCandidate({
		name: input.name?.trim() || nextPointName(fixtures),
		fixture_number: number,
		definition,
		universe: null,
		address: null,
		location: {
			x: Math.round(metres.x * 1000),
			y: Math.round(metres.y * 1000),
			z: Math.round(metres.z * 1000),
		},
	});
}

export type PointAxis = "x" | "y" | "z";

/** A Point's installed location with one axis replaced, in the patch's millimetres. */
export function movedPointLocation(
	point: PatchedFixture,
	axis: PointAxis,
	metres: number,
): { x: number; y: number; z: number } {
	const location = point.location ?? { x: 0, y: 0, z: 0 };
	return { ...location, [axis]: Math.round(metres * 1000) };
}
