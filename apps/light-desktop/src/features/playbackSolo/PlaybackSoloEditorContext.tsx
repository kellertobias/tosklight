import {
	createContext,
	useContext,
	useMemo,
	useSyncExternalStore,
	type PropsWithChildren,
} from "react";
import type { PlaybackAddressValue } from "../../api/playbackAddress";
import type { PlaybackSoloRegion } from "./contracts";

/** UI-only draft controls. Saving still uses the authoritative Solo capability. */
export interface PlaybackSoloEditor {
	readonly available: boolean;
	readonly ready: boolean;
	readonly selecting: boolean;
	readonly selected: readonly PlaybackAddressValue[];
	readonly name: string;
	readonly saving: boolean;
	readonly error: string | null | undefined;
	setName(name: string): void;
	begin(): void;
	beginEdit(region: PlaybackSoloRegion): void;
	save(): Promise<void>;
	cancel(): void;
}

export class PlaybackSoloEditorRegistry {
	private readonly editors = new Map<string, PlaybackSoloEditor>();
	private readonly listeners = new Set<() => void>();
	get = (surfaceId: string) => this.editors.get(surfaceId) ?? null;
	subscribe = (listener: () => void) => {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	};
	register(surfaceId: string, editor: PlaybackSoloEditor) {
		this.editors.set(surfaceId, editor);
		this.notify();
		return () => {
			if (this.editors.get(surfaceId) !== editor) return;
			this.editors.delete(surfaceId);
			this.notify();
		};
	}
	private notify() {
		for (const listener of this.listeners) listener();
	}
}

const Context = createContext<PlaybackSoloEditorRegistry | null>(null);
export function PlaybackSoloEditorProvider({ children }: PropsWithChildren) {
	const registry = useMemo(() => new PlaybackSoloEditorRegistry(), []);
	return <Context.Provider value={registry}>{children}</Context.Provider>;
}
export function usePlaybackSoloEditorRegistry() {
	return useContext(Context);
}
const noSubscribe = () => () => {};
export function usePlaybackSoloEditor(surfaceId?: string) {
	const registry = useContext(Context);
	const snapshot = () => surfaceId ? registry?.get(surfaceId) ?? null : null;
	return useSyncExternalStore(
		registry?.subscribe ?? noSubscribe,
		snapshot,
		snapshot,
	);
}
