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
	sequenceMode: "floor" | "seal" = "floor",
) {
	const projectKinds = new Set<ShowObjectKind>();
	for (const { kind, objectId, object } of installs) {
		const responseEventObserved = watermarks.hasAppliedAtOrAfter(
			kind,
			objectKey(kind, objectId),
			minimumEventSequence,
		);
		if (sequenceMode === "seal")
			watermarks.sealExactResponse(kind, objectId, minimumEventSequence);
		else watermarks.raiseObjectFloor(kind, objectId, minimumEventSequence);
		const collection = authoritative[kind] as ShowObject[];
		const existing = collection.find((candidate) => candidate.id === objectId);
		if (!responseEventObserved && !object) {
			if (!existing) continue;
			authoritative[kind] = collection.filter(
				(candidate) => candidate.id !== objectId,
			) as never;
			projectKinds.add(kind);
		} else if (
			!responseEventObserved &&
			object &&
			(!existing || existing.revision <= object.revision)
		) {
			if (existing?.revision === object.revision) continue;
			upsertCollection(collection, object);
			sortObjects(collection);
			projectKinds.add(kind);
		}
	}
	return projectKinds;
}
