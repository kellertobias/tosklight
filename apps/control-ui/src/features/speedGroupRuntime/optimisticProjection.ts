import type {
	SpeedGroupAction,
	SpeedGroupAuthorityProjection,
	SpeedGroupId,
	SpeedGroupProjection,
} from "./contracts";
import { sameGroup } from "./projectionValue";

export interface OptimisticSpeedGroupMutation {
	requestId: string;
	action: SpeedGroupAction;
}

export function renderOptimisticSpeedGroups(
	authoritative: SpeedGroupAuthorityProjection | null,
	operations: Iterable<OptimisticSpeedGroupMutation>,
) {
	let projection = authoritative;
	if (!projection) return null;
	for (const operation of operations)
		projection = applyOptimisticAction(projection, operation.action);
	return projection;
}

export function applyOptimisticAction(
	projection: SpeedGroupAuthorityProjection,
	action: SpeedGroupAction,
	appliedAtMillis?: number,
) {
	const groups = projection.groups.map((group) => ({ ...group }));
	if (action.type === "synchronize")
		applySynchronization(groups, action.source, action.target);
	else
		applyManual(
			groups,
			action.group,
			action.type === "set_bpm"
				? action.bpm
				: groups[indexOf(action.group)].manualBpm + action.deltaBpm,
		);
	if (appliedAtMillis !== undefined && action.type !== "synchronize")
		groups[indexOf(action.group)].phaseOriginMillis = appliedAtMillis;
	if (
		groups.every((group, index) => sameGroup(group, projection.groups[index]))
	)
		return projection;
	return { ...projection, groups };
}

function applyManual(
	groups: SpeedGroupProjection[],
	group: SpeedGroupId,
	bpm: number,
) {
	breakReciprocalPair(groups, group);
	const selected = groups[indexOf(group)];
	selected.manualBpm = bpm;
	selected.paused = false;
	selected.speedMasterScale = 1;
	selected.synchronizedWith = null;
}

function applySynchronization(
	groups: SpeedGroupProjection[],
	source: SpeedGroupId,
	target: SpeedGroupId,
) {
	breakReciprocalPair(groups, source);
	breakReciprocalPair(groups, target);
	const sourceValue = { ...groups[indexOf(source)] };
	for (const [group, peer] of [
		[source, target],
		[target, source],
	] as const) {
		const selected = groups[indexOf(group)];
		selected.manualBpm = sourceValue.manualBpm;
		selected.paused = sourceValue.paused;
		selected.speedMasterScale = 1;
		selected.synchronizedWith = peer;
		selected.phaseOriginMillis = sourceValue.phaseOriginMillis;
	}
}

function breakReciprocalPair(
	groups: SpeedGroupProjection[],
	group: SpeedGroupId,
) {
	const selected = groups[indexOf(group)];
	const peer = selected.synchronizedWith;
	if (peer && groups[indexOf(peer)].synchronizedWith === group)
		groups[indexOf(peer)].synchronizedWith = null;
	selected.synchronizedWith = null;
}

function indexOf(group: SpeedGroupId) {
	return group.charCodeAt(0) - 65;
}
