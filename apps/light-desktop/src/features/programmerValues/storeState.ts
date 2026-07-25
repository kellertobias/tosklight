import type { ProgrammerValuesProjection } from "./contracts";

export type ProgrammerValuesStatus = "idle" | "loading" | "ready" | "error";

export interface ProgrammerValuesState {
	showId: string | null;
	userId: string | null;
	eventSequence: number | null;
	projection: ProgrammerValuesProjection | null;
	pendingRequestIds: readonly string[];
	status: ProgrammerValuesStatus;
	error: Error | null;
	repairRequired: boolean;
}

export type ProgrammerValuesOptimisticReducer = (
	current: ProgrammerValuesProjection,
) => ProgrammerValuesProjection;

export type ProgrammerValuesSettlement = "settled" | "repair" | "ignored";

const EMPTY_REQUEST_IDS = Object.freeze([]) as readonly string[];

export function emptyProgrammerValuesState(): ProgrammerValuesState {
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
