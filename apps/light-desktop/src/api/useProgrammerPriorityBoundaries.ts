import { useMemo } from "react";
import type { ServerState } from "../features/server/useServerState";
import { configuredServerUrl } from "./client/serverLocation";
import { createFeatureErrorGroup } from "./featureErrorReporting";
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
						applyAction: (scope, request) =>
							state.api.programming.programmerPriorityLiveAction(
								scope.userId,
								request,
							),
					})
				: null,
		[state.api, state.session],
	);
	const authorityKey = [
		configuredServerUrl(),
		state.session?.user.id ?? "",
	].join("|");
	return {
		programmerPriorityTransport: transport,
		programmerPriorityAuthorityKey: authorityKey,
		reportProgrammerPrioritySessionError: errors.reportSession,
		reportProgrammerPriorityMutationError: errors.reportMutation,
	};
}
