import { useCallback, useMemo } from "react";
import type { StoredGroup } from "./types";
import type { ServerState } from "../features/server/useServerState";
import { createFeatureErrorGroup } from "./featureErrorReporting";
import { configuredServerUrl } from "./client/serverLocation";
import { browserDeskBoundaryToken } from "./PatchTransport";
import { HttpGroupRecordingTransport } from "./GroupRecordingTransport";

export function useGroupRecordingBoundaries(state: ServerState) {
	const errors = useMemo(
		() => createFeatureErrorGroup(state.setError),
		[state.setError],
	);
	const groupRecordingTransport = useMemo(
		() =>
			state.session
				? new HttpGroupRecordingTransport({
						baseUrl: configuredServerUrl(),
						sessionToken: state.session.token,
						deskBoundaryToken: browserDeskBoundaryToken(),
					})
				: null,
		[state.session],
	);
	const loadGroupForRepair = useCallback(
		(showId: string, objectId: string) =>
			state.api.showObjects.objectOrNull<StoredGroup>(showId, "group", objectId),
		[state.api],
	);
	return {
		groupRecordingTransport,
		loadGroupForRepair,
		reportGroupRecordingError: errors.reportMutation,
	};
}
