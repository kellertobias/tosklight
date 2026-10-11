import { useCallback, useMemo } from "react";
import type { CueList } from "../../../api/types";
import type { CueListRuntimeSource } from "../../../features/playbackRuntime/actionWriter";
import type { PlaybackIdentity, PlaybackOutcome, PlaybackProjection } from "../../../features/playbackRuntime/contracts";
import { identityKey } from "../../../features/playbackRuntime/contracts";
import { useDirectCueListProjectionMap, usePlaybackProjectionMap, usePlaybackRuntimeActions, usePlaybackRuntimeStatus } from "../../../features/playbackRuntime/PlaybackRuntimeView";
import { useCueLists, usePlaybackPages, useShowObjectCollectionsReady } from "../../../features/showObjects/ShowObjectsState";
import { useShowObjectKindsView } from "../../../features/showObjects/ShowObjectsView";
const SHOW_KINDS = ["cue_list", "playback_page"] as const;
const NO_SOURCES: readonly RunningCueListSource[] = [];
const NO_DYNAMICS: readonly RunningDynamic[] = [];
type Cue = CueList["cues"][number];
type CueListProjection = Extract<PlaybackProjection, {target: "cue_list"}>;
type CueListRuntime = NonNullable<CueListProjection["runtime"]>;
export interface RunningCueListSource
	extends Omit<CueListRuntimeSource, "identity"> {
	identity: Extract<
		PlaybackIdentity,
		{ kind: "playback" | "cue_list" | "direct_cue_list" }
	>;
	key: string;
	/** Stable Cuelist Pool number, independent of the surface runtime identity. */
	cueListNumber?: number | null;


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
	assignedSources: readonly RunningCueListSource[];
	dynamics: readonly RunningDynamic[];
	release(
		source: RunningCueListSource | CueListRuntimeSource,
	): Promise<PlaybackOutcome | null>;
}

export function useRunningPlaybackAuthority(enabled: boolean): RunningPlaybackAuthority {
	useShowObjectKindsView(SHOW_KINDS, enabled);
	const collectionsReady = useShowObjectCollectionsReady(SHOW_KINDS, enabled);
	const cueLists = useCueLists(enabled);
	const pages = usePlaybackPages(enabled);
	const addresses = useMemo(() => enabled && collectionsReady ? pages.flatMap(page => Object.entries(page.body.assignments).map(([position]) => ({page: page.body.number, position: Number(position)}))) : [], [pages, enabled, collectionsReady]);
	const assigned = usePlaybackProjectionMap(addresses);
	const direct = useDirectCueListProjectionMap(enabled && collectionsReady ? cueLists.map(object => object.body.id) : [], enabled && collectionsReady);
	const status = usePlaybackRuntimeStatus(enabled && collectionsReady);
	const assignedReady = addresses.every(address => assigned.has(identityKey({kind: "playback", ...address})));
	const ready = enabled && collectionsReady && assignedReady && direct.ready && status.status === "ready";
	const sources = useMemo(() => {
		const candidates = [...assigned.values(), ...direct.projections.values()];
		return candidates.flatMap(projection => {
			if (!projection || projection.target !== "cue_list" || !projection.runtime?.enabled || !ownsRuntime(projection)) return [];
			const list = cueLists.find(object => object.body.id === projection.cue_list_id)?.body;
			const address = projection.identity.kind === "playback" ? projection.identity : null;
			const definition = address ? pages.find(object => object.body.number === address.page)?.body.assignments[String(address.position)] : null;
			return [{key: identityKey(projection.identity), identity: projection.identity, cueListId: projection.cue_list_id, cueListNumber: list?.pool_number ?? null,
				label: definition?.name || list?.name || `Cuelist ${projection.cue_list_id.slice(0, 8)}`, runtime: projection.runtime, cueList: list, cue: list?.cues[projection.runtime.cue_index]} as RunningCueListSource];
		});
	}, [assigned, direct.projections, cueLists, pages]);
	const actions = usePlaybackRuntimeActions();
	const canRelease = ready && actions !== null;
	const release = useCallback((source: RunningCueListSource | CueListRuntimeSource) => canRelease && actions ? actions.releaseCueListSource(source) : Promise.resolve(null), [actions, canRelease]);
	return {ready, loading: enabled && !ready, error: status.error?.message ?? null, canRelease, sources: ready ? sources : NO_SOURCES,
		assignedSources: ready ? sources.filter(source => source.identity.kind === "playback") : NO_SOURCES, dynamics: NO_DYNAMICS, release};
}
function ownsRuntime(projection: CueListProjection) {
	return projection.runtime?.owner != null && identityKey(projection.runtime.owner) === identityKey(projection.identity);
}
