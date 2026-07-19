import type {
	StoredGroup,
	StoredPreset,
	VersionedObject,
} from "../../api/types";

export type ShowObjectKind = "group" | "preset";

export interface ShowObjectBodies {
	group: StoredGroup;
	preset: StoredPreset;
}

export type ShowObject<K extends ShowObjectKind = ShowObjectKind> =
	VersionedObject<ShowObjectBodies[K]>;

export interface ShowObjectChange<K extends ShowObjectKind = ShowObjectKind> {
	kind: K;
	objectId: string;
	objectRevision: number;
	body: ShowObjectBodies[K] | null;
	deleted: boolean;
}

export interface ShowObjectsChange {
	showId: string;
	showRevision: number;
	eventSequence: number;
	changes: ShowObjectChange[];
}

export interface ShowObjectMutationResponse {
	revision: number;
	event_sequence: number | null;
}

export type ShowObjectsEventMessage =
	| { type: "ready"; cursor: number }
	| { type: "event"; change: ShowObjectsChange }
	| {
			type: "gap";
			afterSequence: number;
			oldestAvailable: number;
			latestSequence: number;
	  }
	| { type: "repaired"; cursor: number }
	| { type: "error"; error: string };

export type ShowObjectCollections = {
	[K in ShowObjectKind]: ShowObject<K>[];
};
