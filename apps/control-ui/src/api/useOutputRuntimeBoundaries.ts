import { useMemo } from "react";
import type { ServerState } from "../features/server/useServerState";
import { createFeatureErrorGroup } from "./featureErrorReporting";
import { configuredServerUrl } from "./LightApiClient";
import { HttpOutputRuntimeTransport } from "./OutputRuntimeTransport";
import { browserDeskBoundaryToken } from "./PatchTransport";

/** Constructs the desk-authenticated adapter without activating its I/O. */
export function useOutputRuntimeBoundaries(state: ServerState) {
	const errors = useMemo(
		() => createFeatureErrorGroup(state.setError),
		[state.setError],
	);
	const transport = useMemo(
		() =>
			state.session
				? new HttpOutputRuntimeTransport({
						baseUrl: configuredServerUrl(),
						sessionToken: state.session.token,
						authenticatedDeskId: state.session.desk.id,
						deskBoundaryToken: browserDeskBoundaryToken(),
					})
				: null,
		[state.session],
	);
	return {
		outputRuntimeTransport: transport,
		outputRuntimeAuthorityKey: [
			configuredServerUrl(),
			state.connectionGeneration,
			state.session?.session_id ?? "",
			state.session?.client_id ?? "",
			state.session?.user.id ?? "",
			state.session?.desk.id ?? "",
		].join("|"),
		reportOutputRuntimeSessionError: errors.reportSession,
		reportOutputRuntimeMutationError: errors.reportMutation,
	};
}
