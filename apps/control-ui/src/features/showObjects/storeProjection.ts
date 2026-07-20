import type {
	ShowObject,
	ShowObjectCollections,
	ShowObjectKind,
} from "./contracts";
import { projectLiveGroupMembership } from "./groupProjection";

interface PendingProjection {
	kind: ShowObjectKind;
	objectId: string;
	body: ShowObject["body"] | null;
}

export function objectKey(kind: ShowObjectKind, objectId: string) {
	return `${kind}:${objectId}`;
}

export function sortObjects<T extends ShowObject>(objects: T[]): T[] {
	return objects.sort((left, right) =>
		left.id < right.id ? -1 : left.id > right.id ? 1 : 0,
	);
}

export function projectCollection<K extends ShowObjectKind>(
	kind: K,
	authoritative: ShowObjectCollections[K],
	pending: Iterable<PendingProjection[]>,
): ShowObjectCollections[K] {
	const projected = new Map(authoritative.map((object) => [object.id, object]));
	for (const operations of pending) {
		const latest = latestProjected(operations);
		if (!latest || latest.kind !== kind || latest.body == null) continue;
		const existing = projected.get(latest.objectId);
		projected.set(latest.objectId, {
			kind,
			id: latest.objectId,
			revision: existing?.revision ?? 0,
			updated_at: existing?.updated_at ?? "",
			body: latest.body,
		} as ShowObjectCollections[K][number]);
	}
	const objects = sortObjects([...projected.values()]);
	return (kind === "group"
		? projectLiveGroupMembership(objects as ShowObject<"group">[])
		: objects) as ShowObjectCollections[K];
}

function latestProjected(operations: readonly PendingProjection[]) {
	for (let index = operations.length - 1; index >= 0; index -= 1) {
		const operation = operations[index];
		if (operation?.body != null) return operation;
	}
	return undefined;
}

export function upsertCollection<T extends ShowObject>(objects: T[], object: T) {
	const index = objects.findIndex((candidate) => candidate.id === object.id);
	if (index < 0) objects.push(object);
	else objects[index] = object;
}
