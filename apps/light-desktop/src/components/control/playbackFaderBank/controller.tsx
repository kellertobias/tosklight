import { useEffect, useMemo, useState } from "react";
import type {
	PlaybackDefinition,
	PlaybackRuntimeProjection,
	PlaybackSurfaceLayout,
} from "../../../api/types";
import { useCueRecording } from "../../../features/cueRecording/CueRecordingProvider";
import {
	useProgrammingCommandLineActions,
	useProgrammingCommandLineView,
	useProgrammingInteractionStatus,
} from "../../../features/programmingInteraction/ProgrammingInteractionView";
import {
	usePlaybackDeskView,
	usePlaybackProjectionMap,
	usePlaybackRuntimeActions,
	usePlaybackRuntimeStatus,
} from "../../../features/playbackRuntime/PlaybackRuntimeView";
import { usePlaybackTopologyActions } from "../../../features/playbackTopology/PlaybackTopologyProvider";
import { usePlaybackTopologyView } from "../../../features/playbackTopology/PlaybackTopologyView";
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
	pageNumber?: number;
	firstSlot?: number;
	count?: number;
	rows?: number;
	buttons?: number;
	playbackLayout?: PlaybackSurfaceLayout | null;
	hardwareConnected?: boolean;
}

export function usePlaybackBankController({
	pageNumber,
	firstSlot = 1,
	count,
	rows,
	buttons,
	playbackLayout,
	hardwareConnected = false,
}: PlaybackFaderBankProps) {
	const topology = usePlaybackTopologyView();
	const topologyActions = usePlaybackTopologyActions();
	const runtimeActions = usePlaybackRuntimeActions();
	const runtimeStatus = usePlaybackRuntimeStatus();
	const playbackDesk = usePlaybackDeskView();
	const commandLine = useProgrammingCommandLineView();
	const commandLineActions = useProgrammingCommandLineActions();
	const commandStatus = useProgrammingInteractionStatus();
	const cueRecording = useCueRecording();
	const { state, dispatch } = useApp();
	const hardware = Boolean(hardwareConnected || state.midiProfile);
	const pageSize = count ?? state.playbackColumns * state.playbackRows;
	const rowCount = playbackLayout?.rows.length ?? rows ?? state.playbackRows;
	const columns =
		playbackLayout?.playbacks_per_row ?? Math.ceil(pageSize / rowCount);
	const activePageNumber = pageNumber ?? playbackDesk?.active_page ?? null;
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
	useEffect(() => setConfiguration(null), [activePageNumber, topology.ready]);
	const heldActions = useMemo(
		() => new HeldPlaybackActions(runtimeActions),
		[runtimeActions],
	);
	useEffect(() => () => heldActions.releaseAll(), [heldActions]);
	const rowTracks = playbackLayout
		? playbackLayout.rows
				.map((row) => `minmax(0, ${playbackRowUnits(row, hardware)}fr)`)
				.join(" ")
		: `repeat(${rowCount}, minmax(0, 1fr))`;
	return {
		topology,
		topologyActions,
		runtimeActions,
		runtimeStatus,
		runtimeProjections: projectionAuthority.projections,
		playbackDesk,
		commandLineActions,
		cueRecording,
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
		assignmentPending: state.cueListSetTarget != null,
		selectionPending: /^SELECT\s*$/i.test(commandLine?.text ?? ""),
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
	const {
		topology,
		pageObject,
		playbackLayout,
		columns,
		firstSlot,
		pageSize,
	} = options;
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
			topology.playbacks,
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
		playbackDefinitions: options.topology.playbacks,
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
	enabled: boolean,
) {
	const numbers = useMemo(
		() =>
			slots.flatMap((slot) => (slot.playback ? [slot.playback.number] : [])),
		[slots],
	);
	const projections = usePlaybackProjectionMap(enabled ? numbers : []);
	const loaded = numbers.every((number) => projections.get(number) !== undefined);
	const matches = slots.every(
		({ playback }) =>
			!playback || projectionMatches(playback, projections.get(playback.number)),
	);
	const error =
		loaded && !matches
			? new Error(
					"Playback runtime authority does not match the visible topology",
				)
			: null;
	return { projections, loaded, matches, error };
}

function projectionMatches(
	playback: PlaybackDefinition,
	projection: PlaybackRuntimeProjection | undefined,
) {
	if (!projection || projection.playback_number !== playback.number) return false;
	const target = playback.target;
	if (target.type === "cue_list")
		return (
			projection.target === "cue_list" &&
			projection.cue_list_id === target.cue_list_id
		);
	if (target.type === "group")
		return (
			projection.target === "group" && projection.group_id === target.group_id
		);
	if (target.type === "speed_group")
		return (
			projection.target === "speed_group" && projection.group === target.group
		);
	return projection.target === target.type;
}
