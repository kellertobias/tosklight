import type { PlaybackAddress } from "../../apps/light-desktop/src/features/playbackTopology/contracts";
import type { PlaybackPreloadScope } from "../../apps/light-desktop/src/api/playbackConfigurationModels";
export type PendingPlaybackAction = [PlaybackAddress, string, {surface_id:string;control_id:number}];
export type CaptureMask = { programmer:boolean; playbackScope:PlaybackPreloadScope };
import type { summarizePlaybackState } from "./showFixtures";

export type Configuration = Record<string, any> & {
	programmer_fade_millis: number;
	sequence_master_fade_millis: number;
	preload_programmer_changes: boolean;
};

export type PlaybackSpec = {
	position: number;
	fixture: number;
	levels?: number[];
	name?: string;
	buttons?: [string, string, string];
	buttonCount?: number;
	hasFader?: boolean;
};

export type Prepared = {
	showId: string;
	fixtures: Record<number, string>;
	cueLists: Record<number, string>;
};

export type Preload003State = Prepared & {
	firstNumber: number;
	secondNumber: number;
	layoutDeskId: string;
};

export type PreloadProgrammerPairState = Prepared & {
	groupFixtures: [string, string];
	beforeLevels: [number, number];
	pending?: {
		blind: boolean;
		groupIds: string[];
		groupValues: string[];
		firstFadeMillis: number | null;
		secondFadeMillis: number | null;
		playbackActions: string[];
		liveLevels: [number, number];
	};
	applicationTimestamp?: string;
};

export type PreloadPlaybackPairState = Prepared & {
	pendingActions?: string[];
	applicationTimestamp?: string;
	committedState?: ReturnType<typeof summarizePlaybackState>;
	releasedState?: ReturnType<typeof summarizePlaybackState>;
};

export type PreloadVirtualPairState = Prepared & {
	pendingActions?: PendingPlaybackAction[];
	applicationTimestamp?: string;
};

export type PreloadMaskPairState = {
	savedMasks: CaptureMask[];
};

export type PreloadCombinedPairState = Prepared & {
	groupFixture: string;
	pending?: {
		groupIds: string[];
		playbackActions: PendingPlaybackAction[];
	};
	applicationTimestamp?: string;
};

export type VirtualZonePairState = Prepared & {
	savedZones?: Array<{ name: string; members: PlaybackAddress[] }>;
	creationState?: [boolean, boolean];
};
