import type { useServer } from "../../../api/ServerContext";
import type {
	CueList,
	PlaybackDefinition,
	PlaybackSurfaceRow,
} from "../../../api/types";

export type PlaybackServer = ReturnType<typeof useServer>;
export type PlaybackSnapshotActive = NonNullable<
	PlaybackServer["playbacks"]
>["active"][number];
export type AuthoritativeControls = NonNullable<
	NonNullable<PlaybackServer["playbacks"]>["authoritative_controls"]
>;
export type PlaybackGroup = PlaybackServer["groups"][number];

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
};
