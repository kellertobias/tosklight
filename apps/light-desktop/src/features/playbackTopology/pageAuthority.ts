import type { ShowObjectsStore } from "../showObjects/store";
import type {
	ExistingPlaybackPageRevisionBasis,
	PlaybackPageRevisionBasis,
} from "./contracts";

export function readyPageRevisions(
	store: ShowObjectsStore,
	page: number,
	revisionBasis?: PlaybackPageRevisionBasis,
) {
	const current = pageRevisions(store, page);
	return current ? (revisionBasis ?? current) : null;
}

export function existingPageRevisions(
	store: ShowObjectsStore,
	page: number,
	revisionBasis?: ExistingPlaybackPageRevisionBasis,
): ExistingPlaybackPageRevisionBasis | null {
	const current = pageRevisions(store, page);
	if (!current) return null;
	if (revisionBasis) return revisionBasis;
	return current?.expectedPageObjectId
		? {
				expectedPageRevision: current.expectedPageRevision,
				expectedPageObjectId: current.expectedPageObjectId,
			}
		: null;
}

function pageRevisions(
	store: ShowObjectsStore,
	page: number,
): PlaybackPageRevisionBasis | null {
	const snapshot = store.getSnapshot();
	if (!snapshot.readyCollections.has("playback_page")) return null;
	const object = snapshot.playbackPages.find(
		(candidate) => candidate.body.number === page,
	);
	return {
		expectedPageRevision: object?.revision ?? 0,
		expectedPageObjectId: object?.id ?? null,
	};
}
