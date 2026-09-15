/**
 * The move preview in flight, shared by every tile of the CAD without re-rendering the CAD itself.
 *
 * A drag reports a new preview on every pointer move. Holding it in CadApp state re-rendered the
 * whole screen — title bar, side panels and every tile's chrome — once per move, so the dragged
 * element trailed the pointer. The store is read only by the tiles' viewports, synchronously.
 *
 * A preview belongs to the scene revision it was drawn on. Once a newer scene arrives (the commit
 * of this move, or another window's change) the stale preview is no longer shown, in the same
 * render that shows the new positions, so a released element never flashes back to where it was.
 */
import { useSyncExternalStore } from "react";
import type { CadTransformPreview } from "./types";

export interface StoredCadPreview {
	preview: CadTransformPreview;
	/** The scene revision whose positions the preview's delta is added to. */
	sceneRevision: number;
}

export interface CadPreviewStore {
	get(): StoredCadPreview | null;
	set(preview: CadTransformPreview, sceneRevision: number): void;
	clear(): void;
	subscribe(listener: () => void): () => void;
}

export function createPreviewStore(): CadPreviewStore {
	let current: StoredCadPreview | null = null;
	const listeners = new Set<() => void>();
	const notify = () => {
		for (const listener of listeners) listener();
	};
	return {
		get: () => current,
		set(preview, sceneRevision) {
			current = { preview, sceneRevision };
			notify();
		},
		clear() {
			if (!current) return;
			current = null;
			notify();
		},
		subscribe(listener) {
			listeners.add(listener);
			return () => listeners.delete(listener);
		},
	};
}

/** The preview to draw over a scene at `sceneRevision`, or null when there is none for it. */
export function useLivePreview(
	store: CadPreviewStore,
	sceneRevision: number,
): CadTransformPreview | null {
	const stored = useSyncExternalStore(store.subscribe, store.get);
	return stored && stored.sceneRevision === sceneRevision ? stored.preview : null;
}
