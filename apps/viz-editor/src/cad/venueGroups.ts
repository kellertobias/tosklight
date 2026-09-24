/**
 * Groups of Venue elements: a name and the fixture IDs of its members, stored in the show.
 *
 * A plain click on a grouped element selects its whole group; Shift picks the element alone. The
 * helpers here are the pure part of that: which group an element is in, a selection widened to
 * whole groups, and the groups after grouping or ungrouping a selection.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { CadEntity } from "./types";

export interface VenueGroup {
	id: string;
	name: string;
	/** The grouped placements' fixture IDs, in the order they were grouped. */
	memberIds: string[];
}

export interface VenueGroups {
	groups: VenueGroup[];
}

export const EMPTY_VENUE_GROUPS: VenueGroups = { groups: [] };

export const venueGroupsSession = {
	get: () => invoke<VenueGroups>("cad_venue_groups"),
	save: (groups: VenueGroups) =>
		invoke<VenueGroups>("save_cad_venue_groups", { groups }),
	onDelta: (handler: (groups: VenueGroups) => void): Promise<UnlistenFn> =>
		listen<VenueGroups>("cad-venue-groups-delta", (event) =>
			handler(event.payload),
		),
};

export function groupOf(
	groups: VenueGroups,
	id: string,
): VenueGroup | undefined {
	return groups.groups.find((group) => group.memberIds.includes(id));
}

/** Every ID replaced by its whole group, in the order first reached and without repeats. */
export function expandToGroups(
	groups: VenueGroups,
	ids: readonly string[],
): string[] {
	const expanded = new Set<string>();
	for (const id of ids)
		for (const member of groupOf(groups, id)?.memberIds ?? [id])
			expanded.add(member);
	return [...expanded];
}

/** The selected placements that can be grouped: Venue elements only, once each. */
export function groupableIds(
	entities: readonly CadEntity[],
	selectedIds: readonly string[],
): string[] {
	const venue = new Set(
		entities
			.filter((entity) => entity.kind === "venue")
			.map((entity) => entity.logicalFixtureId),
	);
	return [...new Set(selectedIds)].filter((id) => venue.has(id));
}

/** The first "Group N" no group is called yet. */
export function nextGroupName(groups: VenueGroups): string {
	const names = new Set(groups.groups.map((group) => group.name));
	for (let number = 1; ; number++)
		if (!names.has(`Group ${number}`)) return `Group ${number}`;
}

/**
 * The groups with `ids` made one new group. An element already in a group leaves it — a group
 * nests no other — and a group left empty goes.
 */
export function groupSelection(
	groups: VenueGroups,
	ids: readonly string[],
	name = nextGroupName(groups),
	id: string = crypto.randomUUID(),
): VenueGroups {
	const members = [...new Set(ids)];
	if (!members.length) return groups;
	const taken = new Set(members);
	return {
		groups: [
			...groups.groups
				.map((group) => ({
					...group,
					memberIds: group.memberIds.filter((member) => !taken.has(member)),
				}))
				.filter((group) => group.memberIds.length),
			{ id, name, memberIds: members },
		],
	};
}

/**
 * What Group or Ungroup would store for the selection, or null when it has nothing to do: Group
 * needs two Venue elements that are not already exactly one group, Ungroup a selected grouped one.
 */
export function venueGroupAction(
	groups: VenueGroups,
	entities: readonly CadEntity[],
	selectedIds: readonly string[],
	action: "group" | "ungroup",
): VenueGroups | null {
	if (action === "ungroup")
		return selectedIds.some((id) => groupOf(groups, id))
			? ungroupSelection(groups, selectedIds)
			: null;
	const ids = groupableIds(entities, selectedIds);
	if (ids.length < 2) return null;
	const existing = groupOf(groups, ids[0]);
	if (
		existing &&
		existing.memberIds.length === ids.length &&
		ids.every((id) => existing.memberIds.includes(id))
	)
		return null;
	return groupSelection(groups, ids);
}

/** The groups without every group any of `ids` belongs to; the elements themselves stay. */
export function ungroupSelection(
	groups: VenueGroups,
	ids: readonly string[],
): VenueGroups {
	const chosen = new Set(ids);
	return {
		groups: groups.groups.filter(
			(group) => !group.memberIds.some((member) => chosen.has(member)),
		),
	};
}

/**
 * The selected elements that are members of a whole selected group: a group every member of which
 * is selected, found through the same expansion a plain pick uses. A group of one reads as the
 * element alone, and an element picked out of its group with Shift is not a group selection.
 */
export function groupSelectedIds(
	selectedIds: readonly string[],
	expand?: (ids: readonly string[]) => string[],
): Set<string> {
	const grouped = new Set<string>();
	if (!expand) return grouped;
	const selected = new Set(selectedIds);
	for (const id of selectedIds) {
		if (grouped.has(id)) continue;
		const members = expand([id]);
		if (members.length > 1 && members.every((member) => selected.has(member)))
			for (const member of members) grouped.add(member);
	}
	return grouped;
}
