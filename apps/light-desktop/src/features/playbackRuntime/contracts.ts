import type {
	PlaybackActionOutcome,
	PlaybackDeskProjection,
	PlaybackRuntimeIdentity,
	PlaybackRuntimeProjection,
	PlaybackRuntimeSnapshot,
	PlaybackTelemetrySample,
	PlaybackTelemetryTick,
} from "../../api/types";
import type { PlaybackCueTransition } from "../../api/types/playback";

export type PlaybackIdentity = PlaybackRuntimeIdentity;
export type PlaybackProjection = PlaybackRuntimeProjection;
export type PlaybackDesk = PlaybackDeskProjection;
export type PlaybackSnapshot = PlaybackRuntimeSnapshot;
export type PlaybackOutcome = PlaybackActionOutcome;
export type PlaybackTelemetry = PlaybackTelemetrySample;
export type PlaybackTelemetryDelta = PlaybackTelemetryTick;

export type PlaybackRuntimeEventMessage =
	| { type: "ready"; cursor: number }
	| {
			type: "event";
			sequence: number;
			payload:
				| {
						type: "runtime";
						projection: PlaybackProjection;
						transition?: PlaybackCueTransition | null;
				  }
				| { type: "desk"; projection: PlaybackDesk }
				| { type: "telemetry"; tick: PlaybackTelemetryTick };
	  }
	| {
			type: "gap";
			afterSequence: number;
			oldestAvailable: number;
			latestSequence: number;
	  }
	| { type: "repaired"; cursor: number }
	| { type: "error"; error: string };

export function playbackIdentity(page: number, position: number): PlaybackIdentity {
	return { kind: "playback", page, position };
}

export function cueListIdentity(cueListId: string): PlaybackIdentity {
	return { kind: "cue_list", cue_list_id: cueListId };
}

export function groupIdentity(groupId: string): PlaybackIdentity {
	return { kind: "group", group_id: groupId };
}

export function identityKey(identity: PlaybackIdentity) {
	if (identity.kind === "playback")
		return `playback:${identity.page}.${identity.position}`;
	if (identity.kind === "cue_list") return `cuelist:${identity.cue_list_id}`;
	if (identity.kind === "direct_cue_list")
		return `direct-cuelist:${identity.cue_list_id}`;
	return `group:${identity.group_id}`;
}

export function projectionKeys(projection: PlaybackProjection) {
	return [identityKey(projection.identity)];
}
