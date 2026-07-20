import { useCallback, useMemo } from "react";
import type { StoredGroup } from "./types";
import type { ServerState } from "../features/server/useServerState";
import { createFeatureErrorGroup } from "./featureErrorReporting";
import { configuredServerUrl } from "./LightApiClient";
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
			state.client.objectOrNull<StoredGroup>(showId, "group", objectId),
		[state.client],
	);
	return {
		groupRecordingTransport,
		loadGroupForRepair,
		reportGroupRecordingError: errors.reportMutation,
	};
}
