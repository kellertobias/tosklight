import { useCallback, useMemo } from "react";
import type { ServerState } from "../features/server/useServerState";
import { createFeatureErrorGroup } from "./featureErrorReporting";
import { configuredServerUrl } from "./LightApiClient";
import { browserDeskBoundaryToken } from "./PatchTransport";
import { HttpProgrammerCaptureModeTransport } from "./ProgrammerCaptureModeTransport";
import { HttpProgrammerPreloadPlaybackQueueTransport } from "./ProgrammerPreloadPlaybackQueueTransport";
import { HttpProgrammerPreloadValuesTransport } from "./ProgrammerPreloadValuesTransport";
import { HttpProgrammerValuesTransport } from "./ProgrammerValuesTransport";

export function useProgrammerValuesBoundaries(state: ServerState) {
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
			state.session
				? new HttpProgrammerValuesTransport({
						baseUrl: configuredServerUrl(),
						sessionToken: state.session.token,
						deskBoundaryToken: browserDeskBoundaryToken(),
					})
				: null,
		[state.session],
	);
	const programmerCaptureModeTransport = useMemo(
		() =>
			state.session
				? new HttpProgrammerCaptureModeTransport({
						baseUrl: configuredServerUrl(),
						sessionToken: state.session.token,
						deskBoundaryToken: browserDeskBoundaryToken(),
					})
				: null,
		[state.session],
	);
	const programmerPreloadValuesTransport = useMemo(
		() =>
			state.session
				? new HttpProgrammerPreloadValuesTransport({
						baseUrl: configuredServerUrl(),
						sessionToken: state.session.token,
						deskBoundaryToken: browserDeskBoundaryToken(),
						authenticatedUserId: state.session.user.id,
					})
				: null,
		[state.session],
	);
	const programmerPreloadPlaybackQueueTransport = useMemo(
		() =>
			state.session
				? new HttpProgrammerPreloadPlaybackQueueTransport({
						baseUrl: configuredServerUrl(),
						sessionToken: state.session.token,
						deskBoundaryToken: browserDeskBoundaryToken(),
						authenticatedUserId: state.session.user.id,
					})
				: null,
		[state.session],
	);
	const programmerScope = useMemo(() => {
		const showId = state.bootstrap?.active_show?.id;
		const userId = state.session?.user.id;
		return showId && userId ? { showId, userId } : null;
	}, [state.bootstrap?.active_show?.id, state.session?.user.id]);
	const authorityKey = programmerScope
		? `${configuredServerUrl()}|${state.connectionGeneration}|${state.session?.session_id ?? ""}|${state.session?.client_id ?? ""}`
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
			if (!programmerValuesTransport)
				throw new Error("Programmer values session is unavailable");
			return programmerValuesTransport.applyAction(scope, request);
		},
		[programmerValuesTransport],
	);
	const applyProgrammerPreloadValuesAction = useCallback(
		(
			scope: NonNullable<typeof programmerScope>,
			request: Parameters<
				HttpProgrammerPreloadValuesTransport["applyAction"]
			>[1],
		) => {
			if (!programmerPreloadValuesTransport)
				throw new Error("Programmer Preload values session is unavailable");
			return programmerPreloadValuesTransport.applyAction(scope, request);
		},
		[programmerPreloadValuesTransport],
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
