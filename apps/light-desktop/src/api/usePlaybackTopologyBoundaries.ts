import { useCallback, useMemo, useRef } from "react";
import type { ServerState } from "../features/server/useServerState";
import { HttpPreloadScopeTransport } from "../features/playbackPreloadScope/transport";
import { HttpPlaybackSoloTransport } from "../features/playbackSolo/transport";
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
	const playbackSoloTransport = useMemo(
		() => (options ? new HttpPlaybackSoloTransport(options) : null),
		[options],
	);
	const playbackPreloadScopeTransport = useMemo(
		() => (options ? new HttpPreloadScopeTransport(options) : null),
		[options],
	);
	const showId = state.bootstrap?.active_show?.id ?? null;
	const authorityId = [
		serverUrl,
		state.connectionGeneration,
		state.session?.session_id ?? "",
		state.session?.client_id ?? "",
	].join("|");
	const playbackSoloAuthority = showId
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
		(desk: string, page: number, surfaceId: string) =>
			playbackClientRef.current.setPlaybackPage(desk, page, surfaceId),
		[],
	);
	return {
		playbackTopologyTransport,
		playbackSoloTransport,
		playbackSoloAuthority,
		playbackPreloadScopeTransport,
		applyPlaybackRuntimeAction,
		applyPlaybackDeskPage,
	};
}
