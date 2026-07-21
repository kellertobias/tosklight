import { useMemo } from "react";
import type { ServerState } from "../features/server/useServerState";
import { createFeatureErrorGroup } from "./featureErrorReporting";
import { configuredServerUrl } from "./LightApiClient";
import { browserDeskBoundaryToken } from "./PatchTransport";
import { HttpPresetRecallTransport } from "./PresetRecallTransport";

export function usePresetRecallBoundaries(state: ServerState) {
	const errors = useMemo(
		() => createFeatureErrorGroup(state.setError),
		[state.setError],
	);
	const presetRecallTransport = useMemo(
		() =>
			state.session
				? new HttpPresetRecallTransport({
						baseUrl: configuredServerUrl(),
						sessionToken: state.session.token,
						deskBoundaryToken: browserDeskBoundaryToken(),
					})
				: null,
		[state.session],
	);
	return {
		presetRecallTransport,
		presetRecallAuthorityKey: [
			configuredServerUrl(),
			state.connectionGeneration,
			state.session?.session_id ?? "",
			state.session?.client_id ?? "",
			state.session?.user.id ?? "",
			state.session?.desk.id ?? "",
		].join("|"),
		reportPresetRecallError: errors.reportMutation,
	};
}
