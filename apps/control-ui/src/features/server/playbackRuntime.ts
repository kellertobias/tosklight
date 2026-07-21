import {
	cueListIdentity,
} from "../playbackRuntime/contracts";
import type { ServerController } from "./model";
import {
	cueListPlaybackRequest,
} from "./playbackActionMapping";
import type { ServerContextValue } from "./ServerContextValue";

export function createPlaybackRuntimeActions(
	model: ServerController,
): Pick<
	ServerContextValue,
	| "playbackAction"
> {
	const { bootstrap, client, setError, session, playbackRuntimeStore } = model;
	return {
		playbackAction: async (cueListId, action) => {
			if (!session || !bootstrap?.active_show) return;
			const requestToken = playbackRuntimeStore.beginRequest(
				cueListIdentity(cueListId),
			);
			try {
				const outcome = await client.playbackRuntimeAction(
					bootstrap.active_show.id,
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
	};
}

function asError(reason: unknown) {
	return reason instanceof Error ? reason : new Error(String(reason));
}
