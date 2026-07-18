import type { SpeedSnapshot } from "./configuration";
import type { ControlDesk } from "./desk";

export interface Cue {
	id?: string;
	cue_only?: boolean;
	number: number;
	name: string;
	fade_millis: number;
	delay_millis: number;
	trigger: { type: string; [key: string]: unknown };
	changes: Array<{
		fixture_id: string;
		attribute: string;
		value: AttributeValue | null;
		automatic_restore?: boolean;
		fade_millis?: number;
		delay_millis?: number;
	}>;
	group_changes?: Array<{
		group_id: string;
		attribute: string;
		value: AttributeValue | null;
		automatic_restore?: boolean;
		fade_millis?: number;
		delay_millis?: number;
	}>;
	phasers?: unknown[];
}

export type AttributeValue =
	| { kind: "normalized"; value: number }
	| { kind: "spread"; value: number[] }
	| { kind: "discrete"; value: string }
	| { kind: "color_xyz"; value: { x: number; y: number; z: number } }
	| { kind: "raw_dmx"; value: number }
	| { kind: "raw_dmx_exact"; value: number };

export interface VisualizationSnapshot {
	revision: number;
	generated_at: string;
	grand_master: number;
	blackout: boolean;
	preload?: boolean;
	values: Array<{
		fixture_id: string;
		attribute: string;
		value: AttributeValue;
	}>;
	/** Post-profile values projected through the same calibrated/channel/master path as DMX. */
	profile_output_values?: Array<{
		fixture_id: string;
		attribute: string;
		value: AttributeValue;
	}>;
}

export interface CueList {
	id: string;
	name: string;
	cues: Cue[];
	mode: "sequence" | "chaser";
	priority: number;
	looped: boolean;
	intensity_priority_mode?: "htp" | "ltp";
	wrap_mode?: "off" | "tracking" | "reset";
	restart_mode?: "first_cue" | "continue_current_cue";
	force_cue_timing?: boolean;
	disable_cue_timing?: boolean;
	chaser_step_millis?: number;
	chaser_xfade_millis?: number;
	chaser_xfade_percent?: number;
	speed_group?: "A" | "B" | "C" | "D" | "E" | null;
	speed_multiplier?: number;
}

export interface PlaybackSnapshot {
	cue_lists: CueList[];
	pool: PlaybackDefinition[];
	pages: PlaybackPage[];
	active: Array<{
		playback_number?: number | null;
		cue_list_id: string;
		cue_index: number;
		previous_index?: number | null;
		paused: boolean;
		activated_at?: string;
		master: number;
		fader_position?: number;
		fader_pickup_required?: boolean;
		flash: boolean;
		transition_timing_bypassed?: boolean;
		manual_xfade_position?: number;
		manual_xfade_direction?: "towards_high" | "towards_low";
		manual_xfade_progress?: number;
		temporary_active?: boolean;
		temporary_master?: number;
		swap_active?: boolean;
		enabled?: boolean;
		current_cue_number?: number | null;
		loaded_cue_number?: number | null;
		normal_next_cue_number?: number | null;
		effective_next_cue_number?: number | null;
		effective_next_is_loaded?: boolean;
	}>;
	desk: ControlDesk;
	active_page: number;
	selected_playback?: number | null;
	authoritative_controls?: {
		speed_groups: SpeedSnapshot[];
		groups: Array<{ id: string; master: number; flash_level: number }>;
		grand_master: {
			level: number;
			blackout: boolean;
			flash_active: boolean;
			dynamics_paused: boolean;
		};
		programmer_fade_millis: number;
		cue_fade_millis: number;
	};
}

export type PlaybackButtonAction =
	| "on"
	| "off"
	| "toggle"
	| "go"
	| "go_minus"
	| "fast_forward"
	| "fast_rewind"
	| "flash"
	| "temp"
	| "swap"
	| "select"
	| "select_contents"
	| "select_dereferenced"
	| "learn"
	| "double"
	| "half"
	| "pause"
	| "blackout"
	| "pause_dynamics"
	| "none";

export interface PlaybackDefinition {
	number: number;
	name: string;
	target:
		| { type: "cue_list"; cue_list_id: string }
		| { type: "group"; group_id: string }
		| { type: "speed_group"; group: string }
		| { type: "programmer_fade" }
		| { type: "cue_fade" }
		| { type: "grand_master" };
	buttons: [PlaybackButtonAction, PlaybackButtonAction, PlaybackButtonAction];
	/** Missing only on legacy show files; every save writes an explicit topology. */
	button_count?: 0 | 1 | 2 | 3;
	fader:
		| "master"
		| "temp"
		| "speed"
		| "x_fade"
		| "direct_bpm"
		| "centered_relative"
		| "learned_percentage";
	/** Missing only on legacy show files; every save writes an explicit topology. */
	has_fader?: boolean;
	go_activates: boolean;
	auto_off: boolean;
	xfade_millis: number;
	color?: string;
	flash_release?: "release_all" | "release_intensity_only";
	protect_from_swap?: boolean;
	presentation_icon?: string;
	presentation_image?: string;
}

export interface PlaybackPage {
	number: number;
	name: string;
	slots: Record<string, number>;
}

export interface VirtualPlaybackExclusionZone {
	id: string;
	name: string;
	slots: number[];
}

export interface VirtualPlaybackExclusionSnapshot {
	show_id: string;
	desk_id: string;
	surfaces: Record<string, VirtualPlaybackExclusionZone[]>;
}
