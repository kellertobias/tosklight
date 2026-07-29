import { useCallback, useMemo, useRef } from "react";
import type { ServerState } from "../features/server/useServerState";
import { HttpVirtualPlaybackZonesTransport } from "../features/virtualPlaybackZones/transport";
import { configuredServerUrl } from "./client/serverLocation";
import { browserDeskBoundaryToken } from "./PatchTransport";
import { HttpPlaybackTopologyTransport } from "./PlaybackTopologyTransport";

export function usePlaybackTopologyBoundaries(state: ServerState) {
	const playbackClientRef = useRef(state.api.playback);
	playbackClientRef.current = state.api.playback;
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
	const authorityId = [
		serverUrl,
		state.connectionGeneration,
		state.session?.session_id ?? "",
		state.session?.client_id ?? "",
	].join("|");
	const virtualPlaybackZonesAuthority = showId
		? { authorityId, scope: { showId } }
		: null;
	const applyPlaybackRuntimeAction = useCallback(
		(
			_show: string,
			_desk: string,
			request: Parameters<
				ServerState["api"]["playback"]["playbackRuntimeAction"]
			>[2],
		) => playbackClientRef.current.playbackRuntimeLiveAction(request),
		[],
	);
	const applyPlaybackDeskPage = useCallback(
		(desk: string, page: number) =>
			playbackClientRef.current.setPlaybackPage(desk, page, {
				existingOnly: true,
			}),
		[],
	);
	return {
		playbackTopologyTransport,
		virtualPlaybackZonesTransport,
		virtualPlaybackZonesAuthority,
		applyPlaybackRuntimeAction,
		applyPlaybackDeskPage,
	};
}
