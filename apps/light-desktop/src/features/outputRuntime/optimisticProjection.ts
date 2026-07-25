import type { OutputRuntimeProjection } from "./contracts";

export interface OptimisticOutputMutation {
	requestId: string;
	grandMaster?: number;
	blackout?: boolean;
}

export function renderOptimisticOutput(
	authoritative: OutputRuntimeProjection | null,
	operations: Iterable<OptimisticOutputMutation>,
) {
	let projection = authoritative;
	if (!projection) return null;
	for (const operation of operations) {
		const grandMaster: number = operation.grandMaster ?? projection.grandMaster;
		const blackout: boolean = operation.blackout ?? projection.blackout;
		if (
			grandMaster !== projection.grandMaster ||
			blackout !== projection.blackout
		)
			projection = { ...projection, grandMaster, blackout };
	}
	return projection;
}
