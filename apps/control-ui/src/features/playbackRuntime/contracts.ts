import type {
	PlaybackActionOutcome,
	PlaybackDeskProjection,
	PlaybackRuntimeIdentity,
	PlaybackRuntimeProjection,
	PlaybackRuntimeSnapshot,
} from "../../api/types";

export type PlaybackIdentity = PlaybackRuntimeIdentity;
export type PlaybackProjection = PlaybackRuntimeProjection;
export type PlaybackDesk = PlaybackDeskProjection;
export type PlaybackSnapshot = PlaybackRuntimeSnapshot;
export type PlaybackOutcome = PlaybackActionOutcome;

export type PlaybackRuntimeEventMessage =
	| { type: "ready"; cursor: number }
	| {
			type: "event";
			sequence: number;
			payload:
				| { type: "runtime"; projection: PlaybackProjection }
				| { type: "desk"; projection: PlaybackDesk };
	  }
	| {
			type: "gap";
			afterSequence: number;
			oldestAvailable: number;
			latestSequence: number;
	  }
	| { type: "repaired"; cursor: number }
	| { type: "error"; error: string };

export function playbackIdentity(playbackNumber: number): PlaybackIdentity {
	return { kind: "playback", playback_number: playbackNumber };
}

export function cueListIdentity(cueListId: string): PlaybackIdentity {
	return { kind: "cue_list", cue_list_id: cueListId };
}

export function groupIdentity(groupId: string): PlaybackIdentity {
	return { kind: "group", group_id: groupId };
}

export function identityKey(identity: PlaybackIdentity) {
	if (identity.kind === "playback")
		return `playback:${identity.playback_number}`;
	if (identity.kind === "cue_list") return `cuelist:${identity.cue_list_id}`;
	return `group:${identity.group_id}`;
}

export function projectionKeys(projection: PlaybackProjection) {
	const keys = new Set([identityKey(projection.requested)]);
	if (projection.playback_number != null)
		keys.add(`playback:${projection.playback_number}`);
	if (projection.target === "cue_list")
		keys.add(`cuelist:${projection.cue_list_id}`);
	if (projection.target === "group") keys.add(`group:${projection.group_id}`);
	return [...keys];
}
