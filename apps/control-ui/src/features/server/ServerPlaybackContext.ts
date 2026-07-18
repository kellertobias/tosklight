import type {
	DmxSnapshot,
	OutputRoute,
	VisualizationSnapshot,
} from "../../api/types";

export interface ServerPlaybackContext {
	playbackAction: (
		cueListId: string,
		action: "go" | "back" | "pause" | "release",
	) => Promise<void>;
	poolPlaybackAction: (
		number: number,
		action:
			| "button"
			| "on"
			| "off"
			| "toggle"
			| "go"
			| "go-minus"
			| "go-to"
			| "load"
			| "fast-forward"
			| "fast-rewind"
			| "temp"
			| "temp-on"
			| "temp-off"
			| "swap"
			| "select"
			| "select-contents"
			| "select-dereferenced"
			| "learn"
			| "double"
			| "half"
			| "pause"
			| "blackout"
			| "pause-dynamics"
			| "flash"
			| "master"
			| "xfade-on"
			| "xfade-off",
		input?: {
			value?: number;
			pressed?: boolean;
			button?: number;
			cue_number?: number;
			surface?: "physical" | "virtual";
		},
	) => Promise<void>;
	readVirtualPlaybackExclusionZones: () => Promise<
		import("../../api/types").VirtualPlaybackExclusionSnapshot
	>;
	saveVirtualPlaybackExclusionZones: (
		surfaceId: string,
		zones: import("../../api/types").VirtualPlaybackExclusionZone[],
	) => Promise<boolean>;
	setPlaybackPage: (page: number) => Promise<void>;
	savePlaybackPage: (
		page: import("../../api/types").PlaybackPage,
	) => Promise<boolean>;
	savePlaybackDefinition: (
		playback: import("../../api/types").PlaybackDefinition,
	) => Promise<void>;
	savePlaybackSlot: (
		page: number,
		slot: number,
		playback: import("../../api/types").PlaybackDefinition,
	) => Promise<boolean>;
	clearPlaybackSlot: (page: number, slot: number) => Promise<boolean>;
	saveCueList: (
		cueList: import("../../api/types").CueList,
		revision: number,
	) => Promise<boolean>;
	unassignPagePlayback: (page: number, slot: number) => Promise<boolean>;
	readDmx: () => Promise<DmxSnapshot>;
	readVisualization: (preload?: boolean) => Promise<VisualizationSnapshot>;
	setDmxOverride: (
		universe: number,
		address: number,
		value: number | null,
	) => Promise<void>;
	saveOutputRoute: (
		id: string,
		route: OutputRoute,
		revision: number,
	) => Promise<boolean>;
	deleteOutputRoute: (id: string, revision: number) => Promise<boolean>;
	storePlayback: (
		slot: number,
		cueListId?: string,
		pageNumber?: number,
	) => Promise<void>;
}
