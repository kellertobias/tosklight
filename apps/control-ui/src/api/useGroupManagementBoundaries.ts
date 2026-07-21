import { useMemo } from "react";
import type { ServerState } from "../features/server/useServerState";
import { createFeatureErrorGroup } from "./featureErrorReporting";
import { HttpGroupManagementTransport } from "./GroupManagementTransport";
import { configuredServerUrl } from "./LightApiClient";
import { browserDeskBoundaryToken } from "./PatchTransport";

export function useGroupManagementBoundaries(state: ServerState) {
	const errors = useMemo(
		() => createFeatureErrorGroup(state.setError),
		[state.setError],
	);
	const groupManagementTransport = useMemo(
		() =>
			state.session
				? new HttpGroupManagementTransport({
						baseUrl: configuredServerUrl(),
						sessionToken: state.session.token,
						deskBoundaryToken: browserDeskBoundaryToken(),
					})
				: null,
		[state.session],
	);
	return {
		groupManagementTransport,
		reportGroupManagementError: errors.reportMutation,
	};
}
