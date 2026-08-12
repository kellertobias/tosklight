import { useCallback, useMemo } from "react";
import type { ServerState } from "../features/server/useServerState";
import { configuredServerUrl } from "./client/serverLocation";
import { createFeatureErrorGroup } from "./featureErrorReporting";
import { browserDeskBoundaryToken } from "./PatchTransport";
import { HttpProgrammerLifecycleTransport } from "./ProgrammerLifecycleTransport";

export function useProgrammerLifecycleBoundaries(state: ServerState) {
	const errors = useMemo(
		() => createFeatureErrorGroup(state.setError),
		[state.setError],
	);
	const transport = useMemo(
		() =>
			state.session
				? new HttpProgrammerLifecycleTransport({
						baseUrl: configuredServerUrl(),
						sessionToken: state.session.token,
						deskBoundaryToken: browserDeskBoundaryToken(),
					})
				: null,
		[state.session],
	);
	const authorityKey = state.session
		? `${configuredServerUrl()}|${state.connectionGeneration}`
		: null;
	const loadSnapshot = useCallback(() => {
		if (!transport)
			throw new Error("Programmer lifecycle session is unavailable");
		return transport.loadSnapshot();
	}, [transport]);
	return {
		programmerLifecycleTransport: transport,
		programmerLifecycleAuthorityKey: authorityKey,
		loadProgrammerLifecycleSnapshot: loadSnapshot,
		reportProgrammerLifecycleSessionError: errors.reportSession,
	};
}
