import {
	PlaybackDesk,
	PlaybackIdentity,
	PlaybackProjection,
	PlaybackSnapshot,
	identityKey,
} from "./contracts";

export const SHOW_ID = "11111111-1111-4111-8111-111111111111";
export const DESK_ID = "22222222-2222-4222-8222-222222222222";
export const CUE_LIST_ID = "33333333-3333-4333-8333-333333333333";
export const GROUP_ID = "front wash";

export function cueProjection(
	playbackNumber = 1,
	cueIndex = 0,
	page = 1,
): PlaybackProjection {
	return {
		scope: { show_id: SHOW_ID, show_revision: 4 },
		identity: { kind: "playback", page, position: playbackNumber },
		target: "cue_list",
		cue_list_id: CUE_LIST_ID,
		runtime: {
			cue_index: cueIndex,
			previous_index: null,
			current: {
				id: "44444444-4444-4444-8444-444444444444",
				number: String(cueIndex + 1),
			},
			loaded: null,
			normal_next: null,
			effective_next: null,
			effective_next_is_loaded: false,
			deleted_cue_hold: null,
			paused: false,
			activated_at: "2026-07-19T10:00:00Z",
			paused_at: null,
			cue_timing: null,
			transition_ordinal: 1,
			master: 1,
			fader_position: 1,
			fader_pickup_required: false,
			fader_pickup_target: null,
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
		command_page: activePage,
		selected_surface_id: "main",
		resolved_pages: { main: activePage },
		selected_cue_list: null,
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
		identity: {kind:"group",group_id:groupId},
		target: "group",
		group_id: groupId,
		master,
		flash_level: 0,
		fader_position: 0,
		fader_pickup_required: false,
		fader_pickup_target: null,
	};
}

export function playbackSnapshot(
	identities: readonly PlaybackIdentity[],
	cursor = 10,
	projections = identities.map((identity) =>
		identity.kind === "group"
			? groupProjection(identity.group_id)
			: {
					...cueProjection(
						identity.kind === "playback" ? identity.position : 1,
						0,
						identity.kind === "playback" ? identity.page : 1,
					),
					identity,
				},
	),
): PlaybackSnapshot {
	return {
		cursor: { sequence: cursor },
		desk: deskProjection(),
		projections,
	};
}

export function runtimeEvent(projection = cueProjection(), sequence = 11) {
	const key = identityKey(projection.identity);
	return {
		type: "event",
		event: {
			sequence,
			occurred_at: "2026-07-19T10:00:01Z",
			desk_id: null,
			class: "transition",
			object: { capability: "playback", id: key },
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
