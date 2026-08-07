import { useEffect, useMemo, useState, useSyncExternalStore } from "react";
import { usePatchedFixturesView } from "../../features/patch/PatchState";
import {
	useStagePositions,
	useStagePositions3d,
} from "../../features/stageLayout/StageLayoutState";
import { useVisualizationRuntimeRead } from "../../features/visualizationRuntime/VisualizationRuntimeView";
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
import type { CueThumbnailUpload } from "../../api/client/cueThumbnails";
import { useCueThumbnailActions } from "../../features/cueThumbnails/CueThumbnailActions";
import { useDesktopBridge } from "../../platform/desktop";
import {
	cueVisualization,
	migrateStagePosition,
	renderStageThumbnail,
} from "../stage3dScene";
import { cueStateHash, stageGeometryTag } from "./cueThumbnailState";

const EMPTY_THUMBNAILS: Record<number, string> = {};
const THUMBNAIL_WIDTH = 240;
const THUMBNAIL_HEIGHT = 135;
const GROUP_KINDS = ["group"] as const;
const NO_SUBSCRIPTION = () => () => undefined;

function useStageFixtures(enabled: boolean) {
	const fixtures = usePatchedFixturesView(enabled);
	const stagePositions = useStagePositions();
	const stagePositions3d = useStagePositions3d();
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
						stagePositions3d[instance.id] ??
						located ??
						migrateStagePosition(
							instanceIndex
								? undefined
								: stagePositions[fixture.fixture_id],
							index,
						),
				};
			}),
		);
	}, [enabled, fixtures, stagePositions, stagePositions3d]);
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
	/** Object URLs this result created, so they can be released when it is replaced. */
	released: string[];
}

/** How many previews are drawn before yielding, so a long cue list never blocks the desk. */
const RENDER_CHUNK = 4;

function yieldToDesk() {
	return new Promise((resume) => setTimeout(resume, 0));
}

/**
 * Folds the cue list into the tracked state each cue leaves on stage, with the tag describing it.
 *
 * This runs on every open and is deliberately cheap: no WebGL, no pictures — only the arithmetic
 * needed to decide which stored previews are still true.
 */
function trackedStates(
	cues: Cue[],
	groups: readonly ShowObject<"group">[],
	live: VisualizationSnapshot,
	geometry: string,
) {
	let state: VisualizationSnapshot = { ...live, values: [] };
	return cues.map((cue) => {
		state = cueVisualization(state, cueChanges(cue, groups));
		return { state, hash: cueStateHash(state.values, geometry) };
	});
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

/**
 * Cue preview pictures for one cue list.
 *
 * Previews are drawn once and stored with the show, so opening a cue list normally moves no pixels
 * at all: the desk folds the cue list to work out what each preview should show, compares that
 * against what the show already holds, and redraws only the cues whose picture is no longer true.
 * A redraw happens when the operator edits a cue, moves a fixture, or changes a group — the same
 * events that make the old picture a lie.
 *
 * Drawing needs the 3D renderer, but displaying does not: a desk with no renderer still shows the
 * pictures another desk stored.
 */
export function useCueThumbnails(cues: Cue[], active: boolean) {
	const readVisualization = useVisualizationRuntimeRead();
	const groups = usePortableGroups(active);
	const groupsReady = useShowObjectCollectionsReady(GROUP_KINDS, active);
	const authorityGeneration = useGroupAuthorityGeneration(active);
	const canDraw = useDesktopBridge().available;
	const stageFixtures = useStageFixtures(active);
	const previews = useCueThumbnailActions();
	const [result, setResult] = useState<ThumbnailResult | null>(null);
	useEffect(() => {
		setResult((previous) => {
			for (const url of previous?.released ?? []) URL.revokeObjectURL(url);
			return null;
		});
		if (!active || !groupsReady || !cues.length || !stageFixtures.length)
			return;
		let cancelled = false;
		const released: string[] = [];
		const run = async () => {
			const live = await readVisualization();
			if (cancelled) return;
			const geometry = stageGeometryTag(stageFixtures);
			const tracked = trackedStates(cues, groups, live, geometry);

			const stored = new Map<string, string>();
			if (previews?.available) {
				for (const entry of await previews.index())
					stored.set(entry.cueId, entry.stateHash);
			}
			if (cancelled) return;

			const values: Record<number, string> = {};
			const uploads: CueThumbnailUpload[] = [];
			const fetched: number[] = [];
			for (let index = 0; index < cues.length; index++) {
				const cueId = cues[index].id;
				const current = cueId ? stored.get(cueId) : undefined;
				if (current !== undefined && current === tracked[index].hash) {
					fetched.push(index);
					continue;
				}
				if (!canDraw) continue;
				if (index && index % RENDER_CHUNK === 0) {
					await yieldToDesk();
					if (cancelled) return;
				}
				const drawn = renderStageThumbnail(
					stageFixtures,
					tracked[index].state,
					"lines_and_beams",
				);
				values[index] = drawn;
				// A cue with no stored identity cannot be persisted; it still gets its picture,
				// just redrawn each time until the cue list is next saved with ids.
				if (cueId)
					uploads.push({
						cueId,
						stateHash: tracked[index].hash,
						imageBase64: drawn.slice(drawn.indexOf(",") + 1),
						width: THUMBNAIL_WIDTH,
						height: THUMBNAIL_HEIGHT,
					});
			}

			// Pictures already in the show are fetched rather than redrawn. This is the ordinary
			// path on a desk that opens a cue list nobody has changed.
			await Promise.all(
				fetched.map(async (index) => {
					const cueId = cues[index].id;
					if (!cueId || !previews) return;
					try {
						const url = await previews.imageUrl(cueId);
						if (cancelled) {
							URL.revokeObjectURL(url);
							return;
						}
						released.push(url);
						values[index] = url;
					} catch {
						// A picture that cannot be read is simply not shown; the cue list stays usable.
					}
				}),
			);
			if (cancelled) return;

			setResult({
				authorityGeneration,
				cues,
				groups,
				stageFixtures,
				values,
				released,
			});
			// Storing is last and deliberately unawaited by the display: a desk that may not write
			// the show still shows everything it drew.
			if (uploads.length && previews?.canStore)
				previews.store(uploads).catch(() => undefined);
		};
		void run().catch(() => undefined);
		return () => {
			cancelled = true;
			for (const url of released) URL.revokeObjectURL(url);
		};
	}, [
		active,
		authorityGeneration,
		canDraw,
		cues,
		groups,
		groupsReady,
		previews,
		readVisualization,
		stageFixtures,
	]);
	if (
		!active ||
		!groupsReady ||
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
