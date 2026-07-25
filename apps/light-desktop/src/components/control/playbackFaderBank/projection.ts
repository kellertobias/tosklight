import type {
	CueList,
	PlaybackDefinition,
	PlaybackPage,
	PlaybackSurfaceLayout,
	PlaybackSurfaceRow,
} from "../../../api/types";
import { playbackSlotNumbers } from "../playbackProjection";
import type { PlaybackGroup, PlaybackSlotProjection } from "./types";
import type { ShowObject } from "../../../features/showObjects/contracts";

export function playbackRowUnits(row: PlaybackSurfaceRow, hardware: boolean) {
	if (hardware) return row.has_fader ? 2 : 1;
	return row.has_fader ? 4 : row.button_count > 1 ? 2 : 1;
}

export function projectPlaybackSlots({
	cueLists,
	playbackDefinitions,
	groups,
	page,
	playbackLayout,
	columns,
	firstSlot,
	pageSize,
}: {
	cueLists: readonly ShowObject<"cue_list">[];
	playbackDefinitions: readonly ShowObject<"playback">[];
	groups: readonly PlaybackGroup[];
	page: PlaybackPage | undefined;
	playbackLayout: PlaybackSurfaceLayout | null | undefined;
	columns: number;
	firstSlot: number;
	pageSize: number;
}): PlaybackSlotProjection[] {
	const cells = playbackLayout
		? playbackLayout.rows.flatMap((row, rowIndex) =>
				Array.from({ length: columns }, (_, columnIndex) => ({
					slot: row.first_playback_slot + columnIndex,
					row,
					rowIndex,
				})),
			)
		: playbackSlotNumbers(page, firstSlot, pageSize).map((_, index) => ({
				slot: firstSlot + index,
				row: null,
				rowIndex: Math.floor(index / columns),
			}));
	return cells.map(({ slot, row, rowIndex }) => {
		const number = page?.slots[String(slot)];
		const playback =
			playbackDefinitions.find(
				(candidate) => candidate.body.number === number,
			)?.body ??
			null;
		const cueListId =
			playback?.target.type === "cue_list" ? playback.target.cue_list_id : null;
		const cue = cueListId
			? (cueLists.find((candidate) => candidate.body.id === cueListId)?.body ??
				null)
			: null;
		const groupId =
			playback?.target.type === "group" ? playback.target.group_id : null;
		const group = groupId
			? (groups.find((candidate) => candidate.id === groupId) ?? null)
			: null;
		return { playback, cue, group, slot, row, rowIndex };
	});
}
