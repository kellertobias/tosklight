import {
	createContext,
	type PropsWithChildren,
	useContext,
	useEffect,
	useMemo,
} from "react";
import type { ShowObjectsStore } from "../showObjects/store";
import type { ShowObject } from "../showObjects/contracts";
import type {
	PresetRecordingActions,
	PresetRecordingTransport,
} from "./contracts";
import { PresetRecordingWriter } from "./writer";

interface PresetRecordingProviderProps {
	showId: string | null;
	store: ShowObjectsStore;
	transport: PresetRecordingTransport | null;
	loadPreset(showId: string, objectId: string): Promise<ShowObject<"preset"> | null>;
	onError?: (error: Error | null) => void;
}

const PresetRecordingContext =
	createContext<PresetRecordingActions | null>(null);

/** Mounting this action boundary performs no reads or subscriptions. */
export function PresetRecordingProvider({
	children,
	showId,
	store,
	transport,
	loadPreset,
	onError,
}: PropsWithChildren<PresetRecordingProviderProps>) {
	const writer = useMemo(
		() =>
			showId && transport
				? new PresetRecordingWriter({
						showId,
						store,
						transport,
						loadPreset,
						onError,
					})
				: null,
		[loadPreset, onError, showId, store, transport],
	);
	const actions = useMemo<PresetRecordingActions>(
		() =>
			writer ?? {
				record: async () => {
					onError?.(new Error("Preset recording is unavailable"));
					return null;
				},
			},
		[onError, writer],
	);
	useEffect(() => () => writer?.stop(), [writer]);
	return (
		<PresetRecordingContext.Provider value={actions}>
			{children}
		</PresetRecordingContext.Provider>
	);
}

export function usePresetRecording() {
	return useContext(PresetRecordingContext);
}
