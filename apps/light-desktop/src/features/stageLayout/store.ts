import type { VersionedObject } from "../../api/types";
import type { StoredStageLayout } from "../server/contracts";

export type StageLayoutObject = VersionedObject<StoredStageLayout>;

export interface StageLayoutSnapshot {
	/** Null until the show's stage layout has been loaded. */
	layout: StageLayoutObject | null;
}

const EMPTY: StageLayoutSnapshot = { layout: null };

/**
 * Authoritative stage layout for scoped readers.
 *
 * Stage positions are read on hot paths such as Cue thumbnails and the Stage window, so they are
 * kept outside the broad server-context update path.
 */
export class StageLayoutStore {
	private readonly listeners = new Set<() => void>();
	private value: StageLayoutSnapshot = EMPTY;

	subscribe = (listener: () => void) => {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	};

	getSnapshot = () => this.value;

	install(layout: StageLayoutObject | null): void {
		if (this.value.layout === layout) return;
		this.value = layout === null ? EMPTY : { layout };
		for (const listener of this.listeners) listener();
	}
}

export const EMPTY_STAGE_LAYOUT_SNAPSHOT = EMPTY;
