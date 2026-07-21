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
	if (action.type === "save_cue_list")
		return repairCueList(options, action, generation);
	if (action.type === "create_page" || action.type === "rename_page")
		return repairPage(options, action, generation);
	if (action.type === "map_existing_playback")
		return repairExistingPlaybackMap(options, action, generation);
	await repairMappedSlot(options, action.page, action.slot, generation);
}

async function repairPage(
	options: PlaybackTopologyRepairOptions,
	action: Extract<
		PlaybackTopologyAction,
		{ type: "create_page" | "rename_page" }
	>,
	generation: number,
) {
	await repairObject(
		options,
		"playback_page",
		action.expectedPageObjectId ?? String(action.page),
		generation,
	);
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
	action: Extract<PlaybackTopologyAction, { type: "save_cue_list" }>,
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

async function repairExistingPlaybackMap(
	options: PlaybackTopologyRepairOptions,
	action: Extract<PlaybackTopologyAction, { type: "map_existing_playback" }>,
	generation: number,
) {
	await repairObject(
		options,
		"playback_page",
		action.expectedPageObjectId ?? String(action.page),
		generation,
	);
	await repairObject(
		options,
		"playback",
		action.expectedPlaybackObjectId,
		generation,
	);
}

async function repairMappedSlot(
	options: PlaybackTopologyRepairOptions,
	pageNumber: number,
	slot: number,
	generation: number,
) {
	const before = options.store.getSnapshot();
	const stalePage = before.playbackPages.find(
		(object) => object.body.number === pageNumber,
	);
	const staleNumber = stalePage?.body.slots[String(slot)];
	const stalePlayback = before.playbacks.find(
		(object) => object.body.number === staleNumber,
	);
	const page = await repairObject(
		options,
		"playback_page",
		stalePage?.id ?? String(pageNumber),
		generation,
	);
	if (stalePlayback)
		await repairObject(options, "playback", stalePlayback.id, generation);
	const playbackNumber = page?.body.slots[String(slot)];
	if (playbackNumber == null) return;
	const currentPlayback = options.store
		.getSnapshot()
		.playbacks.find((object) => object.body.number === playbackNumber);
	if (currentPlayback?.id !== stalePlayback?.id)
		await repairObject(
			options,
			"playback",
			currentPlayback?.id ?? String(playbackNumber),
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
