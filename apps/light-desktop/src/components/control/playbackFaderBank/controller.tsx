import { playbackAddressKey } from "../../../api/playbackAddress";
import { useScreensOptional } from "../../../features/screens/ScreensContext";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type {
	PlaybackDefinition,
	PlaybackRuntimeProjection,
	PlaybackSurfaceLayout,
} from "../../../api/types";
import { useSetInteraction } from "../../../features/controlSurfaceInteraction/SetInteractionProvider";
import { useCueRecording } from "../../../features/cueRecording/CueRecordingProvider";
import { useProgrammingUpdate } from "../../../features/programmingUpdate/ProgrammingUpdateProvider";
import {
	usePlaybackDeskView,
	usePlaybackProjectionMap,
	usePlaybackRuntimeActions,
	usePlaybackRuntimeStatus,
} from "../../../features/playbackRuntime/PlaybackRuntimeView";
import {
	playbackTargetMatches,
	usePlaybackTopologyRuntimeReconciliation,
} from "../../../features/playbackRuntime/topologyReconciliation";
import { usePlaybackTopologyActions } from "../../../features/playbackTopology/PlaybackTopologyProvider";
import { usePlaybackTopologyView } from "../../../features/playbackTopology/PlaybackTopologyView";
import {
	useProgrammingCommandLineActions,
	useProgrammingCommandLineView,
	useProgrammingInteractionStatus,
} from "../../../features/programmingInteraction/ProgrammingInteractionView";
import {
	usePortableGroups,
	useShowObjectCollectionsReady,
} from "../../../features/showObjects/ShowObjectsState";
import { useShowObjectView } from "../../../features/showObjects/ShowObjectsView";
import { useApp } from "../../../state/AppContext";
import { HeldPlaybackActions } from "./heldActions";
import { playbackRowUnits, projectPlaybackSlots } from "./projection";
import type { PlaybackConfigurationState } from "./types";

export interface PlaybackFaderBankProps {
	surfaceId?: string;
	/** Preserve readable touch geometry by scrolling fixed pane rows. */
	scrollable?: boolean;
	pageNumber?: number;
	firstSlot?: number;
	count?: number;
	rows?: number;
	buttons?: number;
	playbackLayout?: PlaybackSurfaceLayout | null;
	hardwareConnected?: boolean;
}

