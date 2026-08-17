export type ControlSurfaceSource =
	| "touch"
	| "mouse"
	| "keyboard"
	| "context_menu"
	| "osc"
	| "hardware"
	| "server";

/** The desk-owned interaction scope captured when SET is armed. */
export interface ControlSurfaceInteractionScope {
	deskId: string;
	showId: string;
	surfaceId: string;
}

/** The exact Group object observed by the originating surface. */
export interface GroupInteractionIdentity {
	objectId: string;
	objectRevision: number;
}

/**
 * The concrete Playback page/slot observed by the originating surface. Addressing
 * remains explicit so a current-page touch cannot be confused with a typed page.
 */
export type PlaybackInteractionIdentity =
	| {
			addressing: "current_page" | "explicit_page";
			pageNumber: number;
			slot: number;
			pageObjectId: string | null;
			pageObjectRevision: number;
			playbackObjectId: string | null;
			playbackObjectRevision: number;
	  }
	| {
			addressing: "virtual";
			pageNumber: number;
			playbackNumber: number;
			pageObjectId: string | null;
			pageObjectRevision: number;
	  };

interface ScopedTerminalIntent {
	source: ControlSurfaceSource;
	scope: ControlSurfaceInteractionScope;
}

export type SelectGroupLive = ScopedTerminalIntent & {
	type: "select_group_live";
	group: GroupInteractionIdentity;
};

export type SelectGroupFrozen = ScopedTerminalIntent & {
	type: "select_group_frozen";
	group: GroupInteractionIdentity;
};

export type OpenGroupSettings = ScopedTerminalIntent & {
	type: "open_group_settings";
	group: GroupInteractionIdentity;
};

export type OpenPlaybackSettings = ScopedTerminalIntent & {
	type: "open_playback_settings";
	playback: PlaybackInteractionIdentity;
};

export type ChooseGroupMasterSource = ScopedTerminalIntent & {
	type: "choose_group_master_source";
	group: GroupInteractionIdentity;
};

export type AssignGroupMaster = ScopedTerminalIntent & {
	type: "assign_group_master";
	group: GroupInteractionIdentity;
	playback: PlaybackInteractionIdentity;
};

export type AssignObject = ScopedTerminalIntent & {
	type: "assign_object";
	sourceCommand: string;
	playback: PlaybackInteractionIdentity;
};

export type SetInteractionTerminalIntent =
	| SelectGroupLive
	| SelectGroupFrozen
	| OpenGroupSettings
	| OpenPlaybackSettings
	| ChooseGroupMasterSource
	| AssignGroupMaster
	| AssignObject;
