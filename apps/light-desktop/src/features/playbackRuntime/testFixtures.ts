import type {
	PlaybackDesk,
	PlaybackIdentity,
	PlaybackProjection,
	PlaybackSnapshot,
} from "./contracts";

export const SHOW_ID = "11111111-1111-4111-8111-111111111111";
export const DESK_ID = "22222222-2222-4222-8222-222222222222";
export const CUE_LIST_ID = "33333333-3333-4333-8333-333333333333";
export const GROUP_ID = "front wash";

export function cueProjection(
	playbackNumber = 1,
	cueIndex = 0,
): PlaybackProjection {
	return {
		scope: { show_id: SHOW_ID, show_revision: 4 },
		requested: { kind: "playback", playback_number: playbackNumber },
		playback_number: playbackNumber,
		target: "cue_list",
		cue_list_id: CUE_LIST_ID,
		runtime: {
			cue_index: cueIndex,
			previous_index: null,
			current: {
				id: "44444444-4444-4444-8444-444444444444",
				number: cueIndex + 1,
			},
			loaded: null,
			normal_next: null,
			effective_next: null,
			effective_next_is_loaded: false,
			paused: false,
			activated_at: "2026-07-19T10:00:00Z",
			master: 1,
			fader_position: 1,
			fader_pickup_required: false,
			flash: false,
			temporary: false,
			temporary_active: false,
			temporary_master: 0,
			swap_active: false,
			enabled: true,
			transition_timing_bypassed: false,
			manual_xfade_position: 0,
			manual_xfade_direction: "towards_high",
			manual_xfade_progress: 0,
		},
	};
}

export function deskProjection(activePage = 1): PlaybackDesk {
	return {
		scope: { show_id: SHOW_ID, show_revision: 4 },
		desk_id: DESK_ID,
		active_page: activePage,
		selected_playback: null,
	};
}

export function groupProjection(
	groupId = GROUP_ID,
	master = 1,
	playbackNumber: number | null = null,
): PlaybackProjection {
	return {
		scope: { show_id: SHOW_ID, show_revision: 4 },
		requested: { kind: "group", group_id: groupId },
		playback_number: playbackNumber,
		target: "group",
		group_id: groupId,
		master,
		flash_level: 0,
	};
}

export function playbackSnapshot(
	identities: readonly PlaybackIdentity[],
	cursor = 10,
	projections = identities.map((identity) =>
		identity.kind === "playback"
			? cueProjection(identity.playback_number)
			: identity.kind === "cue_list"
				? {
					...cueProjection(1),
					requested: identity,
					playback_number: null,
				}
				: groupProjection(identity.group_id),
	),
): PlaybackSnapshot {
	return {
		cursor: { sequence: cursor },
		desk: deskProjection(),
		projections,
	};
}

export function runtimeEvent(projection = cueProjection(), sequence = 11) {
	const playbackNumber = projection.playback_number ?? 1;
	return {
		type: "event",
		event: {
			sequence,
			occurred_at: "2026-07-19T10:00:01Z",
			desk_id: null,
			class: "transition",
			object: { capability: "playback", id: `playback:${playbackNumber}` },
			related_objects: [
				{ capability: "playback", id: `cuelist:${CUE_LIST_ID}` },
			],
			source: { kind: "runtime" },
			correlation_id: null,
			delivery: "lossless",
			payload: {
				type: "playback_runtime_changed",
				change: { projection, transition: null },
			},
		},
	};
}
