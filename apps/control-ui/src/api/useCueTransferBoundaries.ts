import { useMemo } from "react";
import type { ServerState } from "../features/server/useServerState";
import { HttpCueTransferTransport } from "./CueTransferTransport";
import { createFeatureErrorGroup } from "./featureErrorReporting";
import { configuredServerUrl } from "./LightApiClient";
import { browserDeskBoundaryToken } from "./PatchTransport";

export function useCueTransferBoundaries(state: ServerState) {
	const errors = useMemo(
		() => createFeatureErrorGroup(state.setError),
		[state.setError],
	);
	const serverUrl = configuredServerUrl();
	const cueTransferTransport = useMemo(
		() =>
			state.session
				? new HttpCueTransferTransport({
						baseUrl: serverUrl,
						sessionToken: state.session.token,
						deskBoundaryToken: browserDeskBoundaryToken(),
					})
				: null,
		[serverUrl, state.connectionGeneration, state.session],
	);
	const cueTransferAuthorityKey = [
		serverUrl,
		state.connectionGeneration,
		state.session?.session_id ?? "",
		state.session?.client_id ?? "",
		state.session?.user.id ?? "",
		state.session?.desk.id ?? "",
		state.bootstrap?.active_show?.id ?? "",
	].join("|");
	return {
		cueTransferTransport,
		cueTransferConflictRepair: cueTransferTransport,
		cueTransferAuthorityKey,
		reportCueTransferError: errors.reportMutation,
	};
}
