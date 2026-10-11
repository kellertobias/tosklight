import type { ShowObject, ShowObjectKind } from "../showObjects/contracts";
import type { ShowObjectsStore } from "../showObjects/store";
import type { PlaybackTopologyAction } from "./contracts";

interface PlaybackTopologyRepairOptions {
	showId: string;
	store: ShowObjectsStore;
	loadObject<K extends ShowObjectKind>(
		showId: string,
		kind: K,
		objectId: string,
	): Promise<ShowObject<K> | null>;
}

export async function repairPlaybackTopologyConflict(
	options: PlaybackTopologyRepairOptions,
	error: Error,
	action: PlaybackTopologyAction,
	generation: number,
) {
	const failure = playbackTopologyTransportFailure(error);
	if (failure?.status !== 409) return;
	if (failure.currentRevision != null)
		options.store.installShowRevision(
			options.showId,
			failure.currentRevision,
			generation,
		);
	if (
		action.type === "save_cue_list" ||
		action.type === "undo_cue_list" ||
		action.type === "redo_cue_list"
	)
		return repairCueList(options, action, generation);
	if (action.type === "assign_group_master")
		await repairObject(options, "group", action.groupObjectId, generation);
	const page = "address" in action ? action.address.page : action.number;
	return repairObject(options, "playback_page", action.expectedPageObjectId ?? String(page), generation);
}

export function playbackTopologyTransportFailure(reason: unknown) {
	if (!(reason instanceof Error)) return null;
	const failure = reason as Error & {
		status?: unknown;
		retryable?: unknown;
		currentRevision?: unknown;
	};
	if (
		typeof failure.status !== "number" ||
		typeof failure.retryable !== "boolean"
	)
		return null;
	return {
		status: failure.status,
		retryable: failure.retryable,
		currentRevision:
			typeof failure.currentRevision === "number"
				? failure.currentRevision
				: null,
	};
}

async function repairCueList(
	options: PlaybackTopologyRepairOptions,
	action: Extract<
		PlaybackTopologyAction,
		{ type: "save_cue_list" | "undo_cue_list" | "redo_cue_list" }
	>,
	generation: number,
) {
	const stale = options.store
		.getSnapshot()
		.cueLists.find((object) => object.body.id === action.cueListId);
	await repairObject(
		options,
		"cue_list",
		stale?.id ?? action.cueListId,
		generation,
	);
}

async function repairObject<K extends ShowObjectKind>(
	options: PlaybackTopologyRepairOptions,
	kind: K,
	objectId: string,
	generation: number,
) {
	const stamp = options.store.captureObjectAuthority(
		options.showId,
		kind,
		objectId,
	);
	if (!stamp || stamp.authorityGeneration !== generation) return null;
	try {
		const object = await options.loadObject(options.showId, kind, objectId);
		return options.store.installObjectIfAuthorityUnchanged(stamp, object)
			? object
			: null;
	} catch {
		return null;
	}
}
