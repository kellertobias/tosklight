/** The retained playback interaction captured while Preload is active. */
export type ProgrammerPreloadPlaybackAction =
	| "toggle"
	| "go"
	| "back"
	| "off"
	| "on"
	| "temporary_on"
	| "temporary_off";

export interface ProgrammerPreloadPlaybackQueueEntry {
	address: import("../../api/playbackAddress").PlaybackAddressValue;
	action: ProgrammerPreloadPlaybackAction;
	controlSource: { surface_id: string; control_id: number };
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
