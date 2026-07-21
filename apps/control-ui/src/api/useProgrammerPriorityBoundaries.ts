import { useMemo } from "react";
import type { ServerState } from "../features/server/useServerState";
import { createFeatureErrorGroup } from "./featureErrorReporting";
import { configuredServerUrl } from "./LightApiClient";
import { browserDeskBoundaryToken } from "./PatchTransport";
import { HttpProgrammerPriorityTransport } from "./ProgrammerPriorityTransport";

export function useProgrammerPriorityBoundaries(state: ServerState) {
	const errors = useMemo(
		() => createFeatureErrorGroup(state.setError),
		[state.setError],
	);
	const transport = useMemo(
		() =>
			state.session
				? new HttpProgrammerPriorityTransport({
						baseUrl: configuredServerUrl(),
						sessionToken: state.session.token,
						authenticatedUserId: state.session.user.id,
						deskBoundaryToken: browserDeskBoundaryToken(),
					})
				: null,
		[state.session],
	);
	const authorityKey = [
		configuredServerUrl(),
		state.connectionGeneration,
		state.session?.session_id ?? "",
		state.session?.client_id ?? "",
		state.session?.user.id ?? "",
	].join("|");
	return {
		programmerPriorityTransport: transport,
		programmerPriorityAuthorityKey: authorityKey,
		reportProgrammerPrioritySessionError: errors.reportSession,
		reportProgrammerPriorityMutationError: errors.reportMutation,
	};
}