export function usePlaybackBankController({
	surfaceId = "main",
	pageNumber,
	firstSlot = 1,
	count,
	rows,
	buttons,
	playbackLayout,
	hardwareConnected = false,
}: PlaybackFaderBankProps) {
	const surfaces = useScreensOptional();
	const surface = surfaces?.surfaces?.playback_surfaces[surfaceId];
	playbackLayout = surface?.layout ?? playbackLayout;
	const topology = usePlaybackTopologyView();
	const topologyActions = usePlaybackTopologyActions();
	const runtimeActions = usePlaybackRuntimeActions();
	const runtimeStatus = usePlaybackRuntimeStatus();
	const playbackDesk = usePlaybackDeskView();
	const commandLine = useProgrammingCommandLineView();
	const commandLineActions = useProgrammingCommandLineActions();
	const commandStatus = useProgrammingInteractionStatus();
	const cueRecording = useCueRecording();
	const programmingUpdate = useProgrammingUpdate();
	const setInteraction = useSetInteraction();
	const { state, dispatch } = useApp();
	const hardware = Boolean(hardwareConnected || state.midiProfile);
	const pageSize = count ?? state.playbackColumns * state.playbackRows;
	const rowCount = playbackLayout?.rows.length ?? rows ?? state.playbackRows;
	const columns = Math.max(
		1,
		...(playbackLayout?.rows.map((row) => row.count) ?? [
			Math.ceil(pageSize / rowCount),
		]),
	);
	const activePageNumber =
		pageNumber ??
		playbackDesk?.resolved_pages[surfaceId] ??
		surfaces?.surfaces?.resolved_pages[surfaceId] ??
		null;
	const pageObject = topology.pages.find(
		(candidate) => candidate.body.number === activePageNumber,
	);
	const { slots, needsGroups, groupCollectionReady } = useProjectedSlots({
		topology,
		pageObject,
		playbackLayout,
		columns,
		firstSlot,
		pageSize,
	});
	const projectionAuthority = useVisiblePlaybackProjections(
		slots,
		activePageNumber,
		topology.ready && activePageNumber != null,
	);
	const authorityReady =
		topology.ready &&
		(!needsGroups || groupCollectionReady) &&
		runtimeStatus.status === "ready" &&
		commandStatus.status === "ready" &&
		playbackDesk !== null &&
		commandLine !== null &&
		activePageNumber !== null &&
		projectionAuthority.loaded &&
		projectionAuthority.matches;
	const [configuration, setConfiguration] =
		useState<PlaybackConfigurationState | null>(null);
	const [cueRecordChoice, setCueRecordChoice] = useState<{
		cueNumber: string;
	} | null>(null);
	const cueRecordChoiceResolver = useRef<
		((choice: "add" | "merge" | "overwrite" | null) => void) | null
	>(null);
	const chooseCueRecordOperation = useCallback((cueNumbers: string[]) => {
		if (cueNumbers.length !== 1)
			return Promise.resolve<"add" | "merge" | "overwrite" | null>("add");
		setCueRecordChoice({ cueNumber: cueNumbers[0] });
		return new Promise<"add" | "merge" | "overwrite" | null>((resolve) => {
			cueRecordChoiceResolver.current = resolve;
		});
	}, []);
	const resolveCueRecordChoice = useCallback(
		(choice: "add" | "merge" | "overwrite" | null) => {
			cueRecordChoiceResolver.current?.(choice);
			cueRecordChoiceResolver.current = null;
			setCueRecordChoice(null);
		},
		[],
	);
	useEffect(
		() => () => {
			cueRecordChoiceResolver.current?.(null);
		},
		[],
	);
	useEffect(() => setConfiguration(null), [activePageNumber, topology.ready]);
	const [controlSourceId] = useState(() => `software:${crypto.randomUUID()}`);
	const heldActions = useMemo(
		() => new HeldPlaybackActions(runtimeActions, controlSourceId),
		[runtimeActions, controlSourceId],
	);
	useEffect(() => heldActions.listen(window), [heldActions]);
	const rowTracks = playbackRowTracks(playbackLayout, hardware, rowCount);
	return {
		surfaceId,
		controlSourceId,
		selectSurface: () => surfaces?.selectSurface(surfaceId),
		topology,
		topologyActions,
		runtimeActions,
		runtimeStatus,
		runtimeProjections: projectionAuthority.projections,
		playbackDesk,
		commandLineActions,
		commandLine,
		cueRecording,
		programmingUpdate,
		setInteraction,
		state,
		dispatch,
		hardware,
		buttons,
		rowCount,
		columns,
		activePageNumber,
		pageObject,
		configuration,
		setConfiguration,
		cueRecordChoice,
		chooseCueRecordOperation,
		resolveCueRecordChoice,
		...playbackCommandModes(state, commandLine?.text ?? "", setInteraction),
		playbackAddressing:
			pageNumber == null
				? ("current_page" as const)
				: ("explicit_page" as const),
		slots,
		rowTracks,
		heldActions,
		authorityReady,
		authorityError:
			topology.error ??
			runtimeStatus.error ??
			commandStatus.error ??
			projectionAuthority.error,
	};
}

export type PlaybackBankController = ReturnType<
	typeof usePlaybackBankController
>;

interface ProjectedSlotsOptions {
	topology: ReturnType<typeof usePlaybackTopologyView>;
	pageObject:
		| ReturnType<typeof usePlaybackTopologyView>["pages"][number]
		| undefined;
	playbackLayout: PlaybackSurfaceLayout | null | undefined;
	columns: number;
	firstSlot: number;
	pageSize: number;
}

