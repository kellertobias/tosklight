import type { OutputRuntimeProjection } from "./contracts";

export type OutputRuntimeStatus = "idle" | "loading" | "ready" | "error";
export type OutputRuntimeSettlement = "settled" | "ignored" | "repair";

export interface OutputRuntimeState {
	showId: string | null;
	deskId: string | null;
	authorityKey: string | null;
	eventSequence: number | null;
	authorityRevision: number | null;
	projection: OutputRuntimeProjection | null;
	status: OutputRuntimeStatus;
	error: Error | null;
	repairRequired: boolean;
	pendingRequestIds: readonly string[];
}

export function emptyOutputRuntimeState(): OutputRuntimeState {
	return {
		showId: null,
		deskId: null,
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
