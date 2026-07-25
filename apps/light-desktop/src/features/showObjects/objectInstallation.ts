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
			object &&
			(!responseEventObserved
				? !existing || existing.revision <= object.revision
				: // A response carrying a strictly newer revision than the stored object is
					// authoritative even when the watermark already covers its event sequence:
					// a collection re-hydration can stamp the kind floor at the current show
					// event sequence while its payload predates a just-committed write (the
					// 409-retried topology save), and dropping that install pins the stale
					// revision. Only same-or-older bodies and resurrections stay guarded.
					existing !== undefined && existing.revision < object.revision)
		) {
			if (existing?.revision === object.revision) continue;
			upsertCollection(collection, object);
			sortObjects(collection);
			projectKinds.add(kind);
		}
	}
	return projectKinds;
}
