import type { CueList, PatchedFixture, StoredGroup } from "../api/types";
import type { ProgrammerValueTargets } from "../features/programmerValues/useProgrammerValueTargets";
import { resolveGroupMembership } from "../features/showObjects/groupProjection";

type Group = { id: string; body: StoredGroup };

/**
 * The regular Fixture Sheet is a programming surface. Scenic fixtures remain in
 * the show and selection authority, but none of their independent legacy/profile
 * identities may make them visible here.
 */
export function fixtureSheetIncludesFixture(fixture: PatchedFixture) {
	if (fixture.definition.profile_snapshot?.patch_policy === "visual_only")
		return false;
	if (fixture.definition.manufacturer === "Venue") return false;
	const completeFixtureId =
		fixture.virtual_fixture_number != null
			? `0.${fixture.virtual_fixture_number}`
			: fixture.fixture_number != null
				? String(fixture.fixture_number)
				: fixture.fixture_id;
	return !completeFixtureId.startsWith("0.");
}

function fixtureIdsForGroups(
	groupIds: Iterable<string>,
	groups: readonly Group[],
) {
	const wanted = new Set(groupIds);
	const membership = resolveGroupMembership(groups);
	return new Set(
		groups
			.filter((group) => wanted.has(group.id))
			.flatMap((group) => membership.get(group.id) ?? []),
	);
}

export function activeProgrammerFixtureIds(
	programmer: ProgrammerValueTargets | null,
	groups: readonly Group[],
) {
	const active = fixtureIdsForGroups(programmer?.groupIds ?? [], groups);
	for (const fixtureId of programmer?.fixtureIds ?? []) active.add(fixtureId);
	return active;
}

export function cueListFixtureIds(
	cueList: CueList | undefined,
	groups: readonly Group[],
) {
	if (!cueList) return null;
	const fixtureIds = new Set(
		cueList.cues.flatMap((cue) =>
			cue.changes.map((change) => change.fixture_id),
		),
	);
	const groupIds = cueList.cues.flatMap((cue) =>
		(cue.group_changes ?? []).map((change) => change.group_id),
	);
	for (const fixtureId of fixtureIdsForGroups(groupIds, groups))
		fixtureIds.add(fixtureId);
	return fixtureIds;
}

export function fixtureIsIncluded(
	fixture: PatchedFixture,
	fixtureIds: Set<string>,
) {
	return [
		fixture.fixture_id,
		...fixture.logical_heads.map((head) => head.fixture_id),
	].some((fixtureId) => fixtureIds.has(fixtureId));
}

export function compareFixtureIds(a: PatchedFixture, b: PatchedFixture) {
	const aVirtual = a.virtual_fixture_number ?? Number.POSITIVE_INFINITY;
	const bVirtual = b.virtual_fixture_number ?? Number.POSITIVE_INFINITY;
	if (aVirtual !== bVirtual) return aVirtual - bVirtual;
	if (a.virtual_fixture_number != null) return -1;
	if (b.virtual_fixture_number != null) return 1;
	const aNumber = a.fixture_number ?? Number.POSITIVE_INFINITY;
	const bNumber = b.fixture_number ?? Number.POSITIVE_INFINITY;
	return aNumber - bNumber || a.fixture_id.localeCompare(b.fixture_id);
}
