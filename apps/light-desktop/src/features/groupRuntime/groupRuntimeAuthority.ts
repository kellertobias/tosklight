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
	usePlaybackDefinitions,
	usePlaybackPages,
	usePortableGroups,
	useShowObjectCollectionsReady,
} from "../showObjects/ShowObjectsState";
import { useShowObjectView } from "../showObjects/ShowObjectsView";

const GROUP_KINDS = ["group", "playback", "playback_page"] as const;
const NO_GROUPS: readonly RuntimeGroup[] = [];

export interface GroupRuntimeState {
	master: number;
	flashLevel: number;
	playbackNumber: number | null;
}

export type RuntimeGroup = ShowObject<"group"> & {
	runtime: GroupRuntimeState | null;
};

export interface GroupRuntimeAuthority {
	ready: boolean;
	loading: boolean;
	/**
	 * `true` whenever `groups` is usable — live-authoritative or retained from the last
	 * ready state of the same show. Rendering surfaces gate on this instead of `ready` so
	 * a transient scope refresh (opening a window re-subscribes the shared runtime stream)
	 * never blanks already-presented content.
	 */
	serving: boolean;
	canWrite: boolean;
	groups: readonly RuntimeGroup[];
	setMaster(groupId: string, value: number): Promise<PlaybackOutcome | null>;
	setFlash(groupId: string, pressed: boolean): Promise<PlaybackOutcome | null>;
}

export function useGroupRuntimeAuthority(
	enabled = true,
): GroupRuntimeAuthority {
	useShowObjectView("group", enabled);
	useShowObjectView("playback", enabled);
	useShowObjectView("playback_page", enabled);
	const collectionReady = useShowObjectCollectionsReady(GROUP_KINDS, enabled);
	const portable = usePortableGroups(enabled);
	const playbacks = usePlaybackDefinitions(enabled);
	const pages = usePlaybackPages(enabled);
	const groupIds = useMemo(
		() =>
			enabled && collectionReady ? assignedGroupIds(playbacks, pages) : [],
		[collectionReady, enabled, pages, playbacks],
	);
	const needsRuntime = groupIds.length > 0;
	const runtimeEnabled = enabled && collectionReady && needsRuntime;
	const selection = useGroupProjectionMap(groupIds, runtimeEnabled);
	const status = usePlaybackRuntimeStatus(runtimeEnabled);
	const runtimeReady =
		!needsRuntime || (status.status === "ready" && selection.ready);
	const ready = enabled && collectionReady && runtimeReady;
	const projectionCache = useRef(new Map<string, ProjectedGroupCache>());
	const retained = useRef<readonly RuntimeGroup[] | null>(null);
	const liveGroups = useMemo(
		() =>
			ready
				? projectRuntimeGroups(
						portable,
						selection.projections,
						projectionCache.current,
					)
				: null,
		[portable, ready, selection.projections],
	);
	// Retention only bridges transient runtime refreshes within one hydrated Group
	// collection; a Show switch (or disable) resets the collection and drops it.
	if (!enabled || !collectionReady) retained.current = null;
	else if (liveGroups) retained.current = liveGroups;
	const groups = liveGroups ?? retained.current ?? NO_GROUPS;
	const serving = enabled && (ready || retained.current !== null);
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
		serving,
		canWrite,
		groups,
		setMaster,
		setFlash,
	};
}

interface ProjectedGroupCache {
	portable: ShowObject<"group">;
	projection: PlaybackProjection | undefined;
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
		const groupProjection =
			projection?.target === "group" && projection.group_id === group.id
				? projection
				: undefined;
		const existing = cache.get(group.id);
		if (existing?.portable === group && existing.projection === groupProjection)
			return [existing.group];
		const projected: RuntimeGroup = {
			...group,
			runtime: groupProjection
				? {
						master: groupProjection.master,
						flashLevel: groupProjection.flash_level,
						playbackNumber: groupProjection.playback_number,
					}
				: null,
		};
		cache.set(group.id, {
			portable: group,
			projection: groupProjection,
			group: projected,
		});
		return [projected];
	});
}

function assignedGroupIds(
	playbacks: readonly ShowObject<"playback">[],
	pages: readonly ShowObject<"playback_page">[],
) {
	const ids = new Set<string>();
	for (const { body } of playbacks)
		if (body.target.type === "group") ids.add(body.target.group_id);
	for (const { body } of pages)
		for (const playback of Object.values(body.virtual_playbacks))
			if (playback.target.type === "group") ids.add(playback.target.group_id);
	return [...ids].sort((left, right) => left.localeCompare(right));
}
