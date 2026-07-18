export interface DeskUser {
	id: string;
	name: string;
	enabled: boolean;
}

export interface ShowEntry {
	id: string;
	name: string;
	path: string;
	revision: number;
	updated_at: string;
	revision_copy?: RevisionCopySource;
}

export interface RevisionCopySource {
	show_id: string;
	show_name: string;
	revision: number;
	revision_name: string;
	copied_at: string;
}

export interface OutputHealth {
	frames_sent: number;
	packets_sent: number;
	send_errors: number;
	deadline_misses: number;
	maximum_lateness_micros: number;
	frame_hz: number;
	last_tick_micros: number;
	maximum_tick_micros: number;
	scheduler_utilization: number;
}

export interface ProgrammerState {
	session_id: string;
	user_id: string;
	selected: string[];
	command_line: string;
	connected: boolean;
	blind: boolean;
	preload_capture_programmer?: boolean;
	preview: boolean;
	highlight: boolean;
	values: unknown[];
	preload_pending?: Array<{
		fixture_id: string;
		attribute: string;
		value: unknown;
		changed_at: string;
		fade_millis?: number;
		delay_millis?: number;
	}>;
	preload_active?: unknown[];
	preload_group_pending?: Record<
		string,
		Record<
			string,
			{
				value: unknown;
				changed_at: string;
				fade_millis?: number;
				delay_millis?: number;
			}
		>
	>;
	preload_group_active?: Record<string, Record<string, unknown>>;
	preload_playback_pending?: Array<{
		playback_number: number;
		action:
			| "toggle"
			| "go"
			| "go-minus"
			| "off"
			| "on"
			| "temp-on"
			| "temp-off";
		surface: "physical" | "virtual" | string;
	}>;
	group_values?: Record<
		string,
		Record<
			string,
			{
				value: unknown;
				changed_at: string;
				fade?: boolean;
				fade_millis?: number;
				delay_millis?: number;
			}
		>
	>;
	selection_expression?: {
		type: string;
		group_id?: string;
		source_revision?: number;
		rule?: { type: string; n?: number; offset?: number };
	};
}

export type HighlightAction =
	| "on"
	| "off"
	| "toggle"
	| "next"
	| "previous"
	| "all";

export interface HighlightFixtureSummary {
	fixture_id: string;
	number?: number | null;
	/** Accepted for compatibility with patch-shaped fixture summaries. */
	fixture_number?: number | null;
	name?: string | null;
}

/** Authoritative, transient Highlight state for the current desk and user. */
export interface HighlightState {
	active: boolean;
	mode: "selection" | "step";
	output_enabled: boolean;
	capture_only: boolean;
	remembered: HighlightFixtureSummary[];
	active_index: number | null;
	active_fixture: HighlightFixtureSummary | null;
	can_previous: boolean;
	can_next: boolean;
	owner_user_id: string | null;
	owner_user_name?: string | null;
	message?: string | null;
}

export interface BootstrapSnapshot {
	api_version: string;
	attribute_registry: AttributeDescriptor[];
	users: DeskUser[];
	desks: ControlDesk[];
	clients: ClientSummary[];
	active_show: ShowEntry | null;
	active_programmers: ProgrammerState[];
	frame_rate_hz: number;
	output_health: OutputHealth;
	active_timecode_source: string | null;
	active_timecode: string | null;
	active_show_error: string | null;
	hardware_connected: boolean;
}

export interface AttributeDescriptor {
	id: string;
	label: string;
	family:
		| "intensity"
		| "position"
		| "color"
		| "beam"
		| "focus"
		| "control"
		| "custom";
	value_type: "continuous" | "color" | "indexed" | "control";
	default_unit: string | null;
}

export interface SessionResponse {
	session_id: string;
	client_id: string;
	token: string;
	user: DeskUser;
	desk: ControlDesk;
}

export interface ClientSummary {
	client_id: string;
	name: string;
	connected: boolean;
	last_connected_at: string | null;
	desk: ControlDesk;
	can_remove: boolean;
}

export interface DeskLockState {
	locked: boolean;
	message: string;
	wallpaper: string | null;
	unlock_mode: "button" | "pin";
}

export interface ControlDesk {
	id: string;
	name: string;
	osc_alias: string;
	columns: number;
	rows: number;
	buttons: number;
	playback_layout?: PlaybackSurfaceLayout | null;
}

export interface PlaybackSurfaceRow {
	first_playback_slot: number;
	has_fader: boolean;
	button_count: number;
}

export interface PlaybackSurfaceLayout {
	playbacks_per_row: number;
	rows: PlaybackSurfaceRow[];
}

export interface ScreenConfiguration {
	id: string;
	name: string;
	layout: { desks: import("../../types").DeskModel[]; activeDeskId: string };
	show_dock: boolean;
	show_playbacks: boolean;
	playback_count: number;
	playback_rows: number;
	first_playback_slot: number;
	page_mode: "follow_main" | "independent";
	show_page_controls: boolean;
	desired_open: boolean;
	display_id: string | null;
	bounds: { x: number; y: number; width: number; height: number } | null;
	fullscreen: boolean;
	playback_layout?: PlaybackSurfaceLayout | null;
}

export interface ScreenSnapshot {
	screens: ScreenConfiguration[];
	active_pages: Record<string, number>;
}
