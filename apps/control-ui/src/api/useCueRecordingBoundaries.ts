import { useCallback, useMemo } from "react";
import { HttpCueRecordingTransport } from "./CueRecordingTransport";
import { createFeatureErrorGroup } from "./featureErrorReporting";
import { configuredServerUrl } from "./LightApiClient";
import { browserDeskBoundaryToken } from "./PatchTransport";
import type { ServerState } from "../features/server/useServerState";

export function useCueRecordingBoundaries(state: ServerState) {
	const errors = useMemo(
		() => createFeatureErrorGroup(state.setError),
		[state.setError],
	);
	const cueRecordingTransport = useMemo(
		() =>
			state.session
				? new HttpCueRecordingTransport({
						baseUrl: configuredServerUrl(),
						sessionToken: state.session.token,
						deskBoundaryToken: browserDeskBoundaryToken(),
					})
				: null,
		[state.session],
	);
	const selectedCueRecordingPlayback = useCallback(
		() =>
			state.playbackRuntimeStore.getSnapshot().desk?.selected_playback ?? null,
		[state.playbackRuntimeStore],
	);
	return {
		cueRecordingTransport,
		selectedCueRecordingPlayback,
		reportCueRecordingError: errors.reportMutation,
	};
}
