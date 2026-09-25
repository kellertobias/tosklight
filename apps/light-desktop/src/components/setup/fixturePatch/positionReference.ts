import type { PatchedFixture } from "../../../api/types";
import { fixtureDisplayId } from "./fixtureIds";

/** The attribute that makes a fixture a 3D Point, the same test the desk and the Stage apply. */
const POINT_ATTRIBUTE = "point.position.x";

/**
 * Whether a fixture is a 3D Point: a reference object other fixtures and Venue objects can take as
 * their Position Reference. The desk decides this by what the fixture carries, not by its name.
 */
export function isPositionPoint(fixture: PatchedFixture): boolean {
	const profile = fixture.definition.profile_snapshot;
	const mode =
		profile?.modes.find((candidate) => candidate.id === fixture.definition.mode_id) ??
		profile?.modes[0];
	if (mode?.channels.some((channel) => channel.attribute === POINT_ATTRIBUTE))
		return true;
	return fixture.definition.heads.some((head) =>
		head.parameters.some((parameter) => parameter.attribute === POINT_ATTRIBUTE),
	);
}

/** Every 3D Point in the patch, in table order. */
export function positionPoints(fixtures: readonly PatchedFixture[]): PatchedFixture[] {
	return fixtures.filter(isPositionPoint);
}

/**
 * The Position Reference column appears once the show holds a 3D Point and goes again when the
 * last one is removed: with nothing to reference, the column would be a row of dashes.
 */
export function positionReferenceColumnAvailable(
	fixtures: readonly PatchedFixture[],
): boolean {
	return fixtures.some(isPositionPoint);
}

/** How a point is named wherever an operator picks one: its fixture ID, then its name. */
export function positionPointLabel(point: PatchedFixture): string {
	return `${fixtureDisplayId(point)} · ${point.name || point.definition.name}`;
}

/**
 * What a fixture's Position Reference cell reads. A fixture that references a point the show no
 * longer holds reads as none: it is drawn against the stage, and the next write stores it so.
 */
export function positionReferenceLabel(
	fixture: PatchedFixture,
	fixtures: readonly PatchedFixture[],
): string {
	const master = fixture.position_master;
	if (!master) return "None";
	const point = fixtures.find((candidate) => candidate.fixture_id === master);
	return point && isPositionPoint(point) ? positionPointLabel(point) : "None";
}
