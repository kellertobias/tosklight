import { useCallback, useMemo } from "react";
import type { ServerState } from "../features/server/useServerState";
import { createFeatureErrorGroup } from "./featureErrorReporting";
import { configuredServerUrl } from "./LightApiClient";
import { browserDeskBoundaryToken } from "./PatchTransport";
import { HttpProgrammerCaptureModeTransport } from "./ProgrammerCaptureModeTransport";
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
	return {
		programmerValuesTransport,
		programmerCaptureModeTransport,
		programmerValuesAuthorityKey: authorityKey,
		programmerCaptureModeAuthorityKey: authorityKey,
		loadProgrammerValuesSnapshot,
		loadProgrammerCaptureModeSnapshot,
		applyProgrammerValuesAction,
		reportProgrammerValuesSessionError: valuesErrors.reportSession,
		reportProgrammerValuesMutationError: valuesErrors.reportMutation,
		reportProgrammerCaptureModeSessionError: captureModeErrors.reportSession,
	};
}
