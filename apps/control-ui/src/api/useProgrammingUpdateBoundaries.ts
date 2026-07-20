import { useMemo } from "react";
import type { ServerState } from "../features/server/useServerState";
import { configuredServerUrl } from "./LightApiClient";
import { browserDeskBoundaryToken } from "./PatchTransport";
import { HttpProgrammingUpdateTransport } from "./ProgrammingUpdateTransport";

export function useProgrammingUpdateBoundaries(state: ServerState) {
	const serverUrl = configuredServerUrl();
	const programmingUpdateTransport = useMemo(
		() =>
			state.session
				? new HttpProgrammingUpdateTransport({
						baseUrl: serverUrl,
						sessionToken: state.session.token,
						deskBoundaryToken: browserDeskBoundaryToken(),
					})
				: null,
		[serverUrl, state.connectionGeneration, state.session],
	);
	const programmingUpdateAuthorityKey = [
		serverUrl,
		state.connectionGeneration,
		state.session?.session_id ?? "",
		state.session?.client_id ?? "",
		state.session?.user.id ?? "",
		state.session?.desk.id ?? "",
		state.bootstrap?.active_show?.id ?? "",
	].join("|");
	return { programmingUpdateTransport, programmingUpdateAuthorityKey };
}
