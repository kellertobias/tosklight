import { useCallback, useMemo } from "react";
import type { StoredPreset } from "./types";
import type { ServerState } from "../features/server/useServerState";
import { createFeatureErrorGroup } from "./featureErrorReporting";
import { configuredServerUrl } from "./client/serverLocation";
import { browserDeskBoundaryToken } from "./PatchTransport";
import { HttpPresetRecordingTransport } from "./PresetRecordingTransport";

export function usePresetRecordingBoundaries(state: ServerState) {
	const errors = useMemo(
		() => createFeatureErrorGroup(state.setError),
		[state.setError],
	);
	const presetRecordingTransport = useMemo(
		() =>
			state.session
				? new HttpPresetRecordingTransport({
						baseUrl: configuredServerUrl(),
						sessionToken: state.session.token,
						deskBoundaryToken: browserDeskBoundaryToken(),
					})
				: null,
		[state.session],
	);
	const loadPresetForRepair = useCallback(
		(showId: string, objectId: string) =>
			state.api.showObjects.objectOrNull<StoredPreset>(showId, "preset", objectId),
		[state.api],
	);
	return {
		presetRecordingTransport,
		loadPresetForRepair,
		reportPresetRecordingError: errors.reportMutation,
	};
}
