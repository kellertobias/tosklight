import type { PlaybackProjection } from "./contracts";
import type { PlaybackRuntimeState } from "./store";

export interface GroupProjectionSelection {
	ready: boolean;
	projections: ReadonlyMap<string, PlaybackProjection | undefined>;
}

export function selectGroupProjections(
	state: PlaybackRuntimeState,
	groupIds: readonly string[],
): GroupProjectionSelection {
	let ready = true;
	const projections = new Map<string, PlaybackProjection | undefined>();
	for (const groupId of groupIds) {
		const candidates = state.projections.get(`group:${groupId}`);
		const projection = candidates?.find(
			(candidate) =>
				candidate.target === "group" && candidate.group_id === groupId,
		);
		if (!projection) ready = false;
		projections.set(groupId, projection);
	}
	return { ready, projections };
}

export function equalGroupProjectionSelection(
	left: GroupProjectionSelection,
	right: GroupProjectionSelection,
) {
	return (
		left.ready === right.ready &&
		left.projections.size === right.projections.size &&
		[...left.projections].every(
			([key, value]) => right.projections.get(key) === value,
		)
	);
}
