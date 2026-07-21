import type { CueList, PlaybackDefinition } from "../../api/types";
import type {
	ShowObject,
	ShowObjectBodies,
	ShowObjectKind,
} from "../showObjects/contracts";

export type PlaybackTopologyAction =
	| {
			type: "save_cue_list";
			cueListId: string;
			expectedRevision: number;
			expectedObjectId: string | null;
			body: CueList;
	  }
	| {
			type: "configure_slot";
			page: number;
			slot: number;
			expectedPageRevision: number;
			expectedPageObjectId: string | null;
			expectedPlaybackRevision: number;
			expectedPlaybackObjectId: string | null;
			playback: PlaybackDefinition;
	  }
	| {
			type: "map_existing_playback";
			page: number;
			slot: number;
			playbackNumber: number;
			expectedPageRevision: number;
			expectedPageObjectId: string | null;
			expectedPlaybackRevision: number;
			expectedPlaybackObjectId: string;
	  }
	| {
			type: "clear_mapped_playback";
			page: number;
			slot: number;
			expectedPageRevision: number;
			expectedPageObjectId: string | null;
			expectedPlaybackRevision: number;
			expectedPlaybackObjectId: string | null;
	  }
	| {
			type: "create_page";
			page: number;
			expectedPageRevision: number;
			expectedPageObjectId: string | null;
	  }
	| {
			type: "rename_page";
			page: number;
			name: string;
			expectedPageRevision: number;
			expectedPageObjectId: string;
	  };

export interface PlaybackTopologyRequest {
	requestId: string;
	action: PlaybackTopologyAction;
}

export type PlaybackTopologyResolution =
	| { kind: "cue_list"; cueListId: string }
	| { kind: "page"; page: number }
	| {
			kind: "page_slot";
			page: number;
			slot: number;
			playbackNumber: number | null;
	  };

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
	): Promise<PlaybackTopologyOutcome | null>;
	renamePage(
		page: number,
		name: string,
		revisionBasis?: ExistingPlaybackPageRevisionBasis,
	): Promise<PlaybackTopologyOutcome | null>;
	saveCueList(
		cueListId: string,
		expectedRevision: number,
		expectedObjectId: string | null,
		body: CueList,
	): Promise<PlaybackTopologyOutcome | null>;
	configureSlot(
		page: number,
		slot: number,
		playback: PlaybackDefinition,
		revisionBasis?: PlaybackTopologyRevisionBasis,
	): Promise<PlaybackTopologyOutcome | null>;
	mapExistingPlayback(
		page: number,
		slot: number,
		playbackNumber: number,
		revisionBasis?: ExistingPlaybackRevisionBasis,
	): Promise<PlaybackTopologyOutcome | null>;
	clearMappedPlayback(
		page: number,
		slot: number,
		revisionBasis?: PlaybackTopologyRevisionBasis,
	): Promise<PlaybackTopologyOutcome | null>;
}

export interface PlaybackTopologyRevisionBasis {
	expectedPageRevision: number;
	expectedPageObjectId: string | null;
	expectedPlaybackRevision: number;
	expectedPlaybackObjectId: string | null;
}

export interface PlaybackPageRevisionBasis {
	expectedPageRevision: number;
	expectedPageObjectId: string | null;
}

export interface ExistingPlaybackPageRevisionBasis
	extends PlaybackPageRevisionBasis {
	expectedPageObjectId: string;
}

export interface ExistingPlaybackRevisionBasis
	extends Omit<PlaybackTopologyRevisionBasis, "expectedPlaybackObjectId"> {
	expectedPlaybackObjectId: string;
}

export interface PlaybackTopologyCapability extends PlaybackTopologyActions {
	readonly error: Error | null;
}

export interface PlaybackTopologyView {
	ready: boolean;
	error: Error | null;
	cueLists: readonly ShowObject<"cue_list">[];
	playbacks: readonly ShowObject<"playback">[];
	pages: readonly ShowObject<"playback_page">[];
}

export interface PlaybackPagesView {
	ready: boolean;
	error: Error | null;
	pages: readonly ShowObject<"playback_page">[];
}
