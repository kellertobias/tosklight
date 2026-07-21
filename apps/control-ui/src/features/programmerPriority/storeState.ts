import type { ProgrammerPriorityProjection } from "./contracts";

export type ProgrammerPriorityStatus = "idle" | "loading" | "ready" | "error";
export type ProgrammerPrioritySettlement = "settled" | "ignored" | "repair";

export interface ProgrammerPriorityState {
	userId: string | null;
	authorityKey: string | null;
	eventSequence: number | null;
	authorityRevision: number | null;
	projection: ProgrammerPriorityProjection | null;
	status: ProgrammerPriorityStatus;
	error: Error | null;
	repairRequired: boolean;
	pendingRequestIds: readonly string[];
}

export function emptyProgrammerPriorityState(): ProgrammerPriorityState {
	return {
		userId: null,
		authorityKey: null,
		eventSequence: null,
		authorityRevision: null,
		projection: null,
		status: "idle",
		error: null,
		repairRequired: false,
		pendingRequestIds: [],
	};
}
