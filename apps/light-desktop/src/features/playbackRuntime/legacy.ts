import type { PlaybackSnapshot } from "../../api/types";
import type { PlaybackProjection } from "./contracts";

export type LegacyPlaybackRuntime = PlaybackSnapshot["active"][number];

export function legacyPlaybackRuntime(
	projection: PlaybackProjection | null | undefined,
): LegacyPlaybackRuntime | undefined {
	if (projection?.target === "group") {
		return {
			playback_number: projection.playback_number,
			cue_list_id: "",
			cue_index: -1,
			previous_index: null,
			paused: false,
			master: projection.master,
			fader_position: projection.fader_position,
			fader_pickup_required: projection.fader_pickup_required,
			fader_pickup_target: projection.fader_pickup_target,
			flash: projection.flash_level > 0,
			transition_timing_bypassed: false,
			manual_xfade_position: 0,
			manual_xfade_direction: undefined,
			manual_xfade_progress: 0,
			temporary_active: false,
			temporary_master: 0,
			swap_active: false,
			enabled: true,
			current_cue_number: null,
			loaded_cue_number: null,
			normal_next_cue_number: null,
			effective_next_cue_number: null,
			effective_next_is_loaded: false,
		};
	}
	if (projection?.target === "dynamic" && projection.runtime) {
		const runtime = projection.runtime;
		return {
			playback_number: projection.playback_number,
			cue_list_id: "",
			cue_index: -1,
			previous_index: null,
			paused: runtime.paused,
			activated_at: runtime.activated_at,
			master: runtime.master,
			fader_position: runtime.fader_value,
			fader_pickup_required: runtime.fader_pickup_required,
			fader_pickup_target: runtime.fader_pickup_target,
			flash: runtime.flash,
			transition_timing_bypassed: false,
			manual_xfade_position: 0,
			manual_xfade_direction: undefined,
			manual_xfade_progress: 0,
			temporary_active: false,
			temporary_master: 0,
			swap_active: false,
			enabled: runtime.enabled,
			current_cue_number: null,
			loaded_cue_number: null,
			normal_next_cue_number: null,
			effective_next_cue_number: null,
			effective_next_is_loaded: false,
		};
	}
	if (projection?.target !== "cue_list" || !projection.runtime)
		return undefined;
	const runtime = projection.runtime;
	return {
		playback_number: projection.playback_number,
		cue_list_id: projection.cue_list_id,
		cue_index: runtime.cue_index,
		previous_index: runtime.previous_index,
		paused: runtime.paused,
		activated_at: runtime.activated_at,
		paused_at: runtime.paused_at,
		cue_timing: runtime.cue_timing,
		transition_ordinal: runtime.transition_ordinal,
		master: runtime.master,
		fader_position: runtime.fader_position,
		fader_pickup_required: runtime.fader_pickup_required,
		fader_pickup_target: runtime.fader_pickup_target,
		flash: runtime.flash,
		transition_timing_bypassed: runtime.transition_timing_bypassed,
		manual_xfade_position: runtime.manual_xfade_position,
		manual_xfade_direction: runtime.manual_xfade_direction,
		manual_xfade_progress: runtime.manual_xfade_progress,
		temporary_active: runtime.temporary_active,
		temporary_master: runtime.temporary_master,
		swap_active: runtime.swap_active,
		enabled: runtime.enabled,
		current_cue_number: runtime.current?.number ?? null,
		loaded_cue_number: runtime.loaded?.number ?? null,
		normal_next_cue_number: runtime.normal_next?.number ?? null,
		effective_next_cue_number: runtime.effective_next?.number ?? null,
		effective_next_is_loaded: runtime.effective_next_is_loaded,
		deleted_cue_hold: runtime.deleted_cue_hold,
	};
}

export function runtimeMaster(projection: PlaybackProjection | undefined) {
	if (projection?.target === "cue_list")
		return projection.runtime?.master ?? null;
	if (projection?.target === "group") return projection.master;
	if (projection?.target === "grand_master") return projection.runtime.level;
	if (projection?.target === "dynamic") return projection.runtime?.master ?? null;
	return null;
}
