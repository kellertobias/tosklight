import { useState } from "react";
import { useServer } from "../../../api/ServerContext";
import type { PlaybackSurfaceLayout } from "../../../api/types";
import { useApp } from "../../../state/AppContext";
import { playbackRowUnits, projectPlaybackSlots } from "./projection";
import type { PlaybackConfigurationState } from "./types";
import { useShowObjectView } from "../../../features/showObjects/ShowObjectsView";
import { useGroups } from "../../../features/server/useShowObjectsState";

export interface PlaybackFaderBankProps {
	pageNumber?: number;
	firstSlot?: number;
	count?: number;
	rows?: number;
	buttons?: number;
	playbackLayout?: PlaybackSurfaceLayout | null;
}

export function usePlaybackBankController({
	pageNumber,
	firstSlot = 1,
	count,
	rows,
	buttons,
	playbackLayout,
}: PlaybackFaderBankProps) {
	const server = useServer();
	const groups = useGroups(server.playbacks);
	const { state, dispatch } = useApp();
	const hardware = Boolean(
		server.bootstrap?.hardware_connected || state.midiProfile,
	);
	const pageSize = count ?? state.playbackColumns * state.playbackRows;
	const rowCount = playbackLayout?.rows.length ?? rows ?? state.playbackRows;
	const columns =
		playbackLayout?.playbacks_per_row ?? Math.ceil(pageSize / rowCount);
	const activePageNumber =
		pageNumber ?? server.playbacks?.active_page ?? state.playbackPage + 1;
	const page = server.playbacks?.pages.find(
		(candidate) => candidate.number === activePageNumber,
	);
	const [configuration, setConfiguration] =
		useState<PlaybackConfigurationState | null>(null);
	const assignmentPending = state.cueListSetTarget != null;
	const selectionPending = /^SELECT\s*$/i.test(server.commandLine);
	const slots = projectPlaybackSlots({
		server,
		groups,
		page,
		playbackLayout,
		columns,
		firstSlot,
		pageSize,
	});
	useShowObjectView(
		"group",
		slots.some((slot) => slot.playback?.target.type === "group"),
	);
	const rowTracks = playbackLayout
		? playbackLayout.rows
				.map((row) => `minmax(0, ${playbackRowUnits(row, hardware)}fr)`)
				.join(" ")
		: `repeat(${rowCount}, minmax(0, 1fr))`;
	return {
		server,
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
