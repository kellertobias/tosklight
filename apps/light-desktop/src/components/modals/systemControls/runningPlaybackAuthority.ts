import { useCallback, useMemo } from "react";
import type {
	CueList,
	PlaybackDefinition,
	PlaybackPage,
} from "../../../api/types";
import type { CueListRuntimeSource } from "../../../features/playbackRuntime/actionWriter";
import type {
	PlaybackIdentity,
	PlaybackOutcome,
	PlaybackProjection,
} from "../../../features/playbackRuntime/contracts";
import { identityKey } from "../../../features/playbackRuntime/contracts";
import {
	useDirectCueListProjectionMap,
	usePlaybackProjectionMap,
	usePlaybackRuntimeActions,
	usePlaybackRuntimeStatus,
	useVirtualPlaybackProjectionMap,
} from "../../../features/playbackRuntime/PlaybackRuntimeView";
import {
	useCueLists,
	usePlaybackDefinitions,
	usePlaybackPages,
	useShowObjectCollectionsReady,
} from "../../../features/showObjects/ShowObjectsState";
import { useShowObjectKindsView } from "../../../features/showObjects/ShowObjectsView";

const SHOW_KINDS = ["cue_list", "playback", "playback_page"] as const;
const NO_SOURCES: readonly RunningCueListSource[] = [];
const NO_DYNAMICS: readonly RunningDynamic[] = [];

type Cue = CueList["cues"][number];
type CueListProjection = Extract<PlaybackProjection, { target: "cue_list" }>;
type CueListRuntime = NonNullable<CueListProjection["runtime"]>;
type CueListPlayback = PlaybackDefinition & {
	target: { type: "cue_list"; cue_list_id: string };
};

export interface RunningCueListSource
	extends Omit<CueListRuntimeSource, "identity"> {
	identity: Extract<
		PlaybackIdentity,
		{ kind: "playback" | "virtual" | "cue_list" | "direct_cue_list" }
	>;
	key: string;
	/** Stable Cuelist Pool number, independent of the surface runtime identity. */
	cueListNumber?: number | null;
	playbackNumber: number | null;
	locations?: readonly { page: number; slot: number }[];
	label: string;
	runtime: CueListRuntime;
	cueList: CueList | undefined;
	cue: Cue | undefined;
}

export interface RunningDynamic {
	source: RunningCueListSource;
	index: number;
}

export interface RunningPlaybackAuthority {
	ready: boolean;
	loading: boolean;
	error: string | null;
	canRelease: boolean;
	sources: readonly RunningCueListSource[];
	mappedSources: readonly RunningCueListSource[];
	virtualSources: readonly RunningCueListSource[];
	dynamics: readonly RunningDynamic[];
	release(
		source: RunningCueListSource | CueListRuntimeSource,
	): Promise<PlaybackOutcome | null>;
}

export function useRunningPlaybackAuthority(
	enabled: boolean,
): RunningPlaybackAuthority {
	useShowObjectKindsView(SHOW_KINDS, enabled);
	const collectionsReady = useShowObjectCollectionsReady(SHOW_KINDS, enabled);
	const cueListObjects = useCueLists(enabled);
	const playbackObjects = usePlaybackDefinitions(enabled);
	const pageObjects = usePlaybackPages(enabled);
	const cueLists = enabled && collectionsReady ? cueListObjects : [];
	const playbacks = enabled && collectionsReady ? playbackObjects : [];
	const model = useMemo(
		() => portableModel(cueLists, playbacks, pageObjects),
		[cueLists, playbacks, pageObjects],
	);
	const runtimeEnabled = enabled && collectionsReady;
	const mapped = usePlaybackProjectionMap(
		runtimeEnabled ? model.playbackNumbers : [],
	);
	const virtual = useVirtualPlaybackProjectionMap(
		runtimeEnabled ? model.virtualAddresses : [],
	);
	const direct = useDirectCueListProjectionMap(
		runtimeEnabled ? model.cueListIds : [],
		runtimeEnabled,
	);
	const needsRuntime =
		model.playbackNumbers.length > 0 ||
		model.virtualAddresses.length > 0 ||
		model.cueListIds.length > 0;
	const status = usePlaybackRuntimeStatus(runtimeEnabled && needsRuntime);
	const derived = useMemo(
		() => deriveSources(model, mapped, virtual, direct.projections),
		[virtual, mapped, model, direct.projections],
	);
	const runtimeReady =
		!needsRuntime ||
		(status.status === "ready" &&
			derived.mappedReady &&
			derived.virtualReady &&
			direct.ready);
	const ready = enabled && collectionsReady && runtimeReady;
	const actions = usePlaybackRuntimeActions();
	const canRelease = ready && actions !== null;
	const release = useCallback(
		(source: RunningCueListSource | CueListRuntimeSource) =>
			canRelease && actions
				? source.identity.kind === "virtual"
					? actions.virtualPlaybackAction(
							source.identity.page,
							source.identity.playback_number,
							"off",
						)
					: actions.releaseCueListSource({
							identity: source.identity,
							cueListId: source.cueListId,
						})
				: Promise.resolve(null),
		[actions, canRelease],
	);
	return {
		ready,
		loading: enabled && !ready,
		error: status.error?.message ?? null,
		canRelease,
		sources: ready ? derived.sources : NO_SOURCES,
		mappedSources: ready ? derived.mappedSources : NO_SOURCES,
		virtualSources: ready ? derived.virtualSources : NO_SOURCES,
		dynamics: ready ? derived.dynamics : NO_DYNAMICS,
		release,
	};
}

