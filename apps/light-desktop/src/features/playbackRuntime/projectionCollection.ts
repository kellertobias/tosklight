import type {
	PlaybackIdentity,
	PlaybackProjection,
} from "./contracts";
import { identityKey, projectionKeys } from "./contracts";

export function projectSnapshot(
	projections: readonly PlaybackProjection[],
	identities: readonly PlaybackIdentity[],
) {
	const result = new Map<string, PlaybackProjection[]>();
	for (const identity of identities) result.set(identityKey(identity), []);
	for (const projection of projections)
		for (const key of projectionKeys(projection))
			result.set(key, upsertProjection(result.get(key) ?? [], projection));
	return result;
}

export function upsertProjection(
	current: readonly PlaybackProjection[],
	projection: PlaybackProjection,
) {
	const key = identityKey(projection.identity);
	const next = current.filter((candidate) => identityKey(candidate.identity) !== key);
	next.push(projection);
	return next.sort((left, right) => identityKey(left.identity).localeCompare(identityKey(right.identity)));
}
