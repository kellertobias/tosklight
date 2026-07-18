import type { ServerController } from "./model";
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
	const { client, setError, playbacks, setPlaybacks } = model;
	return {
		playbackAction: async (cueListId, action) => {
			try {
				await client.playbackAction(cueListId, action);
				setPlaybacks(await client.playbacks());
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		poolPlaybackAction: async (number, action, input = {}) => {
			try {
				await client.poolPlaybackAction(number, action, input);
				setPlaybacks(await client.playbacks());
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
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
			if (!playbacks?.desk) return;
			try {
				await client.setPlaybackPage(playbacks.desk.id, page);
				setPlaybacks(await client.playbacks());
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
	};
}
