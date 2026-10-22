import type { CueList } from "../../api/types";
import type { TimecodeCueListClip } from "../../api/types/timecode";
import { cueTransitionFrameBounds, type CueClipTimingDefaults, type CueClipTimingRow } from "./cueClipTiming";

/** Both the exact Trigger control and divider grips use the same authoritative writers. */
export function cueTriggerEditing(
	clip: TimecodeCueListClip,
	body: CueList,
	defaults: CueClipTimingDefaults,
	rows: CueClipTimingRow[],
	selectedCueId: string | null,
	onMoveClip?: (clipId: string, frame: number) => void,
	onPlaceCueStart?: (clipId: string, cueId: string, offset: number) => void,
) {
	const selected = rows.find((row) => row.cue.id === selectedCueId);
	const first = Boolean(selected && selected === rows[0]);
	const triggerBounds = selected?.cue.id
		? cueTransitionFrameBounds(clip, body, defaults, selected.cue.id)
		: null;
	const placeRow = (row: CueClipTimingRow, isFirst: boolean, frame: number) => {
		if (!row.cue.id) return;
		if (isFirst) {
			onMoveClip?.(clip.id, Math.max(0, frame));
			return;
		}
		const bounds = cueTransitionFrameBounds(clip, body, defaults, row.cue.id);
		if (!bounds) return;
		onPlaceCueStart?.(clip.id, row.cue.id,
			Math.max(bounds.minimum, Math.min(bounds.maximum, frame)) - clip.start_frame);
	};
	return {
		selected, first, triggerBounds, placeRow,
		canMoveTrigger: Boolean(selected && (first ? onMoveClip : triggerBounds && onPlaceCueStart)),
		placeTrigger(frame: number) { if (selected) placeRow(selected, first, frame); },
	};
}
