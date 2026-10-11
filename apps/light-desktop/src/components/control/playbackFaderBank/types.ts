import type { PlaybackAddressValue } from "../../../api/playbackAddress";
import type {
	CueList,
	PlaybackDefinition,
	PlaybackSurfaceRow,
} from "../../../api/types";
import type { PlaybackControlRuntime } from "../../../features/playbackRuntime/controlRuntime";
import type { ShowObject } from "../../../features/showObjects/contracts";
import type { PlaybackFootprintCellProjection } from "./footprints";

export type AssignedPlayback = PlaybackDefinition & {
	address: PlaybackAddressValue;
};

export type PlaybackSnapshotActive = PlaybackControlRuntime;
export type PlaybackGroup = ShowObject<"group">;

export type PlaybackSlotProjection = {
	playback: AssignedPlayback | null;
	cue: CueList | null;
	group: PlaybackGroup | null;
	slot: number;
	row: PlaybackSurfaceRow | null;
	rowIndex: number;
	columnIndex: number;
	footprint: Extract<
		PlaybackFootprintCellProjection,
		{ state: "anchor" }
	> | null;
};

export type PlaybackConfigurationState = {
	playback: PlaybackDefinition;
	page: number;
	slot: number;
	empty: boolean;
	fallbackButtons: number;
	expectedPageRevision: number;
	expectedPageObjectId: string | null;
};
