import type { Group } from "./model";

/** Group membership comes from the authoritative group, including unpatched members. */
export function groupFixtureCountLabel(total: number, selected: number) {
	return selected > 0 ? `${selected}/${total}` : String(total);
}

export function groupFixtureSelection(
	group: Group | null,
	selectedFixtures: ReadonlySet<string>,
	selectedGroups: ReadonlySet<string>,
) {
	const members = group?.body.fixtures ?? [];
	const selectedCount = members.filter((id) => selectedFixtures.has(id)).length;
	return {
		selected: Boolean(group && selectedGroups.has(group.id)),
		selectedFixtureCount: selectedCount,
		fullySelected: members.length > 0 && selectedCount === members.length,
		partiallySelected: selectedCount > 0 && selectedCount < members.length,
	};
}
