import type {
	ShowObject,
	ShowObjectCollections,
	ShowObjectKind,
} from "./contracts";
import type { ShowObjectEventWatermarks } from "./eventWatermarks";
import { objectKey, sortObjects, upsertCollection } from "./storeProjection";
import type {
	PendingMutation,
	ShowObjectAuthorityStamp,
	ShowObjectSettlement,
} from "./storeTypes";

export function applyOptimisticCommit(
	authoritative: ShowObjectCollections,
	watermarks: ShowObjectEventWatermarks,
	operation: PendingMutation,
	objectRevision: number,
	minimumEventSequence?: number | null,
) {
	const key = objectKey(operation.kind, operation.objectId);
	watermarks.raiseObjectFloor(
		operation.kind,
		operation.objectId,
		minimumEventSequence,
	);
	const collection = authoritative[operation.kind] as ShowObject[];
	const existing = collection.find(
		(candidate) => candidate.id === operation.objectId,
	);
	const sequence = watermarks.appliedSequence(operation.kind, key);
	const eventObserved =
		minimumEventSequence != null
			? sequence >= minimumEventSequence
			: sequence > operation.baseEventSequence;
	if ((existing && existing.revision >= objectRevision) || eventObserved)
		return;
	upsertCollection(collection, {
		kind: operation.kind,
		id: operation.objectId,
		revision: objectRevision,
		updated_at: existing?.updated_at ?? "",
		body: operation.body,
	} as ShowObject);
	sortObjects(collection);
}

export function applyPendingSettlement<K extends ShowObjectKind>(
	authoritative: ShowObjectCollections,
	operation: PendingMutation,
	settlement: ShowObjectSettlement<K>,
) {
	assertPendingSettlementIdentity(operation, settlement);
	const collection = authoritative[operation.kind] as ShowObject[];
	const existing = collection.find(
		(candidate) => candidate.id === settlement.objectId,
	);
	if (existing && existing.revision > settlement.revision) return false;
	if (settlement.object) {
		upsertCollection(collection, settlement.object as ShowObject);
		sortObjects(collection);
		return true;
	}
	if (!existing) return false;
	authoritative[operation.kind] = collection.filter(
		(candidate) => candidate.id !== settlement.objectId,
	) as never;
	return true;
}

export function captureObjectAuthority<K extends ShowObjectKind>(
	authoritative: ShowObjectCollections,
	watermarks: ShowObjectEventWatermarks,
	showId: string,
	authorityGeneration: number,
	kind: K,
	objectId: string,
): ShowObjectAuthorityStamp<K> {
	return {
		showId,
		authorityGeneration,
		kind,
		objectId,
		eventSequence: watermarks.appliedSequence(kind, objectKey(kind, objectId)),
		object:
			(authoritative[kind].find((candidate) => candidate.id === objectId) as
				| ShowObject<K>
				| undefined) ?? null,
	};
}

export function matchesObjectAuthority<K extends ShowObjectKind>(
	authoritative: ShowObjectCollections,
	watermarks: ShowObjectEventWatermarks,
	showId: string | null,
	authorityGeneration: number,
	stamp: ShowObjectAuthorityStamp<K>,
) {
	if (
		showId !== stamp.showId ||
		authorityGeneration !== stamp.authorityGeneration
	)
		return false;
	const current = authoritative[stamp.kind].find(
		(candidate) => candidate.id === stamp.objectId,
	);
	return (
		(current ?? null) === stamp.object &&
		watermarks.appliedSequence(
			stamp.kind,
			objectKey(stamp.kind, stamp.objectId),
		) === stamp.eventSequence
	);
}

export function assertPendingSettlementIdentity<K extends ShowObjectKind>(
	operation: PendingMutation,
	settlement: ShowObjectSettlement<K>,
) {
	if (settlement.object && settlement.object.kind !== operation.kind)
		throw new Error(
			"Show Object settlement kind does not match pending mutation",
		);
	if (settlement.object && settlement.object.id !== settlement.objectId)
		throw new Error("Show Object settlement ID is internally inconsistent");
	if (settlement.object && settlement.object.revision !== settlement.revision)
		throw new Error(
			"Show Object settlement revision is internally inconsistent",
		);
	if (operation.kind === "group" && settlement.objectId !== operation.objectId)
		throw new Error("Group settlement ID does not match pending mutation");
}
