import type { ProgrammerPriorityProjection } from "./contracts";

export interface OptimisticPriority {
	requestId: string;
	priority: number;
}

export function renderOptimisticPriority(
	authoritative: ProgrammerPriorityProjection | null,
	operations: Iterable<OptimisticPriority>,
) {
	let projection = authoritative;
	if (projection)
		for (const operation of operations)
			if (operation.priority !== projection.priority)
				projection = { ...projection, priority: operation.priority };
	return projection;
}
