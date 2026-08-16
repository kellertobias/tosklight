import { useMemo } from "react";
import { type PatchSelectionHost, usePatchHost } from "../../host";
import type { PatchedFixture } from "../../wire";
import type { PatchController } from "./controller";

/**
 * The host's fixture selection.
 *
 * A desk routes this to the shared programmer selection; a planning application supplies the
 * empty selection and the sheet keeps only its own row cursor.
 *
 * A null `orderedFixtureIds` means the host has no live selection to replace — the desk's
 * programmer projection has not loaded, or the host has no programmer at all. Writes are dropped
 * rather than queued against a selection nobody can see.
 */
export function usePatchSelection(): PatchSelectionHost {
	const { selection } = usePatchHost();
	return useMemo(
		() =>
			selection.orderedFixtureIds === null
				? { ...selection, replace: () => undefined }
				: selection,
		[selection],
	);
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

/** The selected parent fixtures in the host's explicit operator order. */
export function selectedFixturesInOperatorOrder(controller: PatchController) {
	const orderedIds = controller.selection.orderedFixtureIds ?? [];
	const order = new Map(orderedIds.map((id, index) => [id, index]));
	return controller.data.all
		.filter((fixture) =>
			fixtureSelectionIds(fixture).some((id) => order.has(id)),
		)
		.sort((left, right) => {
			const leftIndex = Math.min(
				...fixtureSelectionIds(left).map((id) => order.get(id) ?? Infinity),
			);
			const rightIndex = Math.min(
				...fixtureSelectionIds(right).map((id) => order.get(id) ?? Infinity),
			);
			return leftIndex - rightIndex;
		});
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
