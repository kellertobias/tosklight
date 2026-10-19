import type {
	PlaybackPage,
	PlaybackSurfaceLayout,
	PlaybackSurfaceRow,
} from "../../../api/types";
import type { ShowObject } from "../../../features/showObjects/contracts";
import { playbackSlotAssignments } from "../playbackProjection";
import { projectPlaybackFootprints } from "./footprints";
import type { PlaybackGroup, PlaybackSlotProjection } from "./types";

export function playbackRowUnits(row: PlaybackSurfaceRow, hardware: boolean) {
	if (hardware) return row.presentation !== "buttons" ? 2 : 1;
	return row.presentation !== "buttons" ? 4 : row.button_count > 1 ? 2 : 1;
}

export function projectPlaybackSlots({
	cueLists,
	groups,
	page,
	playbackLayout,
	columns,
	firstSlot,
	pageSize,
	ignoreFootprints = false,
}: {
	cueLists: readonly ShowObject<"cue_list">[];
	groups: readonly PlaybackGroup[];
	page: PlaybackPage | undefined;
	playbackLayout: PlaybackSurfaceLayout | null | undefined;
	columns: number;
	firstSlot: number;
	pageSize: number;
	ignoreFootprints?: boolean;
}): PlaybackSlotProjection[] {
	const cells = playbackLayout
		? playbackLayout.rows.flatMap((row, rowIndex) =>
				Array.from({ length: row.count }, (_, columnIndex) => ({
					slot: row.first_position + columnIndex,
					row,
					rowIndex,
					columnIndex,
				})),
			)
		: playbackSlotAssignments(page, firstSlot, pageSize).map((_, index) => ({
				slot: firstSlot + index,
				row: null,
				rowIndex: Math.floor(index / columns),
				columnIndex: index % columns,
			}));
	const footprints = ignoreFootprints ? { cells: [] } : projectPlaybackFootprints({
		page,
		playbackLayout,
		columns,
		firstSlot,
		pageSize,
	});
	const footprintByCell = new Map(
		footprints.cells.map((footprint) => [
			playbackCellKey(footprint),
			footprint,
		]),
	);
	return cells.flatMap(({ slot, row, rowIndex, columnIndex }) => {
		const footprint = footprintByCell.get(
			playbackCellKey({ rowIndex, columnIndex }),
		);
		if (footprint?.state === "claimed") return [];
		const definition = page?.assignments[String(slot)];
		const playback =
			definition && page
				? { ...definition, address: { page: page.number, position: slot } }
				: null;
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
		return [
			{
				playback,
				cue,
				group,
				slot,
				row,
				rowIndex,
				columnIndex,
				footprint: footprint?.state === "anchor" ? footprint : null,
			},
		];
	});
}

/** Every displayed cell owns a row/column, including holes in sparse show pages. */
export function playbackCellGridStyle(cell: PlaybackSlotProjection) {
	const column = cell.columnIndex;
	return {
		gridRow: `${cell.rowIndex + 1} / span 1`,
		gridColumn: `${column + 1} / span 1`,
	};
}

export function playbackCellKey(cell: {
	rowIndex: number;
	columnIndex: number;
}) {
	return `${cell.rowIndex}:${cell.columnIndex}`;
}
