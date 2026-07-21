export const speedGroupIds = ["A", "B", "C", "D", "E"] as const;

export type SpeedGroupId = (typeof speedGroupIds)[number];

export interface SpeedGroupRuntimeScope {
	deskId: string;
}

export interface SpeedGroupProjection {
	group: SpeedGroupId;
	manualBpm: number;
	paused: boolean;
	speedMasterScale: number;
	synchronizedWith: SpeedGroupId | null;
	phaseOriginMillis: number;
}

export interface SpeedGroupAuthorityProjection {
	authorityId: string;
	revision: number;
	groups: readonly SpeedGroupProjection[];
}

export interface SpeedGroupSnapshot {
	cursor: number;
	projection: SpeedGroupAuthorityProjection;
}

export interface SpeedGroupChange {
	authorityId: string;
	revision: number;
	appliedAtMillis: number;
	groups: readonly SpeedGroupProjection[];
}

export type SpeedGroupEventMessage =
	| { type: "ready"; cursor: number }
	| {
			type: "event";
			sequence: number;
			correlationId: string | null;
			change: SpeedGroupChange;
	  }
	| {
			type: "gap";
			afterSequence: number;
			oldestAvailable: number;
			latestSequence: number;
	  }
	| { type: "repaired"; cursor: number }
	| { type: "error"; error: string };

export type SpeedGroupAction =
	| { type: "set_bpm"; group: SpeedGroupId; bpm: number }
	| { type: "adjust_bpm"; group: SpeedGroupId; deltaBpm: number }
	| { type: "synchronize"; source: SpeedGroupId; target: SpeedGroupId };

export interface SpeedGroupActionRequest {
	requestId: string;
	expectedAuthorityId: string;
	expectedRevision: number;
	/** Local decoder context captured with the revision; never serialized. */
	expectedGroups?: readonly SpeedGroupProjection[];
	action: SpeedGroupAction;
}

export type SpeedGroupDurability = "durable" | "persistence_pending";

interface SpeedGroupOutcomeBase {
	requestId: string;
	correlationId: string;
	authorityId: string;
	revision: number;
	appliedAtMillis: number;
	groups: readonly SpeedGroupProjection[];
	replayed: boolean;
	durability: SpeedGroupDurability;
	warning: string | null;
}

export type SpeedGroupActionOutcome = SpeedGroupOutcomeBase &
	(
		| { status: "changed"; eventSequence: number }
		| { status: "no_change"; eventSequence: null }
	);

export interface SpeedGroupRuntimeActions {
	setBpm(
		group: SpeedGroupId,
		bpm: number,
		requestId?: string,
	): Promise<SpeedGroupActionOutcome | null>;
	adjustBpm(
		group: SpeedGroupId,
		deltaBpm: number,
		requestId?: string,
	): Promise<SpeedGroupActionOutcome | null>;
	synchronize(
		source: SpeedGroupId,
		target: SpeedGroupId,
		requestId?: string,
	): Promise<SpeedGroupActionOutcome | null>;
}

export type SpeedGroupErrorKind =
	| "invalid"
	| "unauthorized"
	| "forbidden"
	| "not_found"
	| "conflict"
	| "unavailable"
	| "internal";
