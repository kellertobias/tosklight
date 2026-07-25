import {
	createContext,
	type PropsWithChildren,
	useContext,
	useEffect,
	useMemo,
} from "react";
import type { PlaybackRuntimeStore } from "../playbackRuntime/store";
import type {
	ShowObject,
	ShowObjectKind,
} from "../showObjects/contracts";
import type { ShowObjectsStore } from "../showObjects/store";
import type {
	CueRecordingActions,
	CueRecordingTransport,
} from "./contracts";
import { CueRecordingWriter } from "./writer";

interface CueRecordingProviderProps {
	showId: string | null;
	store: ShowObjectsStore;
	playbackRuntimeStore: PlaybackRuntimeStore;
	transport: CueRecordingTransport | null;
	selectedPlayback(): number | null;
	loadObject<K extends ShowObjectKind>(
		showId: string,
		kind: K,
		objectId: string,
	): Promise<ShowObject<K> | null>;
	onError?: (error: Error | null) => void;
}

const CueRecordingContext = createContext<CueRecordingActions | null>(null);

/** Mounting this action boundary performs no reads or subscriptions. */
export function CueRecordingProvider({
	children,
	showId,
	store,
	playbackRuntimeStore,
	transport,
	selectedPlayback,
	loadObject,
	onError,
}: PropsWithChildren<CueRecordingProviderProps>) {
	const writer = useMemo(
		() =>
			showId && transport
				? new CueRecordingWriter({
						showId,
						store,
						playbackRuntimeStore,
						transport,
						selectedPlayback,
						loadObject,
						onError,
					})
				: null,
		[
			loadObject,
			onError,
			playbackRuntimeStore,
			selectedPlayback,
			showId,
			store,
			transport,
		],
	);
	const actions = useMemo<CueRecordingActions>(
		() =>
			writer ?? {
				record: async () => {
					onError?.(new Error("Cue recording is unavailable"));
					return null;
				},
			},
		[onError, writer],
	);
	useEffect(() => () => writer?.stop(), [writer]);
	return (
		<CueRecordingContext.Provider value={actions}>
			{children}
		</CueRecordingContext.Provider>
	);
}

export function useCueRecording() {
	return useContext(CueRecordingContext);
}
