import { useEffect, useRef } from "react";
import type { StoredGroup, VersionedObject } from "../../api/types";

/** Keeps the selected live Group projection aligned with authoritative membership. */
export function useSelectedGroupMembership(
	groups: readonly VersionedObject<StoredGroup>[],
	selectedGroupId: string | null,
	setSelectedGroupId: (id: string | null) => void,
	setSelectedFixtures: (fixtures: string[]) => void,
) {
	const selectedGroup = selectedGroupId
		? groups.find((candidate) => candidate.id === selectedGroupId)
		: undefined;
	const selectedFixtures = selectedGroup?.body.fixtures;
	const observed = useRef<{ id: string | null; seen: boolean }>({
		id: null,
		seen: false,
	});
	if (observed.current.id !== selectedGroupId)
		observed.current = { id: selectedGroupId, seen: false };
	useEffect(() => {
		if (!selectedGroupId) return;
		if (selectedFixtures) {
			observed.current.seen = true;
			setSelectedFixtures(selectedFixtures);
		} else if (observed.current.seen) {
			setSelectedFixtures([]);
			setSelectedGroupId(null);
		}
	}, [
		selectedFixtures,
		selectedGroupId,
		setSelectedFixtures,
		setSelectedGroupId,
	]);
}
