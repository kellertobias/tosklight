import { useMemo } from "react";
import type { ServerState } from "../features/server/useServerState";
import { createFeatureErrorGroup } from "./featureErrorReporting";
import { configuredServerUrl } from "./LightApiClient";
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
					})
				: null,
		[state.session],
	);
	return {
		speedGroupRuntimeTransport: transport,
		speedGroupRuntimeAuthorityKey: [
			configuredServerUrl(),
			state.connectionGeneration,
			state.session?.session_id ?? "",
			state.session?.client_id ?? "",
			state.session?.desk.id ?? "",
		].join("|"),
		reportSpeedGroupSessionError: errors.reportSession,
		reportSpeedGroupMutationError: errors.reportMutation,
	};
}
