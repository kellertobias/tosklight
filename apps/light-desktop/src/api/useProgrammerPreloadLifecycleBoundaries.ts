import { useMemo } from "react";
import type { ServerState } from "../features/server/useServerState";
import { configuredServerUrl } from "./client/serverLocation";
import { createFeatureErrorGroup } from "./featureErrorReporting";
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
						applyAction: (scope, request) =>
							state.api.programming.programmerPreloadLifecycleLiveAction(
								scope.userId,
								request,
							),
					})
				: null,
		[state.api, state.session],
	);
	return {
		programmerPreloadLifecycleTransport: transport,
		programmerPreloadLifecycleAuthorityKey: [
			configuredServerUrl(),
			state.bootstrap?.active_show?.id ?? "",
			state.session?.user.id ?? "",
			state.session?.desk.id ?? "",
		].join("|"),
		reportProgrammerPreloadLifecycleMutationError: errors.reportMutation,
	};
}
