import { useMemo } from "react";
import {
	useProgrammingSelectionActions,
	useProgrammingSelectionView,
} from "../../../features/programmingInteraction/ProgrammingInteractionView";
import type { PatchedFixture } from "../../../api/types";

export function usePatchSelection(active: boolean) {
	const projection = useProgrammingSelectionView(active);
	const actions = useProgrammingSelectionActions(active);
	const fixtureIds = useMemo(
		() => (projection ? new Set(projection.selected) : null),
		[projection],
	);
	return {
		fixtureIds,
		orderedFixtureIds: projection?.selected ?? null,
		actions: projection ? actions : null,
	};
}

export function fixtureSelectionIds(fixture: PatchedFixture) {
	return fixture.logical_heads.length
		? fixture.logical_heads.map((head) => head.fixture_id)
		: [fixture.fixture_id];
}

export function orderedFixtureSelectionIds(fixtures: readonly PatchedFixture[]) {
	const seen = new Set<string>();
	return fixtures.flatMap((fixture) =>
		fixtureSelectionIds(fixture).filter((fixtureId) => {
			if (seen.has(fixtureId)) return false;
			seen.add(fixtureId);
			return true;
		}),
	);
}

export function toggledFixtureSelection(
	current: readonly string[],
	fixture: PatchedFixture,
) {
	const members = fixtureSelectionIds(fixture);
	const selected = new Set(current);
	return members.every((member) => selected.has(member))
		? current.filter((member) => !members.includes(member))
		: [...current, ...members.filter((member) => !selected.has(member))];
}