function portableModel(
	cueListObjects: ReturnType<typeof useCueLists>,
	playbackObjects: ReturnType<typeof usePlaybackDefinitions>,
	pageObjects: ReturnType<typeof usePlaybackPages>,
) {
	const cueLists = cueListObjects.map((object) => object.body);
	const playbacks = playbackObjects
		.map((object) => object.body)
		.filter(targetsCueList)
		.sort((left, right) => left.number - right.number);
	const cueListNumbers = new Map<string, number>();
	for (const playback of playbacks) {
		if (!cueListNumbers.has(playback.target.cue_list_id))
			cueListNumbers.set(playback.target.cue_list_id, playback.number);
	}
	return {
		cueLists,
		playbacks,
		cueListNumbers,
		cueListIds: cueLists.map((cueList) => cueList.id),
		// Cuelist-requested projections aggregate all owners and erase source identity.
		// Running rows must subscribe to exact physical/virtual addresses instead.
		playbackNumbers: playbacks.map((playback) => playback.number),
		virtualAddresses: pageObjects.flatMap(({ body: page }) =>
			Object.entries(page.virtual_playbacks)
				.filter(([, definition]) => targetsCueList(definition))
				.map(([slot]) => ({ page: page.number, playbackNumber: Number(slot) })),
		),
		pages: pageObjects.map((object) => object.body),
	};
}

function deriveSources(
	model: ReturnType<typeof portableModel>,
	mapped: ReadonlyMap<number, PlaybackProjection | undefined>,
	virtual: ReadonlyMap<string, PlaybackProjection | undefined>,
	direct: ReadonlyMap<string, PlaybackProjection | undefined>,
) {
	const cueLists = new Map(
		model.cueLists.map((cueList) => [cueList.id, cueList]),
	);
	let mappedReady = true;
	const mappedSources = model.playbacks.flatMap((playback) => {
		const projection = mapped.get(playback.number);
		if (!matchesPlayback(projection, playback)) {
			mappedReady = false;
			return [];
		}
		const runtime = projection.runtime;
		return runtime?.enabled && ownsRuntime(projection)
			? [
					source(
						projection,
						runtime,
						cueLists.get(playback.target.cue_list_id),
						model.cueListNumbers.get(playback.target.cue_list_id) ?? null,
						playback,
						model.pages,
					),
				]
			: [];
	});
	let virtualReady = true;
	const virtualSources = model.virtualAddresses.flatMap((address) => {
		const projection = virtual.get(
			`virtual:${address.page}.${address.playbackNumber}`,
		);
		if (
			!projection ||
			projection.target !== "cue_list" ||
			projection.requested.kind !== "virtual"
		) {
			virtualReady = false;
			return [];
		}
		const cueList = cueLists.get(projection.cue_list_id);
		const runtime = projection.runtime;
		return runtime?.enabled && ownsRuntime(projection)
			? [
					source(
						projection,
						runtime,
						cueList,
						model.cueListNumbers.get(projection.cue_list_id) ?? null,
					),
				]
			: [];
	});
	const directSources = model.cueLists.flatMap((cueList) => {
		const projection = direct.get(cueList.id);
		if (
			projection?.target !== "cue_list" ||
			projection.requested.kind !== "direct_cue_list" ||
			!projection.runtime?.enabled ||
			!ownsRuntime(projection)
		)
			return [];
		return [
			source(
				projection,
				projection.runtime,
				cueList,
				model.cueListNumbers.get(cueList.id) ?? null,
			),
		];
	});
	const sources = [...mappedSources, ...virtualSources, ...directSources];
	return {
		mappedReady,
		virtualReady,
		mappedSources,
		virtualSources,
		sources,
		dynamics: [],
	};
}

function source(
	projection: CueListProjection,
	runtime: CueListRuntime,
	cueList: CueList | undefined,
	cueListNumber: number | null,
	playback?: CueListPlayback,
	pages: readonly PlaybackPage[] = [],
): RunningCueListSource {
	const playbackNumber = projection.playback_number;
	const identity: PlaybackIdentity =
		projection.requested.kind === "virtual" ||
		projection.requested.kind === "direct_cue_list"
			? projection.requested
			: playbackNumber == null
				? { kind: "cue_list", cue_list_id: projection.cue_list_id }
				: { kind: "playback", playback_number: playbackNumber };
	return {
		key: identityKey(identity),
		identity,
		cueListId: projection.cue_list_id,
		cueListNumber,
		playbackNumber,
		locations:
			identity.kind === "virtual"
				? [{ page: identity.page, slot: identity.playback_number }]
				: playbackNumber == null
					? []
					: pages
							.flatMap((page) =>
								Object.entries(page.slots)
									.filter(([, number]) => number === playbackNumber)
									.map(([slot]) => ({ page: page.number, slot: Number(slot) })),
							)
							.sort((a, b) => a.page - b.page || a.slot - b.slot),
		label:
			playback?.name ||
			cueList?.name ||
			`Cuelist ${projection.cue_list_id.slice(0, 8)}`,
		runtime,
		cueList,
		cue: cueList?.cues[runtime.cue_index],
	};
}

function matchesPlayback(
	projection: PlaybackProjection | undefined,
	playback: CueListPlayback,
): projection is CueListProjection {
	return (
		projection?.playback_number === playback.number &&
		projection.target === "cue_list" &&
		projection.cue_list_id === playback.target.cue_list_id
	);
}

function ownsRuntime(projection: CueListProjection): boolean {
	const owner = projection.runtime?.owner;
	return (
		owner != null && identityKey(owner) === identityKey(projection.requested)
	);
}

function targetsCueList(
	playback: PlaybackDefinition,
): playback is CueListPlayback {
	return playback.target.type === "cue_list";
}
