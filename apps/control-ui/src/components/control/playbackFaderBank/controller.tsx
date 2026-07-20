import { useState } from "react";
import { useServer } from "../../../api/ServerContext";
import type { PlaybackSurfaceLayout } from "../../../api/types";
import { usePlaybackDeskView } from "../../../features/playbackRuntime/PlaybackRuntimeView";
import { useCueRecording } from "../../../features/cueRecording/CueRecordingProvider";
import { useGroups } from "../../../features/server/useShowObjectsState";
import {
	useCueLists,
	usePlaybackDefinitions,
	usePlaybackPages,
} from "../../../features/showObjects/ShowObjectsState";
import { useShowObjectKindsView } from "../../../features/showObjects/ShowObjectsView";
import { useApp } from "../../../state/AppContext";
import { playbackRowUnits, projectPlaybackSlots } from "./projection";
import type { PlaybackConfigurationState } from "./types";
import { useCommandLineSurface } from "../commandLine/useCommandLineSurface";

export interface PlaybackFaderBankProps {
	pageNumber?: number;
	firstSlot?: number;
	count?: number;
	rows?: number;
	buttons?: number;
	playbackLayout?: PlaybackSurfaceLayout | null;
}

const PLAYBACK_KINDS = ["cue_list", "playback", "playback_page"] as const;
const PLAYBACK_AND_GROUP_KINDS = [...PLAYBACK_KINDS, "group"] as const;

export function usePlaybackBankController({
	pageNumber,
	firstSlot = 1,
	count,
	rows,
	buttons,
	playbackLayout,
}: PlaybackFaderBankProps) {
	const server = useServer();
	const command = useCommandLineSurface();
	const cueRecording = useCueRecording();
	const groups = useGroups(server.playbacks);
	const cueLists = useCueLists();
	const playbackDefinitions = usePlaybackDefinitions();
	const playbackPages = usePlaybackPages();
	const { state, dispatch } = useApp();
	const playbackDesk = usePlaybackDeskView();
	const hardware = Boolean(
		server.bootstrap?.hardware_connected || state.midiProfile,
	);
	const pageSize = count ?? state.playbackColumns * state.playbackRows;
	const rowCount = playbackLayout?.rows.length ?? rows ?? state.playbackRows;
	const columns =
		playbackLayout?.playbacks_per_row ?? Math.ceil(pageSize / rowCount);
	const activePageNumber =
		pageNumber ??
		playbackDesk?.active_page ??
		server.playbacks?.active_page ??
		state.playbackPage + 1;
	const page = playbackPages.find(
		(candidate) => candidate.body.number === activePageNumber,
	)?.body;
	const [configuration, setConfiguration] =
		useState<PlaybackConfigurationState | null>(null);
	const assignmentPending = state.cueListSetTarget != null;
	const selectionPending = /^SELECT\s*$/i.test(command.text);
	const slots = projectPlaybackSlots({
		cueLists,
		playbackDefinitions,
		groups,
		page,
		playbackLayout,
		columns,
		firstSlot,
		pageSize,
	});
	useShowObjectKindsView(
		slots.some((slot) => slot.playback?.target.type === "group")
			? PLAYBACK_AND_GROUP_KINDS
			: PLAYBACK_KINDS,
	);
	const rowTracks = playbackLayout
		? playbackLayout.rows
				.map((row) => `minmax(0, ${playbackRowUnits(row, hardware)}fr)`)
				.join(" ")
		: `repeat(${rowCount}, minmax(0, 1fr))`;
	return {
		server,
		command,
		cueRecording,
		cueLists,
		playbackDesk,
		state,
		dispatch,
		hardware,
		pageNumber,
		buttons,
		rowCount,
		columns,
		activePageNumber,
		page,
		configuration,
		setConfiguration,
		assignmentPending,
		selectionPending,
		slots,
		rowTracks,
	};
}

export type PlaybackBankController = ReturnType<
	typeof usePlaybackBankController
>;
