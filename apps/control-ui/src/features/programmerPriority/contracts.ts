export interface ProgrammerPriorityScope {
	userId: string;
}

/** User-owned priority metadata; normal Programmer values never enter this slice. */
export interface ProgrammerPriorityProjection {
	userId: string;
	revision: number;
	priority: number;
	changedAt: string;
}

export interface ProgrammerPrioritySnapshot {
	cursor: number;
	projection: ProgrammerPriorityProjection;
}

export type ProgrammerPriorityChange =
	| { type: "upsert"; projection: ProgrammerPriorityProjection }
	| { type: "remove"; userId: string; revision: number };

export type ProgrammerPriorityEventMessage =
	| { type: "ready"; cursor: number }
	| {
			type: "event";
			sequence: number;
			correlationId: string | null;
			change: ProgrammerPriorityChange;
	  }
	| {
			type: "gap";
			afterSequence: number;
			oldestAvailable: number;
			latestSequence: number;
	  }
	| { type: "repaired"; cursor: number }
	| { type: "error"; error: string };

export interface ProgrammerPriorityActionRequest {
	requestId: string;
	expectedRevision: number;
	priority: number;
}

interface ProgrammerPriorityOutcomeBase {
	requestId: string;
	correlationId: string;
	projection: ProgrammerPriorityProjection;
	replayed: boolean;
	warning: string | null;
}

export type ProgrammerPriorityActionOutcome = ProgrammerPriorityOutcomeBase &
	(
		| { status: "changed"; eventSequence: number }
		| { status: "no_change"; eventSequence: null }
	);

export interface SetProgrammerPriorityInput {
	priority: number;
	requestId?: string;
}

export interface ProgrammerPriorityActions {
	setPriority(
		input: SetProgrammerPriorityInput,
	): Promise<ProgrammerPriorityActionOutcome | null>;
}

export type ProgrammerPriorityErrorKind =
	| "invalid"
	| "unauthorized"
	| "forbidden"
	| "not_found"
	| "conflict"
	| "unavailable"
	| "internal";
