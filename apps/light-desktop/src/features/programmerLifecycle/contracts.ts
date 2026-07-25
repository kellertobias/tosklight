export interface ProgrammerLifecycleSessionProjection {
	sessionId: string;
}

/** Lightweight installation-wide Programmer activity for one user. */
export interface ProgrammerLifecycleRow {
	programmerId: string;
	userId: string;
	connected: boolean;
	selectedFixtureCount: number;
	normalValueCount: number;
	preloadActive: boolean;
	sessions: readonly ProgrammerLifecycleSessionProjection[];
}

export interface ProgrammerLifecycleProjection {
	revision: number;
	programmers: readonly ProgrammerLifecycleRow[];
}

export interface ProgrammerLifecycleSnapshot {
	cursor: number;
	projection: ProgrammerLifecycleProjection;
}

export type ProgrammerLifecycleDelta =
	| {
			type: "upsert";
			programmer: ProgrammerLifecycleRow;
	  }
	| {
			type: "remove";
			programmerId: string;
	  };

export interface ProgrammerLifecycleChange {
	revision: number;
	delta: ProgrammerLifecycleDelta;
}

export type ProgrammerLifecycleEventMessage =
	| { type: "ready"; cursor: number }
	| {
			type: "event";
			sequence: number;
			correlationId: string | null;
			change: ProgrammerLifecycleChange;
	  }
	| {
			type: "gap";
			afterSequence: number;
			oldestAvailable: number;
			latestSequence: number;
	  }
	| { type: "repaired"; cursor: number }
	| { type: "error"; error: string };
