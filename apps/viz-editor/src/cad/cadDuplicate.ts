/**
 * Duplicating selected elements from the CAD object menu.
 *
 * A copy is a new element of the show with its own fixture ID, so it is selected, moved and deleted
 * on its own. It keeps everything the original is — profile, mode, size, rotation, name — and
 * stands a fixed step to the right of it on the view it was duplicated in, so it is never hidden
 * under the original. Its fixture and virtual numbers are the next free ones above the original's,
 * and it is unpatched: two elements on one DMX address would conflict, and an unpatched element is
 * still fully part of the show until it is given an address.
 */
import type { PatchFixtureProjection, PatchSplitAssignment } from "@tosklight/patch";
import { documentSession } from "../document/session";
import { TauriPatchTransport } from "../document/transport";

const transport = new TauriPatchTransport();

/** How far a copy stands from its original, in millimetres along the view's right. */
export const DUPLICATE_OFFSET_MILLIMETRES = 500;

type Location = PatchFixtureProjection["location"];

function unpatched(splits: readonly PatchSplitAssignment[]): PatchSplitAssignment[] {
	return splits.map((split) => ({ ...split, universe: null, address: null }));
}

function moved(location: Location, offset: readonly [number, number, number]): Location {
	return {
		x: Math.round(location.x + offset[0]),
		y: Math.round(location.y + offset[1]),
		z: Math.round(location.z + offset[2]),
	};
}

/** The first number above `from` that `taken` does not hold, which it then holds. */
function claimAbove(taken: Set<number>, from: number | null): number | null {
	if (from == null) return null;
	let next = from + 1;
	while (taken.has(next)) next++;
	taken.add(next);
	return next;
}

/**
 * Copies of the named fixtures, in the order named. `offset` is the step in show millimetres;
 * `newId` makes each new fixture and multi-patch ID.
 */
export function duplicateFixtures(
	all: readonly PatchFixtureProjection[],
	ids: readonly string[],
	offset: readonly [number, number, number],
	newId: () => string = () => crypto.randomUUID(),
): PatchFixtureProjection[] {
	const byId = new Map(all.map((fixture) => [fixture.fixtureId, fixture]));
	const numbers = new Set(all.flatMap((fixture) => fixture.fixtureNumber ?? []));
	const virtuals = new Set(all.flatMap((fixture) => fixture.virtualFixtureNumber ?? []));
	return ids.flatMap((id) => {
		const original = byId.get(id);
		if (!original) return [];
		return [
			{
				...original,
				fixtureId: newId(),
				fixtureRevision: 0,
				fixtureNumber: claimAbove(numbers, original.fixtureNumber),
				virtualFixtureNumber: claimAbove(virtuals, original.virtualFixtureNumber),
				splitPatches: unpatched(original.splitPatches),
				location: moved(original.location, offset),
				multipatch: original.multipatch.map((copy) => ({
					...copy,
					id: newId(),
					splitPatches: unpatched(copy.splitPatches),
					location: moved(copy.location, offset),
				})),
				logicalHeads: [],
			},
		];
	});
}

/**
 * Writes copies of the named elements to the show in one undoable step and returns their IDs, in
 * the order named.
 */
export async function duplicateSelection(
	ids: readonly string[],
	offset: readonly [number, number, number],
): Promise<string[]> {
	const snapshot = await documentSession.patchSnapshot();
	const copies = duplicateFixtures(snapshot.fixtures, ids, offset);
	if (!copies.length) return [];
	await transport.patchFixtures(snapshot.showId, snapshot.patchRevision, {
		requestId: crypto.randomUUID(),
		fixtures: copies,
		removeFixtureIds: [],
	});
	return copies.map((copy) => copy.fixtureId);
}
