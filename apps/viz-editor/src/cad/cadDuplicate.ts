/**
 * Duplicating selected elements from the CAD object menu.
 *
 * A copy is a new element of the show with its own fixture ID, so it is selected, moved and deleted
 * on its own. It keeps everything the original is — profile, mode, size, rotation, name — and
 * stands beside it, edges touching, on the view it was duplicated in, so it is never hidden under
 * the original; `duplicateStep` measures that step. Its fixture and virtual numbers are the next
 * free ones above the original's,
 * and it is unpatched: two elements on one DMX address would conflict, and an unpatched element is
 * still fully part of the show until it is given an address.
 */
import type { PatchFixtureProjection, PatchSplitAssignment } from "@tosklight/patch";
import { documentSession } from "../document/session";
import { cadSession } from "./session";
import type { CadEntity, CadTransformPreview } from "./types";

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
 * Writes copies of the named elements to the show as one step Undo takes away again, and returns
 * their IDs in the order named. `sceneRevision` is the rig the copies were made from.
 */
export async function duplicateSelection(
	ids: readonly string[],
	offset: readonly [number, number, number],
	sceneRevision: number,
): Promise<string[]> {
	const snapshot = await documentSession.patchSnapshot();
	const copies = duplicateFixtures(snapshot.fixtures, ids, offset);
	if (!copies.length) return [];
	await cadSession.add(sceneRevision, copies);
	return copies.map((copy) => copy.fixtureId);
}

/**
 * What a duplicating move draws: every placement of the fixtures it copies stays where it is, and a
 * copy of each rides the move, so the operator sees the original stay and the copy go.
 */
export function withDuplicatePreview(
	entities: readonly CadEntity[],
	preview: CadTransformPreview | null,
): { entities: readonly CadEntity[]; preview: CadTransformPreview | null } {
	if (!preview?.duplicate) return { entities, preview };
	const copying = new Set(preview.entityIds);
	const [dx, dy, dz] = preview.deltaMillimetres;
	const copies = entities
		.filter((entity) => copying.has(entity.logicalFixtureId))
		.map((entity) => ({
			...entity,
			id: `${entity.id}:copy`,
			logicalFixtureId: `${entity.logicalFixtureId}:copy`,
			positionMillimetres: [
				entity.positionMillimetres[0] + dx,
				entity.positionMillimetres[1] + dy,
				entity.positionMillimetres[2] + dz,
			] as [number, number, number],
		}));
	// The gizmo still follows the move; the originals no longer do.
	return { entities: [...entities, ...copies], preview: { ...preview, entityIds: [] } };
}
