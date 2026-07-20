import { useCallback, useMemo, useRef } from "react";
import type { ServerState } from "../features/server/useServerState";
import { HttpVirtualPlaybackZonesTransport } from "../features/virtualPlaybackZones/transport";
import { configuredServerUrl } from "./LightApiClient";
import { HttpPlaybackTopologyTransport } from "./PlaybackTopologyTransport";
import { browserDeskBoundaryToken } from "./PatchTransport";

export function usePlaybackTopologyBoundaries(state: ServerState) {
	const playbackClientRef = useRef(state.client);
	playbackClientRef.current = state.client;
	const serverUrl = configuredServerUrl();
	const options = useMemo(
		() =>
			state.session
				? {
						baseUrl: serverUrl,
						sessionToken: state.session.token,
						deskBoundaryToken: browserDeskBoundaryToken(),
					}
				: null,
		[serverUrl, state.connectionGeneration, state.session],
	);
	const playbackTopologyTransport = useMemo(
		() => (options ? new HttpPlaybackTopologyTransport(options) : null),
		[options],
	);
	const virtualPlaybackZonesTransport = useMemo(
		() => (options ? new HttpVirtualPlaybackZonesTransport(options) : null),
		[options],
	);
	const showId = state.bootstrap?.active_show?.id ?? null;
	const deskId = state.session?.desk.id ?? null;
	const authorityId = [
		serverUrl,
		state.connectionGeneration,
		state.session?.session_id ?? "",
		state.session?.client_id ?? "",
	].join("|");
	const virtualPlaybackZonesAuthority =
		showId && deskId
			? { authorityId, scope: { showId, deskId } }
			: null;
	const applyPlaybackRuntimeAction = useCallback(
		(
			show: string,
			desk: string,
			request: Parameters<ServerState["client"]["playbackRuntimeAction"]>[2],
		) => playbackClientRef.current.playbackRuntimeAction(show, desk, request),
		[],
	);
	return {
		playbackTopologyTransport,
		virtualPlaybackZonesTransport,
		virtualPlaybackZonesAuthority,
		applyPlaybackRuntimeAction,
	};
}
