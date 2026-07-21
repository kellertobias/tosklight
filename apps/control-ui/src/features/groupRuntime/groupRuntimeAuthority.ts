import { useCallback, useMemo, useRef } from "react";
import type {
	PlaybackOutcome,
	PlaybackProjection,
} from "../playbackRuntime/contracts";
import {
	useGroupProjectionMap,
	usePlaybackRuntimeActions,
	usePlaybackRuntimeStatus,
} from "../playbackRuntime/PlaybackRuntimeView";
import type { ShowObject } from "../showObjects/contracts";
import {
	usePortableGroups,
	useShowObjectCollectionsReady,
} from "../showObjects/ShowObjectsState";
import { useShowObjectView } from "../showObjects/ShowObjectsView";

const GROUP_KINDS = ["group"] as const;
const NO_GROUPS: readonly RuntimeGroup[] = [];

export interface GroupRuntimeState {
	master: number;
	flashLevel: number;
	playbackNumber: number | null;
}

export type RuntimeGroup = ShowObject<"group"> & {
	runtime: GroupRuntimeState;
};

export interface GroupRuntimeAuthority {
	ready: boolean;
	loading: boolean;
	canWrite: boolean;
	groups: readonly RuntimeGroup[];
	setMaster(groupId: string, value: number): Promise<PlaybackOutcome | null>;
	setFlash(groupId: string, pressed: boolean): Promise<PlaybackOutcome | null>;
}

export function useGroupRuntimeAuthority(
	enabled = true,
): GroupRuntimeAuthority {
	useShowObjectView("group", enabled);
	const collectionReady = useShowObjectCollectionsReady(GROUP_KINDS, enabled);
	const portable = usePortableGroups(enabled);
	const groupIds = useMemo(
		() => (enabled && collectionReady ? portable.map(({ id }) => id) : []),
		[collectionReady, enabled, portable],
	);
	const needsRuntime = groupIds.length > 0;
	const runtimeEnabled = enabled && collectionReady && needsRuntime;
	const selection = useGroupProjectionMap(groupIds, runtimeEnabled);
	const status = usePlaybackRuntimeStatus(runtimeEnabled);
	const runtimeReady =
		!needsRuntime || (status.status === "ready" && selection.ready);
	const ready = enabled && collectionReady && runtimeReady;
	const projectionCache = useRef(new Map<string, ProjectedGroupCache>());
	const groups = useMemo(
		() =>
			ready
				? projectRuntimeGroups(
						portable,
						selection.projections,
						projectionCache.current,
					)
				: NO_GROUPS,
		[portable, ready, selection.projections],
	);
	const actions = usePlaybackRuntimeActions();
	const canWrite = ready && actions !== null;
	const setMaster = useCallback(
		(groupId: string, value: number) =>
			canWrite && actions
				? actions.setGroupMaster(groupId, value)
				: Promise.resolve(null),
		[actions, canWrite],
	);
	const setFlash = useCallback(
		(groupId: string, pressed: boolean) =>
			canWrite && actions
				? actions.setGroupFlash(groupId, pressed)
				: Promise.resolve(null),
		[actions, canWrite],
	);
	return {
		ready,
		loading: enabled && !ready,
		canWrite,
		groups,
		setMaster,
		setFlash,
	};
}

interface ProjectedGroupCache {
	portable: ShowObject<"group">;
	projection: PlaybackProjection;
	group: RuntimeGroup;
}

function projectRuntimeGroups(
	portable: readonly ShowObject<"group">[],
	projections: ReadonlyMap<string, PlaybackProjection | undefined>,
	cache: Map<string, ProjectedGroupCache>,
): readonly RuntimeGroup[] {
	const present = new Set(portable.map(({ id }) => id));
	for (const groupId of cache.keys())
		if (!present.has(groupId)) cache.delete(groupId);
	return portable.flatMap((group) => {
		const projection = projections.get(group.id);
		if (projection?.target !== "group" || projection.group_id !== group.id)
			return [];
		const existing = cache.get(group.id);
		if (existing?.portable === group && existing.projection === projection)
			return [existing.group];
		const projected: RuntimeGroup = {
			...group,
			body: { ...group.body, master: projection.master },
			runtime: {
				master: projection.master,
				flashLevel: projection.flash_level,
				playbackNumber: projection.playback_number,
			},
		};
		cache.set(group.id, { portable: group, projection, group: projected });
		return [projected];
	});
}
