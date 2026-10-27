import type {
	TimecodeCueListClipStatus,
	TimecodeLane,
} from "../../api/types/timecode";
import type { TimelineItem } from "./timecodeEditorShared";
/** Resolve status only for this exact Cuelist lane and clip, never a coincident item. */
export function cueStatusForItem(
	statuses: readonly TimecodeCueListClipStatus[],
	lane: TimecodeLane,
	item: TimelineItem,
	cueListId: string | null,
) {
	return lane.content.kind === "cue_list" && item.kind === "clip"
		? statuses.find(
				(status) =>
					status.lane_id === lane.id &&
					status.clip_id === item.selection.itemId &&
					status.cue_list_id === cueListId,
			)
		: undefined;
}
