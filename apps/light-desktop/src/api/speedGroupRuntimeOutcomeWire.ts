import {
	type SpeedGroupActionRequest,
	type SpeedGroupId,
	type SpeedGroupProjection,
	speedGroupIds,
} from "../features/speedGroupRuntime/contracts";
import { WireValidationError } from "./wireValidation";

/** Validates the complete authoritative subset affected by one Speed Group action. */
export function assertSpeedGroupOutcomeGroups(
	groups: readonly SpeedGroupProjection[],
	request: SpeedGroupActionRequest,
) {
	const expected = expectedOutcomeGroups(request);
	if (
		groups.length !== expected.length ||
		groups.some((group, index) => group.group !== expected[index])
	)
		throw new WireValidationError(
			"$.groups",
			`authoritative groups ${expected.join(", ")}`,
			groups,
		);
}

function expectedOutcomeGroups(request: SpeedGroupActionRequest) {
	const action = request.action;
	const addressed =
		action.type === "synchronize"
			? [action.source, action.target]
			: [action.group];
	if (!request.expectedGroups) return addressed.sort();
	assertExpectedGroups(request.expectedGroups);
	const expected = new Set<SpeedGroupId>(addressed);
	for (const group of addressed) {
		const peer = reciprocalPeer(request.expectedGroups, group);
		if (peer) expected.add(peer);
	}
	return [...expected].sort();
}

function assertExpectedGroups(groups: readonly SpeedGroupProjection[]) {
	if (
		groups.length !== speedGroupIds.length ||
		groups.some((group, index) => group.group !== speedGroupIds[index])
	)
		throw new WireValidationError(
			"$.expectedGroups",
			"captured Speed Groups A through E",
			groups,
		);
}

function reciprocalPeer(
	groups: readonly SpeedGroupProjection[],
	group: SpeedGroupId,
) {
	const projection = groups.find((candidate) => candidate.group === group);
	const peer = projection?.synchronizedWith;
	if (!peer) return null;
	const peerProjection = groups.find((candidate) => candidate.group === peer);
	return peerProjection?.synchronizedWith === group ? peer : null;
}
