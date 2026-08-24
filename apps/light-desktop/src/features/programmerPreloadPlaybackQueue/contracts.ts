/** The retained playback interaction captured while Preload is active. */
export type ProgrammerPreloadPlaybackAction =
	| "toggle"
	| "go"
	| "back"
	| "off"
	| "on"
	| "temporary_on"
	| "temporary_off";

/** The application surface retained with a captured playback interaction. */
export type ProgrammerPreloadPlaybackSurface =
	| "virtual"
	| "physical"
	| "osc"
	| "matter";

export interface ProgrammerPreloadPlaybackQueueEntry {
	playbackNumber: number;
	page: number | null;
	action: ProgrammerPreloadPlaybackAction;
	surface: ProgrammerPreloadPlaybackSurface;
}

/** The Programmer's ordered pending playback actions. */
export interface ProgrammerPreloadPlaybackQueueProjection {
	revision: number;
	actions: readonly ProgrammerPreloadPlaybackQueueEntry[];
}

export interface ProgrammerPreloadPlaybackQueueSnapshot {
	cursor: number;
	projection: ProgrammerPreloadPlaybackQueueProjection;
}

export interface ProgrammerPreloadPlaybackQueueScope {
	showId: string;
}

export type ProgrammerPreloadPlaybackQueueEventMessage =
	| { type: "ready"; cursor: number }
	| {
			type: "event";
			sequence: number;
			correlationId: string | null;
			projection: ProgrammerPreloadPlaybackQueueProjection;
	  }
	| {
			type: "gap";
			afterSequence: number;
			oldestAvailable: number;
			latestSequence: number;
	  }
	| { type: "repaired"; cursor: number }
	| { type: "error"; error: string };
