import type {
	ShowObject,
	ShowObjectCollections,
	ShowObjectKind,
	ShowObjectsChange,
} from "./contracts";
import { ShowObjectEventWatermarks } from "./eventWatermarks";
import { sortObjects, upsertCollection } from "./storeProjection";

export interface AppliedShowObjectsChange {
	accepted: boolean;
	projectKinds: Set<ShowObjectKind>;
}

export function applyAuthoritativeChange(
	authoritative: ShowObjectCollections,
	watermarks: ShowObjectEventWatermarks,
	change: ShowObjectsChange,
): AppliedShowObjectsChange {
	let accepted = false;
	const projectKinds = new Set<ShowObjectKind>();
	for (const objectChange of change.changes) {
		const collection = authoritative[objectChange.kind] as ShowObject[];
		const existing = collection.find(
			(candidate) => candidate.id === objectChange.objectId,
		);
		if (
			!watermarks.acceptChange(
				objectChange.kind,
				objectChange.objectId,
				change.eventSequence,
			)
		)
			continue;
		accepted = true;
		if (existing && existing.revision > objectChange.objectRevision) continue;
		projectKinds.add(objectChange.kind);
		if (objectChange.deleted) {
			authoritative[objectChange.kind] = collection.filter(
				(object) => object.id !== objectChange.objectId,
			) as never;
		} else if (objectChange.body) {
			upsertCollection(collection, {
				kind: objectChange.kind,
				id: objectChange.objectId,
				revision: objectChange.objectRevision,
				updated_at: existing?.updated_at ?? "",
				body: objectChange.body,
			} as ShowObject);
			sortObjects(collection);
		}
	}
	return { accepted, projectKinds };
}
