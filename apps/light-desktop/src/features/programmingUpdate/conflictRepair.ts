import type { ShowObject, ShowObjectKind } from "../showObjects/contracts";
import type { ShowObjectsStore } from "../showObjects/store";
import type { ProgrammingUpdateObjectIdentity } from "./contracts";
import { transportFailure } from "./writerSupport";

interface ResolvedProgrammingUpdateObject {
	object: ProgrammingUpdateObjectIdentity;
	showRevision: number;
}

interface ProgrammingUpdateConflictRepairOptions {
	error: Error;
	object: ProgrammingUpdateObjectIdentity | null;
	generation: number;
	showId: string;
	store: ShowObjectsStore;
	loadObject<K extends ShowObjectKind>(
		showId: string,
		kind: K,
		objectId: string,
	): Promise<ShowObject<K> | null>;
	resolveObject?: () => Promise<ResolvedProgrammingUpdateObject>;
}

/** Repairs only the exact conflicted portable object; never reloads bootstrap. */
export async function repairProgrammingUpdateConflict(
	options: ProgrammingUpdateConflictRepairOptions,
) {
	const failure = transportFailure(options.error);
	if (failure?.status !== 409) return;
	if (failure.currentShowRevision != null)
		options.store.installShowRevision(
			options.showId,
			failure.currentShowRevision,
			options.generation,
		);
	let object = options.object;
	if (!object && options.resolveObject) {
		try {
			const resolved = await options.resolveObject();
			object = resolved.object;
			options.store.installShowRevision(
				options.showId,
				resolved.showRevision,
				options.generation,
			);
		} catch {
			return;
		}
	}
	if (!object) return;
	await repairObject(options, object);
}

async function repairObject(
	options: ProgrammingUpdateConflictRepairOptions,
	object: ProgrammingUpdateObjectIdentity,
) {
	const stamp = options.store.captureObjectAuthority(
		options.showId,
		object.kind,
		object.object_id,
	);
	if (!stamp || stamp.authorityGeneration !== options.generation) return;
	try {
		const loaded = await options.loadObject(
			options.showId,
			object.kind,
			object.object_id,
		);
		options.store.installObjectIfAuthorityUnchanged(stamp, loaded);
	} catch {
		// The original revision conflict remains the actionable failure.
	}
}
