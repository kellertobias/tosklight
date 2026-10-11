import type { CueList, PlaybackDefinition } from "../../api/types";
import type {
	ShowObject,
	ShowObjectBodies,
	ShowObjectKind,
} from "../showObjects/contracts";

export interface PlaybackAddress {
	page: number;
	position: number;
}

export type PlaybackTopologyAction =
	| {
			type: "save_cue_list";
			cueListId: string;
			expectedRevision: number;
			expectedObjectId: string | null;
			body: CueList;
	  }
	| {
			type: "undo_cue_list";
			cueListId: string;
			expectedRevision: number;
			expectedObjectId: string;
	  }
	| {
			type: "redo_cue_list";
			cueListId: string;
			expectedRevision: number;
			expectedObjectId: string;
	  }
	| ({
			type: "assign_playback";
			address: PlaybackAddress;
			definition: PlaybackDefinition;
	  } & PlaybackPageRevisionBasis)
	| ({
			type: "update_playback";
			address: PlaybackAddress;
			definition: PlaybackDefinition;
	  } & PlaybackPageRevisionBasis)
	| ({
			type: "clear_playback";
			address: PlaybackAddress;
	  } & PlaybackPageRevisionBasis)
	| ({
			type: "assign_group_master";
			groupObjectId: string;
			expectedGroupRevision: number;
			address: PlaybackAddress;
	  } & PlaybackPageRevisionBasis)
	| ({
			type: "create_playback_page";
			number: number;
			name: string;
	  } & PlaybackPageRevisionBasis)
	| ({
			type: "rename_playback_page";
			number: number;
			name: string;
	  } & PlaybackPageRevisionBasis)
	| ({
			type: "delete_playback_page";
			number: number;
	  } & PlaybackPageRevisionBasis);

export interface PlaybackTopologyRequest {
	requestId: string;
	action: PlaybackTopologyAction;
}

export type PlaybackTopologyResolution =
	| { kind: "cue_list"; cueListId: string }
	| { kind: "page"; page: number }
	| { kind: "playback"; page: number; position: number };

export type PlaybackTopologyObject<K extends ShowObjectKind = ShowObjectKind> =
	| {
			state: "present";
			kind: K;
			objectId: string;
			objectRevision: number;
			body: ShowObjectBodies[K];
	  }
	| {
			state: "deleted";
			kind: K;
			objectId: string;
			objectRevision: number;
	  };

interface PlaybackTopologyOutcomeBase {
	requestId: string;
	correlationId: string;
	showRevision: number;
	resolution: PlaybackTopologyResolution;
	objects: PlaybackTopologyObject[];
	replayed: boolean;
}

export type PlaybackTopologyOutcome = PlaybackTopologyOutcomeBase &
	(
		| { status: "changed"; eventSequence: number }
		| { status: "no_change"; eventSequence?: never }
	);

export interface PlaybackTopologyTransport {
	apply(
		showId: string,
		expectedShowRevision: number,
		request: PlaybackTopologyRequest,
	): Promise<PlaybackTopologyOutcome>;
}

export interface PlaybackTopologyActions {
	createPage(
		page: number,
		revisionBasis?: PlaybackPageRevisionBasis,
		name?: string,
	): Promise<PlaybackTopologyOutcome | null>;
	renamePage(
		page: number,
		name: string,
		revisionBasis?: ExistingPlaybackPageRevisionBasis,
	): Promise<PlaybackTopologyOutcome | null>;
	deletePage(
		page: number,
		revisionBasis?: ExistingPlaybackPageRevisionBasis,
	): Promise<PlaybackTopologyOutcome | null>;
	saveCueList(
		cueListId: string,
		expectedRevision: number,
		expectedObjectId: string | null,
		body: CueList,
	): Promise<PlaybackTopologyOutcome | null>;
	undoCueList(
		cueListId: string,
		expectedRevision: number,
		expectedObjectId: string,
	): Promise<PlaybackTopologyOutcome | null>;
	redoCueList(
		cueListId: string,
		expectedRevision: number,
		expectedObjectId: string,
	): Promise<PlaybackTopologyOutcome | null>;
	assignPlayback(
		address: PlaybackAddress,
		definition: PlaybackDefinition,
		revisionBasis?: PlaybackPageRevisionBasis,
	): Promise<PlaybackTopologyOutcome | null>;
	updatePlayback(
		address: PlaybackAddress,
		definition: PlaybackDefinition,
		revisionBasis?: PlaybackPageRevisionBasis,
	): Promise<PlaybackTopologyOutcome | null>;
	clearPlayback(
		address: PlaybackAddress,
		revisionBasis?: PlaybackPageRevisionBasis,
	): Promise<PlaybackTopologyOutcome | null>;
	assignGroupMaster(
		groupObjectId: string,
		expectedGroupRevision: number,
		address: PlaybackAddress,
		revisionBasis?: PlaybackPageRevisionBasis,
	): Promise<PlaybackTopologyOutcome | null>;
}

export interface PlaybackPageRevisionBasis {
	expectedPageRevision: number;
	expectedPageObjectId: string | null;
}

export interface ExistingPlaybackPageRevisionBasis
	extends PlaybackPageRevisionBasis {
	expectedPageObjectId: string;
}

export interface PlaybackTopologyCapability extends PlaybackTopologyActions {
	readonly error: Error | null;
}

export interface PlaybackTopologyView {
	ready: boolean;
	error: Error | null;
	cueLists: readonly ShowObject<"cue_list">[];
	pages: readonly ShowObject<"playback_page">[];
}

export interface PlaybackPagesView {
	ready: boolean;
	error: Error | null;
	pages: readonly ShowObject<"playback_page">[];
}
