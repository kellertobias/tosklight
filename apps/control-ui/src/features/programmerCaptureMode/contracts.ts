/** User-owned Programmer capture switches that determine where value writes land. */
export interface ProgrammerCaptureModeProjection {
	userId: string;
	revision: number;
	blind: boolean;
	preview: boolean;
	preloadCaptureProgrammer: boolean;
}

export interface ProgrammerCaptureModeSnapshot {
	cursor: number;
	projection: ProgrammerCaptureModeProjection;
}

export interface ProgrammerCaptureModeScope {
	showId: string;
	userId: string;
}

export type ProgrammerCaptureModeEventMessage =
	| { type: "ready"; cursor: number }
	| {
			type: "event";
			sequence: number;
			correlationId: string | null;
			projection: ProgrammerCaptureModeProjection;
	  }
	| {
			type: "gap";
			afterSequence: number;
			oldestAvailable: number;
			latestSequence: number;
	  }
	| { type: "repaired"; cursor: number }
	| { type: "error"; error: string };

export function capturesProgrammerWrites(
	projection: ProgrammerCaptureModeProjection | null,
) {
	return Boolean(projection?.blind && projection.preloadCaptureProgrammer);
}
