import type { PatchedFixture } from "../../api/types";
import type { PatchStoreSnapshot } from "./store";

export const EMPTY_FIXTURES: readonly PatchedFixture[] = [];

export interface PatchStatus {
	status: PatchStoreSnapshot["status"];
	error: string | null;
}

export function selectPatchedFixtures(
	snapshot: PatchStoreSnapshot,
): readonly PatchedFixture[] {
	return snapshot.fixtures;
}

export function selectPatchStatus(snapshot: PatchStoreSnapshot): PatchStatus {
	return { status: snapshot.status, error: snapshot.error };
}

/** Exact top-level fixtures for the requested storage IDs, in authoritative Patch order. */
export function selectFixturesById(
	snapshot: PatchStoreSnapshot,
	fixtureIds: ReadonlySet<string>,
): readonly PatchedFixture[] {
	if (!fixtureIds.size) return EMPTY_FIXTURES;
	return snapshot.fixtures.filter((fixture) =>
		fixtureIds.has(fixture.fixture_id),
	);
}

/**
 * Fixtures a selection addresses, matching either the top-level fixture or any of its logical
 * heads, so a head-level selection still resolves its owning fixture exactly once.
 */
export function selectFixturesForSelection(
	snapshot: PatchStoreSnapshot,
	selectedFixtureIds: ReadonlySet<string>,
): readonly PatchedFixture[] {
	if (!selectedFixtureIds.size) return EMPTY_FIXTURES;
	return snapshot.fixtures.filter(
		(fixture) =>
			selectedFixtureIds.has(fixture.fixture_id) ||
			fixture.logical_heads.some((head) =>
				selectedFixtureIds.has(head.fixture_id),
			),
	);
}
