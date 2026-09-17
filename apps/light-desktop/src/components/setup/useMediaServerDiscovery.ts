import type { TitleActionGroup } from "@tosklight/ui";
import { useCallback, useEffect, useRef, useState } from "react";
import type {
	DiscoveredMediaOutput,
	MediaServerDiscovery,
} from "../../api/client/mediaOutput";
import { useMediaServers } from "../../features/mediaServers/MediaServersContext";

export type MediaServerDiscoveryController = {
	discovery: MediaServerDiscovery | null;
	busy: boolean;
	error: string | null;
	refresh: () => Promise<void>;
	updateOutput: (serverKey: string, output: DiscoveredMediaOutput) => void;
};

const UNAVAILABLE =
	"Media Server discovery is unavailable. Check the desk's network connection, then Refresh Discovery. Manual patching still works.";

/**
 * Media Server discovery for Show Patch: refreshed once whenever the view becomes active and on
 * demand from the window title.
 */
export function useMediaServerDiscovery(
	active: boolean,
): MediaServerDiscoveryController {
	const server = useMediaServers();
	// The Media Servers state changes with every pushed status; discovery must not follow it.
	const serverRef = useRef(server);
	serverRef.current = server;
	const connected = Boolean(server);
	const [discovery, setDiscovery] = useState<MediaServerDiscovery | null>(null);
	const [busy, setBusy] = useState(false);
	const [error, setError] = useState<string | null>(null);
	const refresh = useCallback(async () => {
		const server = serverRef.current;
		if (!server) return;
		setBusy(true);
		setError(null);
		try {
			const next = await server.discoverMediaServers();
			setDiscovery(next);
			setError(next.discoveryError);
		} catch (reason) {
			setError(
				reason instanceof Error && reason.message
					? `${reason.message} Refresh Discovery to retry; manual patching still works.`
					: UNAVAILABLE,
			);
		} finally {
			setBusy(false);
		}
	}, []);
	const updateOutput = useCallback(
		(serverKey: string, output: DiscoveredMediaOutput) =>
			setDiscovery((current) =>
				current
					? {
							...current,
							servers: current.servers.map((candidate) =>
								candidate.key === serverKey
									? {
											...candidate,
											outputs: candidate.outputs.map((known) =>
												known.id === output.id ? output : known,
											),
										}
									: candidate,
							),
						}
					: current,
			),
		[],
	);
	useEffect(() => {
		if (active && connected) void refresh();
	}, [active, connected, refresh]);
	return { discovery, busy, error, refresh, updateOutput };
}

/** Refresh Discovery as its own window-title action group. */
export function mediaDiscoveryGroup(
	controller: Pick<MediaServerDiscoveryController, "busy" | "refresh">,
): TitleActionGroup {
	return {
		id: "media-discovery",
		actions: [
			{
				id: "refresh-discovery",
				label: controller.busy ? "Discovering…" : "Refresh Discovery",
				disabled: controller.busy,
				onPress: () => void controller.refresh(),
			},
		],
	};
}
