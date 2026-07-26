import { useMemo } from "react";
import type { ServerState } from "../features/server/useServerState";
import { configuredServerUrl } from "./client/serverLocation";
import { createFeatureErrorGroup } from "./featureErrorReporting";
import { browserDeskBoundaryToken } from "./PatchTransport";
import { HttpSpeedGroupRuntimeTransport } from "./SpeedGroupRuntimeTransport";

/** Constructs the desk-authenticated adapter without activating its I/O. */
export function useSpeedGroupRuntimeBoundaries(state: ServerState) {
	const errors = useMemo(
		() => createFeatureErrorGroup(state.setError),
		[state.setError],
	);
	const transport = useMemo(
		() =>
			state.session
				? new HttpSpeedGroupRuntimeTransport({
						baseUrl: configuredServerUrl(),
						sessionToken: state.session.token,
						authenticatedDeskId: state.session.desk.id,
						deskBoundaryToken: browserDeskBoundaryToken(),
						applyAction: (_scope, request) =>
							state.api.desk.speedGroupRuntimeLiveAction(request),
					})
				: null,
		[state.api, state.session],
	);
	return {
		speedGroupRuntimeTransport: transport,
		speedGroupRuntimeAuthorityKey: [
			configuredServerUrl(),
			state.session?.desk.id ?? "",
		].join("|"),
		reportSpeedGroupSessionError: errors.reportSession,
		reportSpeedGroupMutationError: errors.reportMutation,
	};
}
