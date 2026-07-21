export interface OutputRuntimeScope {
	showId: string;
	deskId: string;
}

/** Installation-global Grand Master and blackout authority for one active Show. */
export interface OutputRuntimeProjection {
	showId: string;
	identity: "global_master";
	revision: number;
	grandMaster: number;
	blackout: boolean;
}

export interface OutputRuntimeSnapshot {
	cursor: number;
	projection: OutputRuntimeProjection;
}

export interface OutputRuntimeChange {
	projection: OutputRuntimeProjection;
}

export type OutputRuntimeEventMessage =
	| { type: "ready"; cursor: number }
	| {
			type: "event";
			sequence: number;
			correlationId: string | null;
			change: OutputRuntimeChange;
	  }
	| {
			type: "gap";
			afterSequence: number;
			oldestAvailable: number;
			latestSequence: number;
	  }
	| { type: "repaired"; cursor: number }
	| { type: "error"; error: string };

export interface OutputRuntimeActionRequest {
	requestId: string;
	expectedShowId: string;
	expectedRevision: number;
	grandMaster?: number;
	blackout?: boolean;
}

export type OutputRuntimeDurability = "durable" | "persistence_pending";

interface OutputRuntimeOutcomeBase {
	requestId: string;
	correlationId: string;
	projection: OutputRuntimeProjection;
	replayed: boolean;
	durability: OutputRuntimeDurability;
	warning: string | null;
}

export type OutputRuntimeActionOutcome = OutputRuntimeOutcomeBase &
	(
		| { status: "changed"; eventSequence: number }
		| { status: "no_change"; eventSequence: null }
	);

export interface SetOutputRuntimeInput {
	requestId?: string;
	grandMaster?: number;
	blackout?: boolean;
}

export interface OutputRuntimeActions {
	setOutput(
		input: SetOutputRuntimeInput,
	): Promise<OutputRuntimeActionOutcome | null>;
}

export type OutputRuntimeErrorKind =
	| "invalid"
	| "unauthorized"
	| "forbidden"
	| "not_found"
	| "conflict"
	| "unavailable"
	| "internal";
