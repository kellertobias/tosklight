import type {
	CueList,
	PlaybackDefinition,
	PlaybackSurfaceRow,
} from "../../../api/types";
import type { LegacyPlaybackRuntime } from "../../../features/playbackRuntime/legacy";
import type { ShowObject } from "../../../features/showObjects/contracts";

export type PlaybackSnapshotActive = LegacyPlaybackRuntime;
export type PlaybackGroup = ShowObject<"group">;

export type PlaybackSlotProjection = {
	playback: PlaybackDefinition | null;
	cue: CueList | null;
	group: PlaybackGroup | null;
	slot: number;
	row: PlaybackSurfaceRow | null;
	rowIndex: number;
};

export type PlaybackConfigurationState = {
	playback: PlaybackDefinition;
	page: number;
	slot: number;
	empty: boolean;
	fallbackButtons: number;
	expectedPageRevision: number;
	expectedPageObjectId: string | null;
	expectedPlaybackRevision: number;
	expectedPlaybackObjectId: string | null;
};
