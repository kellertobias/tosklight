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

export function identityKey(identity: PlaybackIdentity) {
	return identity.kind === "playback"
		? `playback:${identity.playback_number}`
		: `cuelist:${identity.cue_list_id}`;
}

export function projectionKeys(projection: PlaybackProjection) {
	const keys = new Set([identityKey(projection.requested)]);
	if (projection.playback_number != null)
		keys.add(`playback:${projection.playback_number}`);
	if (projection.target === "cue_list")
		keys.add(`cuelist:${projection.cue_list_id}`);
	return [...keys];
}
