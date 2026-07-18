export interface DeskConfiguration {
	frame_rate_hz: number;
	output_bind_ip: string;
	osc_bind: string | null;
	art_timecode_bind: string | null;
	midi_inputs: string[];
	rtp_midi_bind: string | null;
	timecode_sources: Array<{
		source_prefix: string;
		priority: number;
		fallback: boolean;
		loss_timeout_millis: number;
	}>;
	osc_timecode: { address: string; rate: string } | null;
	backup_retention: number;
	speed_groups_bpm: [number, number, number, number, number];
	programmer_fade_millis: number;
	sequence_master_fade_millis: number;
	preload_programmer_changes: boolean;
	preload_physical_playback_actions: boolean;
	preload_virtual_playback_actions: boolean;
	patch_preview_highlight_dmx?: boolean;
	matter_enabled?: boolean;
	update_settings_by_desk?: Record<string, unknown>;
	file_manager_system_picker_fallback: boolean;
	file_manager_roots: Array<{
		id: string;
		label: string;
		path: string;
		icon?: string;
	}>;
}

export interface MatterPairingData {
	qr_code: string;
	manual_code: string;
	discriminator: number;
}

export interface MatterPlaybackLight {
	endpoint_id: number;
	page: number;
	playback: number;
	playback_number: number;
	name: string;
	on: boolean;
	level: number;
}

export interface MatterBridgeStatus {
	enabled: boolean;
	transport: "disabled" | "adapter_ready" | "starting" | "running" | "failed";
	commissionable: boolean;
	network_running: boolean;
	commissioned: boolean;
	commissioning_window_open: boolean;
	pairing?: MatterPairingData | null;
	revision: number;
	lights: MatterPlaybackLight[];
	limitation?: string | null;
}

export type SpeedGroupId = "A" | "B" | "C" | "D" | "E";

export type FrequencyPreset = "sub" | "low" | "mid" | "high" | "full_range";

export type FrequencySelection =
	| { type: "preset"; preset: FrequencyPreset }
	| { type: "custom"; low_hz: number; high_hz: number };

export interface SoundToLightConfig {
	enabled: boolean;
	analysis_mode: "tempo_bpm";
	frequency: FrequencySelection;
	input_gain_db: number;
	confidence_threshold: number;
	smoothing: number;
	minimum_bpm: number;
	maximum_bpm: number;
	signal_hold_millis: number;
	multiplier: number;
}

export interface SoundObservation {
	captured_at_millis: number;
	source_available: boolean;
	usable_signal: boolean;
	level: number;
	selected_band_level: number;
	detected_bpm: number | null;
	confidence: number;
}

export type SoundLossReason =
	| "source_unavailable"
	| "no_usable_signal"
	| "low_confidence"
	| "tempo_outside_range"
	| "waiting_for_analysis";

export type SoundStatus =
	| { state: "disabled" }
	| { state: "active"; detected_bpm: number; confidence: number }
	| { state: "holding"; reason: SoundLossReason; remaining_millis: number }
	| { state: "manual_fallback"; reason: SoundLossReason };

export interface SpeedSnapshot {
	manual_bpm: number;
	sound_bpm: number | null;
	effective_bpm: number;
	source: "manual" | "sound" | "held_sound" | "manual_fallback";
	sound_status: SoundStatus;
	paused: boolean;
	phase_advancing: boolean;
	speed_master_scale: number;
	sound_multiplier: number;
	source_available: boolean;
	usable_signal: boolean;
	input_level: number;
	selected_band_level: number;
}

export interface SpeedGroupSoundState {
	group: SpeedGroupId;
	configuration: SoundToLightConfig;
	snapshot: SpeedSnapshot;
}

export type SpeedGroupAction = "learn" | "double" | "half" | "pause";

export interface SpeedGroupActionInput {
	action: SpeedGroupAction;
	captured_at_millis?: number;
}
