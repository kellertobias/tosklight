import { useCallback, useMemo, useRef } from "react";
import type { ServerState } from "../features/server/useServerState";
import type { ShowObjectKind } from "../features/showObjects/contracts";
import { createFeatureErrorGroup } from "./featureErrorReporting";
import { configuredServerUrl } from "./LightApiClient";
import { browserDeskBoundaryToken } from "./PatchTransport";
import { WebSocketPlaybackEventTransport } from "./PlaybackEventTransport";
import { WebSocketProgrammingEventTransport } from "./ProgrammingEventTransport";
import { HttpShowObjectSnapshotTransport } from "./ShowObjectSnapshotTransport";
import { WebSocketShowObjectsEventTransport } from "./ShowObjectsEventTransport";
import type { PlaybackRuntimeIdentity } from "./types";
import { useCueRecordingBoundaries } from "./useCueRecordingBoundaries";
import { useCueTransferBoundaries } from "./useCueTransferBoundaries";
import { useGroupRecordingBoundaries } from "./useGroupRecordingBoundaries";
import { usePlaybackTopologyBoundaries } from "./usePlaybackTopologyBoundaries";
import { usePresetRecallBoundaries } from "./usePresetRecallBoundaries";
import { usePresetRecordingBoundaries } from "./usePresetRecordingBoundaries";
import { useProgrammerLifecycleBoundaries } from "./useProgrammerLifecycleBoundaries";
import { useProgrammerValuesBoundaries } from "./useProgrammerValuesBoundaries";
import { useProgrammingUpdateBoundaries } from "./useProgrammingUpdateBoundaries";

export function useServerFeatureBoundaries(state: ServerState) {
	const programmingErrors = useMemo(
		() => createFeatureErrorGroup(state.setError),
		[state.setError],
	);
	const programmerValues = useProgrammerValuesBoundaries(state);
	const programmerLifecycle = useProgrammerLifecycleBoundaries(state);
	const presetRecording = usePresetRecordingBoundaries(state);
	const presetRecall = usePresetRecallBoundaries(state);
	const groupRecording = useGroupRecordingBoundaries(state);
	const cueRecording = useCueRecordingBoundaries(state);
	const cueTransfer = useCueTransferBoundaries(state);
	const playbackTopology = usePlaybackTopologyBoundaries(state);
	const programmingUpdate = useProgrammingUpdateBoundaries(state);
	const showObjectsAuthorityKey = [
		configuredServerUrl(),
		state.connectionGeneration,
		state.session?.session_id ?? "",
		state.session?.client_id ?? "",
		state.session?.user.id ?? "",
	].join("|");
	const programmingAuthorityKey = showObjectsAuthorityKey;
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
	const showObjectSnapshotTransport = useMemo(
		() =>
			state.session
				? new HttpShowObjectSnapshotTransport({
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
		(showId: string, kind: ShowObjectKind) => {
			if (!showObjectSnapshotTransport)
				throw new Error("Show-object session is unavailable");
			return showObjectSnapshotTransport.collection(showId, kind);
		},
		[showObjectSnapshotTransport],
	);
	const loadShowObjectSnapshot = useCallback(
		<K extends ShowObjectKind>(showId: string, kind: K, objectId: string) => {
			if (!showObjectSnapshotTransport)
				throw new Error("Show-object session is unavailable");
			return showObjectSnapshotTransport.object(showId, kind, objectId);
		},
		[showObjectSnapshotTransport],
	);
	const loadShowObject = useCallback(
		async <K extends ShowObjectKind>(
			showId: string,
			kind: K,
			objectId: string,
		) => (await loadShowObjectSnapshot(showId, kind, objectId)).object,
		[loadShowObjectSnapshot],
	);
	const reportShowObjectError = useFeatureErrorReporter(state.setError);
	const reportPlaybackError = useFeatureErrorReporter(state.setError);
	const reportPlaybackTopologyError = useFeatureErrorReporter(state.setError);
	return {
		showObjectsTransport,
		showObjectsAuthorityKey,
		playbackTransport,
		playbackAuthorityKey: showObjectsAuthorityKey,
		programmingTransport,
		programmingAuthorityKey,
		...programmerLifecycle,
		...programmerValues,
		...presetRecording,
		...presetRecall,
		...groupRecording,
		...cueRecording,
		...cueTransfer,
		...playbackTopology,
		...programmingUpdate,
		loadPlaybackSnapshot,
		loadProgrammingInteractionSnapshot,
		loadShowObjectCollection,
		loadShowObjectSnapshot,
		loadShowObject,
		reportShowObjectError,
		reportPlaybackError,
		reportPlaybackTopologyError,
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
