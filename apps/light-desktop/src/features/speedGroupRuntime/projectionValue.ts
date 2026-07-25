import {
	type SpeedGroupAction,
	type SpeedGroupAuthorityProjection,
	type SpeedGroupId,
	type SpeedGroupProjection,
	type SpeedGroupSnapshot,
	speedGroupIds,
} from "./contracts";
import { SpeedGroupProtocolError } from "./transport";

const GROUP_SET = new Set<SpeedGroupId>(speedGroupIds);

export function canonicalAuthority(
	projection: SpeedGroupAuthorityProjection,
): SpeedGroupAuthorityProjection {
	assertAuthorityId(projection.authorityId);
	assertNonNegativeInteger(projection.revision, "revision");
	if (projection.groups.length !== speedGroupIds.length)
		throw protocolError("snapshot must contain exactly five groups");
	const groups = projection.groups.map((group, index) => {
		const canonical = canonicalGroup(group);
		if (canonical.group !== speedGroupIds[index])
			throw protocolError("snapshot groups must be ordered A through E");
		return canonical;
	});
	return Object.freeze({ ...projection, groups: Object.freeze(groups) });
}

export function canonicalPartialGroups(
	groups: readonly SpeedGroupProjection[],
) {
	if (groups.length < 1 || groups.length > speedGroupIds.length)
		throw protocolError("change must contain one through five groups");
	const seen = new Set<SpeedGroupId>();
	return Object.freeze(
		groups.map((group) => {
			const canonical = canonicalGroup(group);
			if (seen.has(canonical.group))
				throw protocolError("change contains a duplicate group");
			seen.add(canonical.group);
			return canonical;
		}),
	);
}

export function canonicalGroup(group: SpeedGroupProjection) {
	assertGroup(group.group);
	assertBpm(group.manualBpm);
	if (typeof group.paused !== "boolean")
		throw protocolError("paused must be boolean");
	if (
		!Number.isFinite(group.speedMasterScale) ||
		group.speedMasterScale < 0 ||
		group.speedMasterScale > 4
	)
		throw protocolError("speed master scale must be from 0 through 4");
	if (group.synchronizedWith !== null) {
		assertGroup(group.synchronizedWith);
		if (group.synchronizedWith === group.group)
			throw protocolError("a group cannot synchronize with itself");
	}
	assertNonNegativeInteger(group.phaseOriginMillis, "phase origin");
	return Object.freeze({ ...group });
}

export function sameAuthority(
	left: SpeedGroupAuthorityProjection,
	right: SpeedGroupAuthorityProjection,
) {
	return (
		sameAuthorityId(left.authorityId, right.authorityId) &&
		left.revision === right.revision &&
		left.groups.every((group, index) => sameGroup(group, right.groups[index]))
	);
}

export function sameAuthorityId(left: string, right: string) {
	return left.toLowerCase() === right.toLowerCase();
}

export function sameGroup(
	left: SpeedGroupProjection,
	right: SpeedGroupProjection | undefined,
) {
	return Boolean(
		right &&
			left.group === right.group &&
			left.manualBpm === right.manualBpm &&
			left.paused === right.paused &&
			left.speedMasterScale === right.speedMasterScale &&
			left.synchronizedWith === right.synchronizedWith &&
			left.phaseOriginMillis === right.phaseOriginMillis,
	);
}

export function mergeGroups(
	current: SpeedGroupAuthorityProjection,
	authorityId: string,
	revision: number,
	groups: readonly SpeedGroupProjection[],
) {
	assertAuthorityId(authorityId);
	assertNonNegativeInteger(revision, "revision");
	const replacements = new Map(
		canonicalPartialGroups(groups).map((group) => [group.group, group]),
	);
	return canonicalAuthority({
		authorityId,
		revision,
		groups: current.groups.map(
			(group) => replacements.get(group.group) ?? group,
		),
	});
}

export function assertAction(action: SpeedGroupAction) {
	if (action.type === "set_bpm") {
		assertGroup(action.group);
		assertBpm(action.bpm);
		return;
	}
	if (action.type === "adjust_bpm") {
		assertGroup(action.group);
		if (!Number.isFinite(action.deltaBpm) || action.deltaBpm === 0)
			throw protocolError("BPM adjustment must be finite and non-zero");
		return;
	}
	assertGroup(action.source);
	assertGroup(action.target);
	if (action.source === action.target)
		throw protocolError("synchronization needs two different groups");
}

export function assertBpm(value: number) {
	if (!Number.isFinite(value) || value < 0.1 || value > 999)
		throw protocolError("BPM must be from 0.1 through 999");
}

export function assertGroup(value: SpeedGroupId) {
	if (!GROUP_SET.has(value)) throw protocolError("group must be A through E");
}

export function assertRequestId(value: string) {
	const bytes = new TextEncoder().encode(value).length;
	if (!value || bytes > 128 || /\p{Cc}/u.test(value))
		throw protocolError("request ID must contain 1-128 printable bytes");
}

export function assertAuthorityId(value: string) {
	if (!value) throw protocolError("authority ID is required");
}

export function assertCursor(value: number) {
	assertNonNegativeInteger(value, "event cursor");
}

export function assertRepairDoesNotRegress(
	current: {
		eventSequence: number | null;
		authorityId: string | null;
		authorityRevision: number | null;
	},
	snapshot: SpeedGroupSnapshot,
) {
	if (current.eventSequence !== null && snapshot.cursor < current.eventSequence)
		throw speedGroupProtocolError(
			"repair cursor moved backwards",
			snapshot.cursor,
		);
	if (
		current.authorityId === snapshot.projection.authorityId &&
		current.authorityRevision !== null &&
		snapshot.projection.revision < current.authorityRevision
	)
		throw speedGroupProtocolError(
			"repair revision moved backwards",
			snapshot.cursor,
		);
}

export function assertNonNegativeInteger(value: number, label: string) {
	if (!Number.isSafeInteger(value) || value < 0)
		throw protocolError(`${label} must be a non-negative safe integer`);
}

export function speedGroupProtocolError(
	message: string,
	eventSequence: number | null = null,
) {
	return new SpeedGroupProtocolError(
		`Speed Group runtime ${message}`,
		eventSequence,
	);
}

const protocolError = speedGroupProtocolError;
