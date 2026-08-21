import type { TimecodeCueListClipExecution as WireTimecodeCueListClipExecution } from "../generated/light-wire";

export interface TimecodeDefinition {
	id: string;
	number: number;
	name: string;
	duration_frame?: number | null;
	transport_offset_frame: number;
	auto_start: boolean;
	audio?: TimecodeAudio | null;
	markers: TimecodeMarker[];
	lanes: TimecodeLane[];
}

export interface TimecodeAudio {
	asset_id: string;
	asset_revision: number;
	end_fade_frames?: number | null;
}

export interface TimecodeMarker {
	id: string;
	frame: number;
	name: string;
	color?: string | null;
}

export interface TimecodeLane {
	id: string;
	name: string;
	content: TimecodeLaneContent;
}

export type TimecodeLaneContent =
	| { kind: "cue_list"; cue_list_id: string; clips: TimecodeCueListClip[] }
	| { kind: "speed_group"; group: string; keyframes: TimecodeSpeedKeyframe[] }
	| { kind: "audio_volume"; keyframes: TimecodeVolumeKeyframe[] }
	| {
			kind: "audio_player";
			fixture_id: string;
			clips: TimecodeAudioPlayerClip[];
	  };

export interface TimecodeAudioPlayerClip {
	id: string;
	start_frame: number;
	end_frame: number;
	folder: number;
	file: number;
	repeat: boolean;
	volume_keyframes: TimecodeVolumeKeyframe[];
}

export interface TimecodeCueListClip {
	id: string;
	start_frame: number;
	end_frame: number;
	start_cue_id: string;
	end_cue_id: string;
	start_behavior: "state" | "cue";
	end_behavior: "release" | "hold";
	/// Transition points placed in the lane for Cues that wait for a manual GO.
	cue_starts: TimecodeCueStart[];
}

export interface TimecodeCueStart {
	cue_id: string;
	offset_frame: number;
}

export interface TimecodeSpeedKeyframe {
	id: string;
	frame: number;
	bpm: number;
	phase: number;
}

export interface TimecodeVolumeKeyframe {
	id: string;
	frame: number;
	value: number;
	fade_frames: number;
	curve: "linear" | "ease_in" | "ease_out" | "ease_in_out";
}

export interface TimecodeObjectRecord {
	revision: number;
	definition: TimecodeDefinition;
}

export interface TimecodeCollectionSnapshot {
	show_revision: number;
	objects: TimecodeObjectRecord[];
}

export type TimecodePatch = Partial<
	Pick<
		TimecodeDefinition,
		| "number"
		| "name"
		| "duration_frame"
		| "transport_offset_frame"
		| "auto_start"
		| "audio"
		| "markers"
		| "lanes"
	>
>;

export type TimecodeObjectAction =
	| { type: "create"; definition: TimecodeDefinition }
	| {
			type: "update";
			timecode_id: string;
			expected_revision: number;
			patch: TimecodePatch;
	  }
	| { type: "delete"; timecode_id: string; expected_revision: number };

export type TimecodeTransportAction =
	| { type: "go" }
	| { type: "pause" }
	| { type: "stop" }
	| { type: "rewind" }
	| { type: "seek"; frame: number };

export interface TimecodeTransportSnapshot {
	timecode_id: string;
	revision: number;
	state: "stopped" | "playing" | "paused";
	frame: number;
	duration_frame: number;
	audio_linked: boolean;
	cue_list_clips: TimecodeCueListClipStatus[];
}

export type TimecodeCueListClipStatus = WireTimecodeCueListClipExecution;

export interface TimecodeAudioOutputDevices {
	devices: string[];
}

export interface TimecodeAudioImportResult {
	asset_id: string;
	asset_revision: number;
	name: string;
	media_type: string;
	sample_rate: number;
	channels: number;
	sample_frames: number;
}

export interface TimecodeAudioWaveform {
	peaks: number[];
}
