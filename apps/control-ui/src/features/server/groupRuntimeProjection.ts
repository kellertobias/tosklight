import type {
	PlaybackSnapshot,
	StoredGroup,
	VersionedObject,
} from "../../api/types";

type GroupObject = VersionedObject<StoredGroup>;
type RuntimeGroupControl = NonNullable<
	PlaybackSnapshot["authoritative_controls"]
>["groups"][number];

/** Overlays desk/runtime master feedback without mutating portable Group objects. */
export function projectRuntimeGroupMasters(
	groups: readonly GroupObject[],
	controls: readonly RuntimeGroupControl[] | undefined,
): readonly GroupObject[] {
	if (!controls?.length) return groups;
	const masters = new Map(controls.map((control) => [control.id, control.master]));
	let changed = false;
	const projected = groups.map((group) => {
		const master = masters.get(group.id);
		if (master == null || master === group.body.master) return group;
		changed = true;
		return { ...group, body: { ...group.body, master } };
	});
	return changed ? projected : groups;
}

/** Applies successful command feedback to the runtime projection until the next server sample. */
export function updateRuntimeGroupMaster(
	playbacks: PlaybackSnapshot | null,
	groupId: string,
	master: number,
): PlaybackSnapshot | null {
	const controls = playbacks?.authoritative_controls;
	if (!playbacks || !controls) return playbacks;
	const existing = controls.groups.find((group) => group.id === groupId);
	const groups = existing
		? controls.groups.map((group) =>
				group.id === groupId ? { ...group, master } : group,
			)
		: [...controls.groups, { id: groupId, master, flash_level: 0 }];
	return {
		...playbacks,
		authoritative_controls: { ...controls, groups },
	};
}
