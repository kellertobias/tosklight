import { useMemo, useState } from "react";
import type { TimecodeDefinition } from "../../api/types/timecode";
import { moveTimelineItem, timelineItems, type TimelineItem } from "./editorModel";

/** A Trigger drag previews geometry locally; only release invokes the timing writer. */
export function useClipTriggerPreview(
	definition: TimecodeDefinition,
	items: readonly TimelineItem[],
) {
	const [preview, setPreview] = useState<{
		laneId: string;
		clipId: string;
		frame: number;
	} | null>(null);
	const displayDefinition = useMemo(
		() => preview
			? moveTimelineItem(definition, {
				kind: "clip", laneId: preview.laneId, itemId: preview.clipId,
			}, preview.frame)
			: definition,
		[definition, preview],
	);
	const displayItems = useMemo(
		() => preview ? timelineItems(displayDefinition) : items,
		[displayDefinition, items, preview],
	);
	return {
		displayDefinition,
		displayItems,
		previewMoveClip(laneId: string, clipId: string, frame: number | null) {
			setPreview(frame === null ? null : { laneId, clipId, frame });
		},
	};
}
