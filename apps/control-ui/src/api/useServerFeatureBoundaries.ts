import { useCallback, useMemo, useRef } from "react";
import type { ServerState } from "../features/server/useServerState";
import type { ShowObject } from "../features/showObjects/contracts";
import { configuredServerUrl } from "./LightApiClient";
import { browserDeskBoundaryToken } from "./PatchTransport";
import { WebSocketPlaybackEventTransport } from "./PlaybackEventTransport";
import { WebSocketProgrammingEventTransport } from "./ProgrammingEventTransport";
import { HttpProgrammerValuesTransport } from "./ProgrammerValuesTransport";
import { WebSocketShowObjectsEventTransport } from "./ShowObjectsEventTransport";
import { createFeatureErrorGroup } from "./featureErrorReporting";
import type { PlaybackRuntimeIdentity } from "./types";

export function useServerFeatureBoundaries(state: ServerState) {
	const programmingErrors = useMemo(
		() => createFeatureErrorGroup(state.setError),
		[state.setError],
	);
	const programmerValuesErrors = useMemo(
		() => createFeatureErrorGroup(state.setError),
		[state.setError],
	);
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
	const programmerValuesTransport = useMemo(
		() =>
			state.session
				? new HttpProgrammerValuesTransport({
						baseUrl: configuredServerUrl(),
						sessionToken: state.session.token,
						deskBoundaryToken: browserDeskBoundaryToken(),
					})
				: null,
		[state.session],
	);
	const programmerValuesScope = useMemo(() => {
		const showId = state.bootstrap?.active_show?.id;
		const userId = state.session?.user.id;
		return showId && userId ? { showId, userId } : null;
	}, [state.bootstrap?.active_show?.id, state.session?.user.id]);
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
	const loadProgrammerValuesSnapshot = useCallback(() => {
		if (!programmerValuesTransport || !programmerValuesScope)
			throw new Error("Programmer values session is unavailable");
		return programmerValuesTransport.loadSnapshot(programmerValuesScope);
	}, [programmerValuesScope, programmerValuesTransport]);
	const applyProgrammerValuesAction = useCallback(
		(scope: NonNullable<typeof programmerValuesScope>, request: Parameters<HttpProgrammerValuesTransport["applyAction"]>[1]) => {
			if (!programmerValuesTransport)
				throw new Error("Programmer values session is unavailable");
			return programmerValuesTransport.applyAction(scope, request);
		},
		[programmerValuesTransport],
	);
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
		playbackTransport,
		programmingTransport,
		programmerValuesTransport,
		programmerValuesAuthorityKey: programmerValuesScope
			? `${configuredServerUrl()}|${state.connectionGeneration}|${state.session?.session_id ?? ""}|${state.session?.client_id ?? ""}`
			: "",
		loadPlaybackSnapshot,
		loadProgrammingInteractionSnapshot,
		loadProgrammerValuesSnapshot,
		applyProgrammerValuesAction,
		loadShowObjectCollection,
		loadShowObject,
		reportShowObjectError: useFeatureErrorReporter(state.setError),
		reportPlaybackError: useFeatureErrorReporter(state.setError),
		reportProgrammingSessionError: programmingErrors.reportSession,
		reportProgrammingMutationError: programmingErrors.reportMutation,
		reportProgrammerValuesSessionError: programmerValuesErrors.reportSession,
		reportProgrammerValuesMutationError: programmerValuesErrors.reportMutation,
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
