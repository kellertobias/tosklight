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

export type PlaybackPaneShape = "pane_normal" | "pane_taller" | "pane_wider";

export type PlaybackPaneFootprintProjection =
	| { type: "single_button" }
	| {
			type: PlaybackPaneShape;
			rowStart: number;
			columnStart: number;
			rowSpan: 3 | 4;
			columnSpan: 1 | 2;
	  };

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
	paneFootprint?: PlaybackPaneFootprintProjection;
};

export type PaneFootprintDefault = "single_button" | "pane_normal";

export type PlaybackConfigurationState = {
	playback: PlaybackDefinition;
	page: number;
	slot: number;
	empty: boolean;
	fallbackButtons: number;
	paneFootprintDefault?: PaneFootprintDefault;
	expectedPageRevision: number;
	expectedPageObjectId: string | null;
};
