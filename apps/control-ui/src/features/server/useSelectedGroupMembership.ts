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
	const applied = useRef<{
		id: string | null;
		fixtures: readonly string[] | null;
	}>({
		id: null,
		fixtures: null,
	});
	if (observed.current.id !== selectedGroupId) {
		observed.current = { id: selectedGroupId, seen: false };
		applied.current = { id: selectedGroupId, fixtures: null };
	}
	useEffect(() => {
		if (!selectedGroupId) return;
		if (selectedFixtures) {
			observed.current.seen = true;
			if (
				!applied.current.fixtures ||
				!sameMembership(applied.current.fixtures, selectedFixtures)
			) {
				applied.current = { id: selectedGroupId, fixtures: [...selectedFixtures] };
				setSelectedFixtures(selectedFixtures);
			}
		} else if (observed.current.seen) {
			applied.current = { id: null, fixtures: null };
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

function sameMembership(left: readonly string[], right: readonly string[]) {
	return (
		left.length === right.length &&
		left.every((fixtureId, index) => fixtureId === right[index])
	);
}
