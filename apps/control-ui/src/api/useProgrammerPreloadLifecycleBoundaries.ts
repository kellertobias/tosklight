import { useMemo } from "react";
import type { ServerState } from "../features/server/useServerState";
import { createFeatureErrorGroup } from "./featureErrorReporting";
import { configuredServerUrl } from "./LightApiClient";
import { browserDeskBoundaryToken } from "./PatchTransport";
import { HttpProgrammerPreloadLifecycleTransport } from "./ProgrammerPreloadLifecycleTransport";

export function useProgrammerPreloadLifecycleBoundaries(state: ServerState) {
	const errors = useMemo(
		() => createFeatureErrorGroup(state.setError),
		[state.setError],
	);
	const transport = useMemo(
		() =>
			state.session
				? new HttpProgrammerPreloadLifecycleTransport({
						baseUrl: configuredServerUrl(),
						sessionToken: state.session.token,
						authenticatedUserId: state.session.user.id,
						authenticatedDeskId: state.session.desk.id,
						deskBoundaryToken: browserDeskBoundaryToken(),
					})
				: null,
		[state.session],
	);
	return {
		programmerPreloadLifecycleTransport: transport,
		programmerPreloadLifecycleAuthorityKey: [
			configuredServerUrl(),
			state.connectionGeneration,
			state.session?.session_id ?? "",
			state.session?.client_id ?? "",
			state.session?.user.id ?? "",
			state.session?.desk.id ?? "",
		].join("|"),
		reportProgrammerPreloadLifecycleMutationError: errors.reportMutation,
	};
}
