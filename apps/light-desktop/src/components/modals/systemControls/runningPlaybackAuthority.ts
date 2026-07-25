import { useCallback, useMemo } from "react";
import type { CueList, PlaybackDefinition } from "../../../api/types";
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
} from "../../../features/playbackRuntime/PlaybackRuntimeView";
import {
	useCueLists,
	usePlaybackDefinitions,
	useShowObjectCollectionsReady,
} from "../../../features/showObjects/ShowObjectsState";
import { useShowObjectKindsView } from "../../../features/showObjects/ShowObjectsView";

const SHOW_KINDS = ["cue_list", "playback"] as const;
const NO_SOURCES: readonly RunningCueListSource[] = [];
const NO_DYNAMICS: readonly RunningDynamic[] = [];

type Cue = CueList["cues"][number];
type CueListProjection = Extract<PlaybackProjection, { target: "cue_list" }>;
type CueListRuntime = NonNullable<CueListProjection["runtime"]>;
type CueListPlayback = PlaybackDefinition & {
	target: { type: "cue_list"; cue_list_id: string };
};

export interface RunningCueListSource extends CueListRuntimeSource {
	key: string;
	playbackNumber: number | null;
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
	canRelease: boolean;
	sources: readonly RunningCueListSource[];
	mappedSources: readonly RunningCueListSource[];
	virtualSources: readonly RunningCueListSource[];
	dynamics: readonly RunningDynamic[];
	release(source: CueListRuntimeSource): Promise<PlaybackOutcome | null>;
}

export function useRunningPlaybackAuthority(
	enabled: boolean,
): RunningPlaybackAuthority {
	useShowObjectKindsView(SHOW_KINDS, enabled);
	const collectionsReady = useShowObjectCollectionsReady(SHOW_KINDS, enabled);
	const cueListObjects = useCueLists(enabled);
	const playbackObjects = usePlaybackDefinitions(enabled);
	const cueLists = enabled && collectionsReady ? cueListObjects : [];
	const playbacks = enabled && collectionsReady ? playbackObjects : [];
	const model = useMemo(
		() => portableModel(cueLists, playbacks),
		[cueLists, playbacks],
	);
	const runtimeEnabled = enabled && collectionsReady;
	const mapped = usePlaybackProjectionMap(
		runtimeEnabled ? model.playbackNumbers : [],
	);
	const direct = useDirectCueListProjectionMap(
		runtimeEnabled ? model.cueListIds : [],
		runtimeEnabled,
	);
	const needsRuntime =
		model.playbackNumbers.length > 0 || model.cueListIds.length > 0;
	const status = usePlaybackRuntimeStatus(runtimeEnabled && needsRuntime);
	const derived = useMemo(
		() => deriveSources(model, mapped, direct.projections),
		[direct.projections, mapped, model],
	);
	const runtimeReady =
		!needsRuntime ||
		(status.status === "ready" && derived.mappedReady && direct.ready);
	const ready = enabled && collectionsReady && runtimeReady;
	const actions = usePlaybackRuntimeActions();
	const canRelease = ready && actions !== null;
	const release = useCallback(
		(source: CueListRuntimeSource) =>
			canRelease && actions
				? actions.releaseCueListSource(source)
				: Promise.resolve(null),
		[actions, canRelease],
	);
	return {
		ready,
		loading: enabled && !ready,
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
) {
	const cueLists = cueListObjects.map((object) => object.body);
	const playbacks = playbackObjects
		.map((object) => object.body)
		.filter(targetsCueList)
		.sort((left, right) => left.number - right.number);
	return {
		cueLists,
		playbacks,
		cueListIds: cueLists.map((cueList) => cueList.id),
		playbackNumbers: playbacks.map((playback) => playback.number),
	};
}

function deriveSources(
	model: ReturnType<typeof portableModel>,
	mapped: ReadonlyMap<number, PlaybackProjection | undefined>,
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
		return runtime
			? [
					source(
						projection,
						runtime,
						cueLists.get(playback.target.cue_list_id),
						playback,
					),
				]
			: [];
	});
	const virtualSources = model.cueLists.flatMap((cueList) => {
		const projection = direct.get(cueList.id);
		if (!isDirectRuntime(projection, cueList.id)) return [];
		const runtime = projection.runtime;
		return runtime ? [source(projection, runtime, cueList)] : [];
	});
	const sources = [...mappedSources, ...virtualSources];
	return {
		mappedReady,
		mappedSources,
		virtualSources,
		sources,
		dynamics: sources.flatMap((running) =>
			(running.cue?.phasers ?? []).map((_, index) => ({
				source: running,
				index,
			})),
		),
	};
}

function source(
	projection: CueListProjection,
	runtime: CueListRuntime,
	cueList: CueList | undefined,
	playback?: CueListPlayback,
): RunningCueListSource {
	const playbackNumber = projection.playback_number;
	const identity: PlaybackIdentity =
		playbackNumber == null
			? { kind: "cue_list", cue_list_id: projection.cue_list_id }
			: { kind: "playback", playback_number: playbackNumber };
	return {
		key: identityKey(identity),
		identity,
		cueListId: projection.cue_list_id,
		playbackNumber,
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

function isDirectRuntime(
	projection: PlaybackProjection | undefined,
	cueListId: string,
): projection is CueListProjection {
	return (
		projection?.playback_number === null &&
		projection.target === "cue_list" &&
		projection.cue_list_id === cueListId
	);
}

function targetsCueList(
	playback: PlaybackDefinition,
): playback is CueListPlayback {
	return playback.target.type === "cue_list";
}
