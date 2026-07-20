import {
	createContext,
	type PropsWithChildren,
	useContext,
	useCallback,
	useEffect,
	useMemo,
	useState,
} from "react";
import type {
	ShowObject,
	ShowObjectKind,
} from "../showObjects/contracts";
import type { ShowObjectsStore } from "../showObjects/store";
import type {
	PlaybackTopologyActions,
	PlaybackTopologyCapability,
	PlaybackTopologyTransport,
} from "./contracts";
import { useStrictModeSafeStop } from "../shared/useStrictModeSafeStop";
import { PlaybackTopologyWriter } from "./writer";

interface PlaybackTopologyProviderProps {
	showId: string | null;
	store: ShowObjectsStore;
	transport: PlaybackTopologyTransport | null;
	loadObject<K extends ShowObjectKind>(
		showId: string,
		kind: K,
		objectId: string,
	): Promise<ShowObject<K> | null>;
	onError?: (error: Error | null) => void;
}

const PlaybackTopologyContext = createContext<PlaybackTopologyCapability | null>(
	null,
);

/** Action-only boundary; views separately opt into exact topology hydration. */
export function PlaybackTopologyProvider({
	children,
	showId,
	store,
	transport,
	loadObject,
	onError,
}: PropsWithChildren<PlaybackTopologyProviderProps>) {
	const [error, setError] = useState<Error | null>(null);
	const reportError = useCallback(
		(next: Error | null) => {
			setError(next);
			onError?.(next);
		},
		[onError],
	);
	const writer = useMemo(
		() =>
			showId && transport
				? new PlaybackTopologyWriter({
						showId,
						store,
						transport,
						loadObject,
						onError: reportError,
					})
				: null,
		[loadObject, reportError, showId, store, transport],
	);
	useStrictModeSafeStop(writer);
	useEffect(() => setError(null), [writer]);
	const actions = useMemo<PlaybackTopologyActions | null>(
		() =>
			writer && {
				saveCueList: writer.saveCueList.bind(writer),
				configureSlot: writer.configureSlot.bind(writer),
				clearMappedPlayback: writer.clearMappedPlayback.bind(writer),
			},
		[writer],
	);
	const capability = useMemo<PlaybackTopologyCapability | null>(
		() => actions && { ...actions, error },
		[actions, error],
	);
	return (
		<PlaybackTopologyContext.Provider value={capability}>
			{children}
		</PlaybackTopologyContext.Provider>
	);
}

export function usePlaybackTopologyActions() {
	return useContext(PlaybackTopologyContext);
}
