import { useCallback, useMemo, useRef } from "react";
import type { ServerState } from "../features/server/useServerState";
import type { ShowObject } from "../features/showObjects/contracts";
import { createFeatureErrorGroup } from "./featureErrorReporting";
import { configuredServerUrl } from "./LightApiClient";
import { browserDeskBoundaryToken } from "./PatchTransport";
import { WebSocketPlaybackEventTransport } from "./PlaybackEventTransport";
import { WebSocketProgrammingEventTransport } from "./ProgrammingEventTransport";
import { WebSocketShowObjectsEventTransport } from "./ShowObjectsEventTransport";
import type { PlaybackRuntimeIdentity } from "./types";
import { useProgrammerLifecycleBoundaries } from "./useProgrammerLifecycleBoundaries";
import { useProgrammerValuesBoundaries } from "./useProgrammerValuesBoundaries";
import { usePresetRecordingBoundaries } from "./usePresetRecordingBoundaries";
import { useGroupRecordingBoundaries } from "./useGroupRecordingBoundaries";

export function useServerFeatureBoundaries(state: ServerState) {
	const programmingErrors = useMemo(
		() => createFeatureErrorGroup(state.setError),
		[state.setError],
	);
	const programmerValues = useProgrammerValuesBoundaries(state);
	const programmerLifecycle = useProgrammerLifecycleBoundaries(state);
	const presetRecording = usePresetRecordingBoundaries(state);
	const groupRecording = useGroupRecordingBoundaries(state);
	const showObjectsAuthorityKey = [
		configuredServerUrl(),
		state.connectionGeneration,
		state.session?.session_id ?? "",
		state.session?.client_id ?? "",
	].join("|");
	const showObjectsTransport = useMemo(
		() =>
			state.session
				? new WebSocketShowObjectsEventTransport({
						baseUrl: configuredServerUrl(),
						sessionToken: state.session.token,
						deskBoundaryToken: browserDeskBoundaryToken(),
					})
				: null,
		[state.session],
	);
	const playbackTransport = useMemo(
		() =>
			state.session
				? new WebSocketPlaybackEventTransport({
						baseUrl: configuredServerUrl(),
						sessionToken: state.session.token,
						deskBoundaryToken: browserDeskBoundaryToken(),
					})
				: null,
		[state.session],
	);
	const programmingTransport = useMemo(
		() =>
			state.session
				? new WebSocketProgrammingEventTransport({
						baseUrl: configuredServerUrl(),
						sessionToken: state.session.token,
						deskBoundaryToken: browserDeskBoundaryToken(),
					})
				: null,
		[state.session],
	);
	const loadPlaybackSnapshot = useCallback(
		(identities: PlaybackRuntimeIdentity[]) => {
			if (!state.session) throw new Error("Playback session is unavailable");
			return state.client.playbackRuntimeSnapshot(
				state.session.desk.id,
				identities,
			);
		},
		[state.client, state.session],
	);
	const loadProgrammingInteractionSnapshot = useCallback(() => {
		if (!state.session)
			throw new Error("Programming interaction session is unavailable");
		return state.client.programmingInteractionSnapshot(state.session.desk.id);
	}, [state.client, state.session]);
	const loadShowObjectCollection = useCallback(
		(showId: string, kind: "group" | "preset") =>
			state.client.objects(showId, kind) as Promise<ShowObject[]>,
		[state.client],
	);
	const loadShowObject = useCallback(
		(showId: string, kind: "group" | "preset", objectId: string) =>
			state.client.objectOrNull(
				showId,
				kind,
				objectId,
			) as Promise<ShowObject | null>,
		[state.client],
	);
	return {
		showObjectsTransport,
		showObjectsAuthorityKey,
		playbackTransport,
		programmingTransport,
		...programmerLifecycle,
		...programmerValues,
		...presetRecording,
		...groupRecording,
		loadPlaybackSnapshot,
		loadProgrammingInteractionSnapshot,
		loadShowObjectCollection,
		loadShowObject,
		reportShowObjectError: useFeatureErrorReporter(state.setError),
		reportPlaybackError: useFeatureErrorReporter(state.setError),
		reportProgrammingSessionError: programmingErrors.reportSession,
		reportProgrammingMutationError: programmingErrors.reportMutation,
	};
}

function useFeatureErrorReporter(
	setError: React.Dispatch<React.SetStateAction<string | null>>,
) {
	const lastError = useRef<string | null>(null);
	return useCallback(
		(error: Error | null) => {
			if (error) {
				lastError.current = error.message;
				setError(error.message);
				return;
			}
			setError((current) => (current === lastError.current ? null : current));
			lastError.current = null;
		},
		[setError],
	);
}
