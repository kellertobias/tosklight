import { useCallback, useMemo } from "react";
import type { ServerState } from "../features/server/useServerState";
import { configuredServerUrl } from "./client/serverLocation";
import { createFeatureErrorGroup } from "./featureErrorReporting";
import { browserDeskBoundaryToken } from "./PatchTransport";
import { HttpProgrammerCaptureModeTransport } from "./ProgrammerCaptureModeTransport";
import { HttpProgrammerPreloadPlaybackQueueTransport } from "./ProgrammerPreloadPlaybackQueueTransport";
import { HttpProgrammerPreloadValuesTransport } from "./ProgrammerPreloadValuesTransport";
import { HttpProgrammerValuesTransport } from "./ProgrammerValuesTransport";

function useProgrammerScope(state: ServerState) {
	return useMemo(() => {
		const showId = state.bootstrap?.active_show?.id;
		const sessionId = state.session?.session_id;
		return showId && sessionId ? { showId, sessionId } : null;
	}, [state.bootstrap?.active_show?.id, state.session?.session_id]);
}

export function useProgrammerValuesBoundaries(state: ServerState) {
	const sessionToken = state.session?.token ?? null;
	const sessionUserId = state.session?.session_id ?? null;
	const valuesErrors = useMemo(
		() => createFeatureErrorGroup(state.setError),
		[state.setError],
	);
	const captureModeErrors = useMemo(
		() => createFeatureErrorGroup(state.setError),
		[state.setError],
	);
	const preloadValuesErrors = useMemo(
		() => createFeatureErrorGroup(state.setError),
		[state.setError],
	);
	const preloadPlaybackQueueErrors = useMemo(
		() => createFeatureErrorGroup(state.setError),
		[state.setError],
	);
	const programmerValuesTransport = useMemo(
		() =>
			sessionToken
				? new HttpProgrammerValuesTransport({
						baseUrl: configuredServerUrl(),
						sessionToken,
						deskBoundaryToken: browserDeskBoundaryToken(),
					})
				: null,
		[sessionToken],
	);
	const programmerCaptureModeTransport = useMemo(
		() =>
			sessionToken
				? new HttpProgrammerCaptureModeTransport({
						baseUrl: configuredServerUrl(),
						sessionToken,
						deskBoundaryToken: browserDeskBoundaryToken(),
					})
				: null,
		[sessionToken],
	);
	const programmerPreloadValuesTransport = useMemo(
		() =>
			sessionToken && sessionUserId
				? new HttpProgrammerPreloadValuesTransport({
						baseUrl: configuredServerUrl(),
						sessionToken,
						deskBoundaryToken: browserDeskBoundaryToken(),
						authenticatedSessionId: sessionUserId,
					})
				: null,
		[sessionToken, sessionUserId],
	);
	const programmerPreloadPlaybackQueueTransport = useMemo(
		() =>
			sessionToken && sessionUserId
				? new HttpProgrammerPreloadPlaybackQueueTransport({
						baseUrl: configuredServerUrl(),
						sessionToken,
						deskBoundaryToken: browserDeskBoundaryToken(),
						authenticatedSessionId: sessionUserId,
					})
				: null,
		[sessionToken, sessionUserId],
	);
	const programmerScope = useProgrammerScope(state);
	const authorityKey = programmerScope
		? `${configuredServerUrl()}|${programmerScope.showId}|${programmerScope.sessionId}`
		: "";
	const loadProgrammerValuesSnapshot = useCallback(() => {
		if (!programmerValuesTransport || !programmerScope)
			throw new Error("Programmer values session is unavailable");
		return programmerValuesTransport.loadSnapshot(programmerScope);
	}, [programmerScope, programmerValuesTransport]);
	const loadProgrammerCaptureModeSnapshot = useCallback(() => {
		if (!programmerCaptureModeTransport || !programmerScope)
			throw new Error("Programmer capture mode session is unavailable");
		return programmerCaptureModeTransport.loadSnapshot(programmerScope);
	}, [programmerCaptureModeTransport, programmerScope]);
	const loadProgrammerPreloadValuesSnapshot = useCallback(() => {
		if (!programmerPreloadValuesTransport || !programmerScope)
			throw new Error("Programmer Preload values session is unavailable");
		return programmerPreloadValuesTransport.loadSnapshot(programmerScope);
	}, [programmerPreloadValuesTransport, programmerScope]);
	const loadProgrammerPreloadPlaybackQueueSnapshot = useCallback(() => {
		if (!programmerPreloadPlaybackQueueTransport || !programmerScope)
			throw new Error("Programmer Preload playback queue is unavailable");
		return programmerPreloadPlaybackQueueTransport.loadSnapshot(
			programmerScope,
		);
	}, [programmerPreloadPlaybackQueueTransport, programmerScope]);
	const applyProgrammerValuesAction = useCallback(
		(
			scope: NonNullable<typeof programmerScope>,
			request: Parameters<HttpProgrammerValuesTransport["applyAction"]>[1],
		) => {
			if (!sessionToken)
				throw new Error("Programmer values session is unavailable");
			return state.api.programming.programmerValuesLiveAction(
				scope.sessionId,
				request,
			);
		},
		[sessionToken, state.api],
	);
	const applyProgrammerPreloadValuesAction = useCallback(
		(
			scope: NonNullable<typeof programmerScope>,
			request: Parameters<
				HttpProgrammerPreloadValuesTransport["applyAction"]
			>[1],
		) => {
			if (!sessionToken)
				throw new Error("Programmer Preload values session is unavailable");
			return state.api.programming.programmerPreloadValuesLiveAction(
				scope.sessionId,
				request,
			);
		},
		[sessionToken, state.api],
	);
	return {
		programmerValuesTransport,
		programmerPreloadValuesTransport,
		programmerPreloadPlaybackQueueTransport,
		programmerCaptureModeTransport,
		programmerValuesAuthorityKey: authorityKey,
		programmerPreloadValuesAuthorityKey: authorityKey,
		programmerPreloadPlaybackQueueAuthorityKey: authorityKey,
		programmerCaptureModeAuthorityKey: authorityKey,
		loadProgrammerValuesSnapshot,
		loadProgrammerPreloadValuesSnapshot,
		loadProgrammerPreloadPlaybackQueueSnapshot,
		loadProgrammerCaptureModeSnapshot,
		applyProgrammerValuesAction,
		applyProgrammerPreloadValuesAction,
		reportProgrammerValuesSessionError: valuesErrors.reportSession,
		reportProgrammerValuesMutationError: valuesErrors.reportMutation,
		reportProgrammerPreloadValuesSessionError:
			preloadValuesErrors.reportSession,
		reportProgrammerPreloadValuesMutationError:
			preloadValuesErrors.reportMutation,
		reportProgrammerPreloadPlaybackQueueSessionError:
			preloadPlaybackQueueErrors.reportSession,
		reportProgrammerCaptureModeSessionError: captureModeErrors.reportSession,
	};
}
