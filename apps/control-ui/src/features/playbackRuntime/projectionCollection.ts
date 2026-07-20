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
	const key = projection.playback_number ?? null;
	const next = current.filter((candidate) => candidate.playback_number !== key);
	next.push(projection);
	return next.sort(
		(left, right) =>
			(left.playback_number ?? 0) - (right.playback_number ?? 0),
	);
}
