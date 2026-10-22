import { useMemo, useState } from "react";
import type { CueList } from "../../api/types";
import type { TimecodeDefinition } from "../../api/types/timecode";
import type { CueClipTimingDefaults } from "./cueClipTiming";
import { timelineItems, type TimelineItem } from "./editorModel";
import { cueClipEndBounds, extendCueClipEnd } from "./extendCueClipEnd";
import { useClipTriggerPreview } from "./useClipTriggerPreview";

/** Trigger and ending gestures share local geometry; neither writes until its release. */
export function useCueLaneView(
	definition: TimecodeDefinition,
	items: readonly TimelineItem[],
	laneId: string,
	cueList: CueList | undefined,
	timingDefaults: CueClipTimingDefaults,
) {
	const trigger = useClipTriggerPreview(definition, items);
	const [endPreview, setEndPreview] = useState<{
		clipId: string;
		frame: number;
	} | null>(null);
	const context = useMemo(
		() => (cueList ? { cueList, timingDefaults } : undefined),
		[cueList, timingDefaults],
	);
	const displayDefinition = useMemo(
		() =>
			endPreview
				? extendCueClipEnd(
						trigger.displayDefinition,
						laneId,
						endPreview.clipId,
						endPreview.frame,
						context,
					)
				: trigger.displayDefinition,
		[trigger.displayDefinition, laneId, endPreview, context],
	);
	const displayItems = useMemo(
		() =>
			endPreview ? timelineItems(displayDefinition) : trigger.displayItems,
		[displayDefinition, endPreview, trigger.displayItems],
	);
	return {
		displayDefinition,
		displayItems,
		previewMoveClip: trigger.previewMoveClip,
		previewClipEnd(clipId: string, frame: number | null) {
			setEndPreview(frame === null ? null : { clipId, frame });
		},
		boundsForClip(clipId: string) {
			return cueClipEndBounds(
				trigger.displayDefinition,
				laneId,
				clipId,
				context,
			);
		},
	};
}
