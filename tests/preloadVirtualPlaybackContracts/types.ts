import type { summarizePlaybackState } from "./showFixtures";

export type Configuration = Record<string, any> & {
	programmer_fade_millis: number;
	sequence_master_fade_millis: number;
	preload_programmer_changes: boolean;
	preload_physical_playback_actions: boolean;
	preload_virtual_playback_actions: boolean;
};

export type PlaybackSpec = {
	number: number;
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
	pendingActions?: Array<[number, string, string]>;
	applicationTimestamp?: string;
};

export type PreloadMaskPairState = {
	savedMasks: Array<[boolean, boolean, boolean]>;
};

export type PreloadCombinedPairState = Prepared & {
	groupFixture: string;
	pending?: {
		groupIds: string[];
		playbackActions: Array<[number, string, string]>;
	};
	applicationTimestamp?: string;
};

export type VirtualZonePairState = Prepared & {
	savedZones?: Array<{ name: string; slots: number[] }>;
	creationState?: [boolean, boolean];
};
