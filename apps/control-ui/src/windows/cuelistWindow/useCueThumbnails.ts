import { useEffect, useMemo, useState, useSyncExternalStore } from "react";
import { usePatchedFixturesView } from "../../features/patch/PatchState";
import { useServer } from "../../api/ServerContext";
import type {
	AttributeValue,
	Cue,
	VisualizationSnapshot,
} from "../../api/types";
import type { ShowObject } from "../../features/showObjects/contracts";
import {
	usePortableGroups,
	useShowObjectCollectionsReady,
	useShowObjectsStore,
} from "../../features/showObjects/ShowObjectsState";
import { useDesktopBridge } from "../../platform/desktop";
import {
	cueVisualization,
	migrateStagePosition,
	renderStageThumbnail,
} from "../stage3dScene";

const EMPTY_THUMBNAILS: Record<number, string> = {};
const GROUP_KINDS = ["group"] as const;
const NO_SUBSCRIPTION = () => () => undefined;

function useStageFixtures(enabled: boolean) {
	const server = useServer();
	const fixtures = usePatchedFixturesView(enabled);
	return useMemo(() => {
		if (!enabled) return [];
		return fixtures.flatMap((fixture, fixtureIndex) =>
			[
				{
					id: fixture.fixture_id,
					location: fixture.location,
					rotation: fixture.rotation,
				},
				...(fixture.multipatch ?? []),
			].map((instance, instanceIndex) => {
				const index = fixtureIndex * 16 + instanceIndex;
				const located =
					instance.location &&
					(instance.location.x || instance.location.y || instance.location.z)
						? {
								x: instance.location.x / 1000,
								y: instance.location.y / 1000,
								z: instance.location.z / 1000,
								rotationX: instance.rotation?.x ?? 0,
								rotationY: instance.rotation?.y ?? 0,
								rotationZ: instance.rotation?.z ?? 0,
							}
						: null;
				return {
					fixture,
					instanceId: instance.id,
					index,
					position:
						server.stageLayout?.body.positions3d?.[instance.id] ??
						located ??
						migrateStagePosition(
							instanceIndex
								? undefined
								: server.stageLayout?.body.positions[fixture.fixture_id],
							index,
						),
				};
			}),
		);
	}, [enabled, fixtures, server.stageLayout]);
}

function cueChanges(cue: Cue, groups: readonly ShowObject<"group">[]) {
	const changes = [...(cue.changes ?? [])] as Array<{
		fixture_id: string;
		attribute: string;
		value: AttributeValue | null;
	}>;
	for (const groupChange of cue.group_changes ?? []) {
		const group = groups.find(
			(candidate) => candidate.id === groupChange.group_id,
		);
		for (const fixture_id of group?.body.fixtures ?? []) {
			changes.push({
				fixture_id,
				attribute: groupChange.attribute,
				value: groupChange.value,
			});
		}
	}
	return changes;
}

interface ThumbnailResult {
	authorityGeneration: number;
	cues: Cue[];
	groups: readonly ShowObject<"group">[];
	stageFixtures: ReturnType<typeof useStageFixtures>;
	values: Record<number, string>;
}

function useGroupAuthorityGeneration(enabled: boolean) {
	const store = useShowObjectsStore();
	const getGeneration = () =>
		enabled ? store.getSnapshot().authorityGeneration : -1;
	return useSyncExternalStore(
		enabled ? store.subscribe : NO_SUBSCRIPTION,
		getGeneration,
		getGeneration,
	);
}

export function useCueThumbnails(cues: Cue[], active: boolean) {
	const server = useServer();
	const groups = usePortableGroups(active);
	const groupsReady = useShowObjectCollectionsReady(GROUP_KINDS, active);
	const authorityGeneration = useGroupAuthorityGeneration(active);
	const tauri = useDesktopBridge().available;
	const stageFixtures = useStageFixtures(active);
	const [result, setResult] = useState<ThumbnailResult | null>(null);
	useEffect(() => {
		setResult(null);
		if (
			!active ||
			!groupsReady ||
			!tauri ||
			!cues.length ||
			!stageFixtures.length
		)
			return;
		let cancelled = false;
		void server
			.readVisualization()
			.then((live) => {
				if (cancelled) return;
				let state: VisualizationSnapshot = { ...live, values: [] };
				const next: Record<number, string> = {};
				for (let index = 0; index < cues.length; index++) {
					state = cueVisualization(
						state,
						cueChanges(cues[index], groups),
					);
					next[index] = renderStageThumbnail(stageFixtures, state);
				}
				if (!cancelled)
					setResult({
						authorityGeneration,
						cues,
						groups,
						stageFixtures,
						values: next,
					});
			})
			.catch(() => undefined);
		return () => {
			cancelled = true;
		};
	}, [
		active,
		authorityGeneration,
		cues,
		groups,
		groupsReady,
		server.readVisualization,
		stageFixtures,
		tauri,
	]);
	if (
		!active ||
		!groupsReady ||
		!tauri ||
		!cues.length ||
		!stageFixtures.length ||
		result?.authorityGeneration !== authorityGeneration ||
		result.cues !== cues ||
		result.groups !== groups ||
		result.stageFixtures !== stageFixtures
	)
		return EMPTY_THUMBNAILS;
	return result.values;
}
