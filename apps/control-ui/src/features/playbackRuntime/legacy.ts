import type { PlaybackSnapshot } from "../../api/types";
import type { PlaybackProjection } from "./contracts";

export type LegacyPlaybackRuntime = PlaybackSnapshot["active"][number];

export function legacyPlaybackRuntime(
	projection: PlaybackProjection | null | undefined,
): LegacyPlaybackRuntime | undefined {
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
		master: runtime.master,
		fader_position: runtime.fader_position,
		fader_pickup_required: runtime.fader_pickup_required,
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
	};
}

export function runtimeMaster(projection: PlaybackProjection | undefined) {
	if (projection?.target === "cue_list")
		return projection.runtime?.master ?? null;
	if (projection?.target === "group") return projection.master;
	if (projection?.target === "grand_master") return projection.runtime.level;
	return null;
}
