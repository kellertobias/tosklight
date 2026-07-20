import type {
	ShowObject,
	ShowObjectCollections,
	ShowObjectKind,
} from "./contracts";
import { ShowObjectEventWatermarks } from "./eventWatermarks";
import { objectKey, sortObjects, upsertCollection } from "./storeProjection";
import type { ShowObjectInstall } from "./storeTypes";

export function installAuthoritativeObjects(
	authoritative: ShowObjectCollections,
	watermarks: ShowObjectEventWatermarks,
	installs: readonly ShowObjectInstall[],
	minimumEventSequence?: number | null,
) {
	const projectKinds = new Set<ShowObjectKind>();
	for (const { kind, objectId, object } of installs) {
		projectKinds.add(kind);
		const responseEventObserved = watermarks.hasAppliedAtOrAfter(
			kind,
			objectKey(kind, objectId),
			minimumEventSequence,
		);
		watermarks.raiseObjectFloor(kind, objectId, minimumEventSequence);
		const collection = authoritative[kind] as ShowObject[];
		const existing = collection.find((candidate) => candidate.id === objectId);
		if (!responseEventObserved && !object) {
			authoritative[kind] = collection.filter(
				(candidate) => candidate.id !== objectId,
			) as never;
		} else if (
			!responseEventObserved &&
			object &&
			(minimumEventSequence != null ||
				!existing ||
				existing.revision <= object.revision)
		) {
			upsertCollection(collection, object);
			sortObjects(collection);
		}
	}
	return projectKinds;
}
