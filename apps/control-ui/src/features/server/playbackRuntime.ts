import type { ServerController } from "./model";
import {
	cueListPlaybackRequest,
	poolPlaybackRequest,
} from "./playbackActionMapping";
import type { ServerContextValue } from "./ServerContextValue";

export function createPlaybackRuntimeActions(
	model: ServerController,
): Pick<
	ServerContextValue,
	| "playbackAction"
	| "poolPlaybackAction"
	| "readVirtualPlaybackExclusionZones"
	| "saveVirtualPlaybackExclusionZones"
	| "setPlaybackPage"
> {
	const { client, setError, playbacks, session, playbackRuntimeStore } = model;
	return {
		playbackAction: async (cueListId, action) => {
			try {
				if (!session) return;
				const outcome = await client.playbackRuntimeAction(
					session.desk.id,
					cueListPlaybackRequest(cueListId, action),
				);
				playbackRuntimeStore.installOutcome(outcome);
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		poolPlaybackAction: async (number, action, input = {}) => {
			if (!session) return;
			const optimistic =
				action === "master" && input.value != null
					? playbackRuntimeStore.beginOptimisticMaster(number, input.value)
					: null;
			try {
				const outcome = await client.playbackRuntimeAction(
					session.desk.id,
					poolPlaybackRequest(number, action, input),
				);
				playbackRuntimeStore.installOutcome(outcome, optimistic);
				setError(null);
			} catch (reason) {
				const error =
					reason instanceof Error ? reason : new Error(String(reason));
				playbackRuntimeStore.rollbackProjection(optimistic, error);
				setError(error.message);
			}
		},
		readVirtualPlaybackExclusionZones: () =>
			client.virtualPlaybackExclusionZones(),
		saveVirtualPlaybackExclusionZones: async (surfaceId, zones) => {
			try {
				await client.saveVirtualPlaybackExclusionZones(surfaceId, zones);
				setError(null);
				return true;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return false;
			}
		},
		setPlaybackPage: async (page) => {
			const deskId = session?.desk.id ?? playbacks?.desk.id;
			if (!deskId) return;
			const optimistic = playbackRuntimeStore.beginOptimisticPage(page);
			try {
				const outcome = await client.setPlaybackPage(deskId, page);
				playbackRuntimeStore.commitPage(
					optimistic,
					page,
					outcome.event_sequence,
				);
				setError(null);
			} catch (reason) {
				const error =
					reason instanceof Error ? reason : new Error(String(reason));
				playbackRuntimeStore.rollbackPage(optimistic, error);
				setError(error.message);
			}
		},
	};
}
