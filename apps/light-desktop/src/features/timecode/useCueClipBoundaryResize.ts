import { useCallback } from "react";
import type { CueList } from "../../api/types";
import type { TimecodeDefinition } from "../../api/types/timecode";
import type { ResizeClipBoundary } from "./CueClipBoundaryContext";
import { cueListClipScale, resizeTimelineClip } from "./editorModel";
import { scaleClipCueTimings } from "./TimecodeKeyframeActions";
import type { TimecodeCueListOption } from "./timecodeEditorShared";

/** Exact boundary edits share edge-drag resizing, Cuelist authority and compound history. */
export function useCueClipBoundaryResize({ definition, cueLists, onBeginGesture, onPreview,
	onEndGesture, onSaveCueList, onCueTimingError,
}: {
	definition: TimecodeDefinition;
	cueLists: readonly TimecodeCueListOption[];
	onBeginGesture(): void;
	onPreview(value: TimecodeDefinition): void;
	onEndGesture(): void;
	onSaveCueList?(id: string, body: CueList): Promise<CueList>;
	onCueTimingError?(message: string): void;
}): ResizeClipBoundary {
	return useCallback((chosen, edge, target) => {
		const next = resizeTimelineClip(definition, chosen, edge, target);
		const beforeLane = definition.lanes.find((lane) => lane.id === chosen.laneId);
		const afterLane = next.lanes.find((lane) => lane.id === chosen.laneId);
		if (beforeLane?.content.kind !== "cue_list" || afterLane?.content.kind !== "cue_list") return;
		const before = beforeLane.content.clips.find((clip) => clip.id === chosen.itemId);
		const after = afterLane.content.clips.find((clip) => clip.id === chosen.itemId);
		if (!before || !after || (before.start_frame === after.start_frame && before.end_frame === after.end_frame)) return;
		onBeginGesture();
		onPreview(next);
		scaleClipCueTimings({ definition, cueLists, selection: chosen,
			ratio: cueListClipScale(before, after.start_frame, after.end_frame), onSaveCueList, onCueTimingError });
		onEndGesture();
	}, [definition, cueLists, onBeginGesture, onPreview, onEndGesture, onSaveCueList, onCueTimingError]);
}
