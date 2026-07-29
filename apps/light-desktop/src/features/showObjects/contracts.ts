import type { DynamicDefinitionProjection } from "../../api/types";
import type {
	CueList,
	PatchLayer,
	PlaybackDefinition,
	PlaybackPage,
	StoredGroup,
	StoredPreset,
	VersionedObject,
} from "../../api/types";
import type { StoredDeskLayout, StoredStageLayout } from "../server/contracts";

export type ShowObjectKind =
	| "dynamic"
	| "group"
	| "preset"
	| "cue_list"
	| "patch_layer"
	| "playback"
	| "playback_page"
	| "stage_layout"
	| "user_layout";

export interface ShowObjectBodies {
	dynamic: DynamicDefinitionProjection;
	group: StoredGroup;
	preset: StoredPreset;
	cue_list: CueList;
	patch_layer: PatchLayer;
	playback: PlaybackDefinition;
	playback_page: PlaybackPage;
	stage_layout: StoredStageLayout;
	user_layout: StoredDeskLayout;
}

export type ShowObject<K extends ShowObjectKind = ShowObjectKind> =
	VersionedObject<ShowObjectBodies[K]> & {
		validationError?: string | null;
	};

export interface ShowObjectChange<K extends ShowObjectKind = ShowObjectKind> {
	kind: K;
	objectId: string;
	objectRevision: number;
	body: ShowObjectBodies[K] | null;
	validationError?: string | null;
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
