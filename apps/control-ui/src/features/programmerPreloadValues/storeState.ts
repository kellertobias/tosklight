import type { ProgrammerPreloadValuesProjection } from "./contracts";

export type ProgrammerPreloadValuesStatus =
	| "idle"
	| "loading"
	| "ready"
	| "error";

export interface ProgrammerPreloadValuesState {
	showId: string | null;
	userId: string | null;
	eventSequence: number | null;
	projection: ProgrammerPreloadValuesProjection | null;
	pendingRequestIds: readonly string[];
	status: ProgrammerPreloadValuesStatus;
	error: Error | null;
	repairRequired: boolean;
}

export type ProgrammerPreloadValuesOptimisticReducer = (
	current: ProgrammerPreloadValuesProjection,
) => ProgrammerPreloadValuesProjection;

export type ProgrammerPreloadValuesSettlement =
	| "settled"
	| "repair"
	| "ignored";

const EMPTY_REQUEST_IDS = Object.freeze([]) as readonly string[];

export function emptyProgrammerPreloadValuesState(): ProgrammerPreloadValuesState {
	return {
		showId: null,
		userId: null,
		eventSequence: null,
		projection: null,
		pendingRequestIds: EMPTY_REQUEST_IDS,
		status: "idle",
		error: null,
		repairRequired: false,
	};
}
