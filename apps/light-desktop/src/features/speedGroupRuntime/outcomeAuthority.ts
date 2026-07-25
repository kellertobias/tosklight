import type {
	SpeedGroupActionOutcome,
	SpeedGroupAuthorityProjection,
} from "./contracts";
import {
	applyOptimisticAction,
	type OptimisticSpeedGroupMutation,
} from "./optimisticProjection";
import {
	assertCursor,
	canonicalPartialGroups,
	mergeGroups,
	sameAuthorityId,
	sameGroup,
	speedGroupProtocolError,
} from "./projectionValue";

type ChangedOutcome = SpeedGroupActionOutcome & { status: "changed" };
type NoChangeOutcome = SpeedGroupActionOutcome & { status: "no_change" };

export function authorityAfterChangedOutcome(
	authoritative: SpeedGroupAuthorityProjection,
	operation: OptimisticSpeedGroupMutation,
	outcome: ChangedOutcome,
) {
	assertOutcomeAuthority(authoritative, outcome);
	assertCursor(outcome.eventSequence);
	if (outcome.revision < authoritative.revision) return null;
	if (outcome.revision === authoritative.revision) {
		assertGroupsMatch(authoritative, outcome.groups);
		return null;
	}
	if (outcome.revision !== authoritative.revision + 1)
		throw speedGroupProtocolError("outcome revision is not contiguous");
	const optimistic = applyOptimisticAction(
		authoritative,
		operation.action,
		outcome.appliedAtMillis,
	);
	return mergeGroups(
		optimistic,
		outcome.authorityId,
		outcome.revision,
		outcome.groups,
	);
}

export function assertNoChangeOutcome(
	authoritative: SpeedGroupAuthorityProjection,
	outcome: NoChangeOutcome,
) {
	assertOutcomeAuthority(authoritative, outcome);
	if (outcome.revision > authoritative.revision) return false;
	if (outcome.revision === authoritative.revision)
		assertGroupsMatch(authoritative, outcome.groups);
	return true;
}

function assertOutcomeAuthority(
	authoritative: SpeedGroupAuthorityProjection,
	outcome: SpeedGroupActionOutcome,
) {
	if (!sameAuthorityId(outcome.authorityId, authoritative.authorityId))
		throw speedGroupProtocolError("outcome authority does not match");
}

function assertGroupsMatch(
	authoritative: SpeedGroupAuthorityProjection,
	groups: SpeedGroupActionOutcome["groups"],
) {
	for (const group of canonicalPartialGroups(groups)) {
		const current = authoritative.groups.find(
			(candidate) => candidate.group === group.group,
		);
		if (!sameGroup(group, current))
			throw speedGroupProtocolError("outcome group conflicts with authority");
	}
}
