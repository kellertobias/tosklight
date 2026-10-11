import type { PlaybackAddressValue } from "./playbackAddress";

/** Validated operator configuration models, constructed at the API boundary. */
export interface PlaybackPreloadScope {
	position_ranges: Array<{ start: number; end: number }>;
	pages: number[] | null;
}
export interface PlaybackPreloadScopeSnapshot {
	show_id: string;
	revision: number;
	scope: PlaybackPreloadScope;
}
export interface PlaybackPreloadScopeUpdateOutcome extends PlaybackPreloadScopeSnapshot {
	request_id: string;
	replayed: boolean;
	changed: boolean;
}
export type PlaybackPageSource =
	| { type: "pinned"; page: number }
	| { type: "own"; page: number }
	| { type: "follow"; surface_id: string };
export interface PlaybackSurfaceConfiguration {
	name: string;
	layout: {
		rows: Array<{
			first_position: number;
			count: number;
			presentation: "fader" | "buttons" | "push_encoder";
			button_count: number;
		}>;
	};
	page_source: PlaybackPageSource;
}
export interface PlaybackSurfacesSnapshot {
	playback_surfaces: Record<string, PlaybackSurfaceConfiguration>;
	selected_surface_id: string;
	resolved_pages: Record<string, number>;
	command_page: number;
}
export interface PlaybackConfigurationNotification {
	desk_id: string;
	address: PlaybackAddressValue;
}
