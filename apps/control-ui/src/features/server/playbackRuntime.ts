import {
	cueListIdentity,
	playbackIdentity,
} from "../playbackRuntime/contracts";
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
			if (!session) return;
			const requestToken = playbackRuntimeStore.beginRequest(
				cueListIdentity(cueListId),
			);
			try {
				const outcome = await client.playbackRuntimeAction(
					session.desk.id,
					cueListPlaybackRequest(cueListId, action),
				);
				if (playbackRuntimeStore.installOutcome(outcome, requestToken))
					setError(null);
			} catch (reason) {
				const error = asError(reason);
				if (playbackRuntimeStore.rollbackProjection(requestToken, error))
					setError(error.message);
			}
		},
		poolPlaybackAction: async (number, action, input = {}) => {
			if (!session) return;
			const requestToken =
				action === "master" && input.value != null
					? playbackRuntimeStore.beginOptimisticMaster(number, input.value)
					: null;
			const trackedRequest =
				requestToken ??
				playbackRuntimeStore.beginRequest(playbackIdentity(number));
			try {
				const outcome = await client.playbackRuntimeAction(
					session.desk.id,
					poolPlaybackRequest(number, action, input),
				);
				if (playbackRuntimeStore.installOutcome(outcome, trackedRequest))
					setError(null);
			} catch (reason) {
				const error = asError(reason);
				if (playbackRuntimeStore.rollbackProjection(trackedRequest, error))
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
			const authority = playbackRuntimeStore.captureScope();
			const optimistic = playbackRuntimeStore.beginOptimisticPage(page);
			try {
				const outcome = await client.setPlaybackPage(deskId, page);
				if (!playbackRuntimeStore.isScopeCurrent(authority)) return;
				const accepted = optimistic
					? playbackRuntimeStore.commitPage(
							optimistic,
							page,
							outcome.event_sequence,
						)
					: true;
				if (accepted) setError(null);
			} catch (reason) {
				if (!playbackRuntimeStore.isScopeCurrent(authority)) return;
				const error =
					reason instanceof Error ? reason : new Error(String(reason));
				const accepted = optimistic
					? playbackRuntimeStore.rollbackPage(optimistic, error)
					: true;
				if (accepted) setError(error.message);
			}
		},
	};
}

function asError(reason: unknown) {
	return reason instanceof Error ? reason : new Error(String(reason));
}