function useProjectedSlots(options: ProjectedSlotsOptions) {
	const { topology, pageObject, playbackLayout, columns, firstSlot, pageSize } =
		options;
	const current = useMemo(
		() => ({
			topology,
			pageObject,
			playbackLayout,
			columns,
			firstSlot,
			pageSize,
		}),
		[
			columns,
			firstSlot,
			pageObject,
			pageSize,
			playbackLayout,
			topology.cueLists,
		],
	);
	const withoutGroups = useMemo(() => projectSlots(current, []), [current]);
	const needsGroups = withoutGroups.some(
		(slot) => slot.playback?.target.type === "group",
	);
	const groups = usePortableGroups(needsGroups);
	useShowObjectView("group", needsGroups);
	const groupCollectionReady = useShowObjectCollectionsReady(
		["group"],
		needsGroups,
	);
	const slots = useMemo(
		() => (needsGroups ? projectSlots(current, groups) : withoutGroups),
		[current, groups, needsGroups, withoutGroups],
	);
	return { slots, needsGroups, groupCollectionReady };
}

function projectSlots(
	options: ProjectedSlotsOptions,
	groups: ReturnType<typeof usePortableGroups>,
) {
	return projectPlaybackSlots({
		cueLists: options.topology.cueLists,
		groups,
		page: options.pageObject?.body,
		playbackLayout: options.playbackLayout,
		columns: options.columns,
		firstSlot: options.firstSlot,
		pageSize: options.pageSize,
	});
}

function useVisiblePlaybackProjections(
	slots: ReturnType<typeof projectPlaybackSlots>,
	page: number | null,
	enabled: boolean,
) {
	const addresses = useMemo(
		() =>
			page == null
				? []
				: slots.flatMap((slot) =>
						slot.playback ? [slot.playback.address] : [],
					),
		[slots, page],
	);
	const exactProjections = usePlaybackProjectionMap(enabled ? addresses : []);
	const projections = new Map(
		slots.flatMap((slot) => {
			if (!slot.playback) return [];
			const projection = exactProjections.get(
				playbackAddressKey(slot.playback.address),
			);
			return projection ? [[slot.slot, projection] as const] : [];
		}),
	);
	const loaded = addresses.every((address) =>
		exactProjections.has(playbackAddressKey(address)),
	);
	const matches = slots.every(
		({ playback, slot }) =>
			!playback || projectionMatches(playback, projections.get(slot)),
	);
	const targetKey = JSON.stringify(
		slots.flatMap(({ playback }) =>
			playback ? [[playback.address, playback.target]] : [],
		),
	);
	const reconciling = usePlaybackTopologyRuntimeReconciliation(
		targetKey,
		loaded,
		matches,
	);
	const error =
		loaded && !matches && !reconciling
			? new Error(
					"Playback runtime authority does not match the visible topology",
				)
			: null;
	return { projections, loaded: loaded && !reconciling, matches, error };
}

function projectionMatches(
	playback: PlaybackDefinition,
	projection: PlaybackRuntimeProjection | undefined,
) {
	if (!projection) return false;
	return playbackTargetMatches(playback.target, projection);
}

function playbackRowTracks(
	layout: PlaybackSurfaceLayout | null | undefined,
	hardware: boolean,
	rows: number,
) {
	return layout
		? layout.rows
				.map((row) => `minmax(0, ${playbackRowUnits(row, hardware)}fr)`)
				.join(" ")
		: `repeat(${rows}, minmax(0, 1fr))`;
}

function playbackCommandModes(
	state: ReturnType<typeof useApp>["state"],
	commandText: string,
	setInteraction: ReturnType<typeof useSetInteraction>,
) {
	return {
		assignmentPending: state.cueListSetTarget != null,
		selectionPending: /^SELECT\s*$/i.test(commandText),
		offPending: /^OFF\s*$/i.test(commandText),
		dynamicAssignmentPending: /^ASSIGN\s+DYNAMIC\s+\d+\s*$/i.test(commandText),
		groupAssignmentPending:
			setInteraction?.state?.phase === "group_source_pending" ||
			setInteraction?.state?.phase === "object_source_pending" ||
			(!setInteraction && /^ASSIGN\s+GROUP\s+\S+\s*$/i.test(commandText)),
		setInteractionArmed: setInteraction?.state?.phase === "set_armed",
	};
}
