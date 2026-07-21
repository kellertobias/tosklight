import type { SpeedGroupAuthorityProjection } from "./contracts";

export type SpeedGroupRuntimeStatus = "idle" | "loading" | "ready" | "error";
export type SpeedGroupSettlement = "settled" | "ignored" | "repair";

export interface SpeedGroupRuntimeState {
	deskId: string | null;
	authorityKey: string | null;
	eventSequence: number | null;
	authorityId: string | null;
	authorityRevision: number | null;
	projection: SpeedGroupAuthorityProjection | null;
	status: SpeedGroupRuntimeStatus;
	error: Error | null;
	repairRequired: boolean;
	pendingRequestIds: readonly string[];
}

export function emptySpeedGroupState(): SpeedGroupRuntimeState {
	return {
		deskId: null,
		authorityKey: null,
		eventSequence: null,
		authorityId: null,
		authorityRevision: null,
		projection: null,
		status: "idle",
		error: null,
		repairRequired: false,
		pendingRequestIds: [],
	};
}
