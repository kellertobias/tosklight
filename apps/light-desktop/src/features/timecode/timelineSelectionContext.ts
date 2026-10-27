import type { TimecodeDefinition } from "../../api/types/timecode";
import { sameSelection, type TimecodeEditorSelection } from "./editorModel";
import type { TimelineItem } from "./timecodeEditorShared";

/** One selection model shared by sidebar, keyframe strip, and encoder deck. */
export function timelineSelectionContext(
	definition: TimecodeDefinition,
	items: readonly TimelineItem[],
	selection: TimecodeEditorSelection | null,
	activeLaneId: string | null,
) {
	const keyframeItems = items.filter(
		(item) =>
			item.laneId === activeLaneId &&
			(item.kind === "speed" || item.kind === "volume"),
	);
	const laneIndex = Math.max(
		0,
		definition.lanes.findIndex((lane) => lane.id === activeLaneId),
	);
	const keyframeIndex = Math.max(
		0,
		keyframeItems.findIndex((item) => sameSelection(item.selection, selection)),
	);
	const activeLane = definition.lanes.find((lane) => lane.id === activeLaneId);
	const activeKeyframe = keyframeItems.find((item) =>
		sameSelection(item.selection, selection),
	);
	const activeMarker =
		selection?.kind === "marker"
			? definition.markers.find((marker) => marker.id === selection.itemId)
			: undefined;
	const speedKeyframe =
		selection?.kind === "speed" && activeLane?.content.kind === "speed_group"
			? activeLane.content.keyframes.find(
					(keyframe) => keyframe.id === selection.itemId,
				)
			: undefined;
	const volumeKeyframe =
		selection?.kind === "volume" && activeLane?.content.kind === "audio_volume"
			? activeLane.content.keyframes.find(
					(keyframe) => keyframe.id === selection.itemId,
				)
			: undefined;
	return {
		keyframeItems,
		laneIndex,
		keyframeIndex,
		activeLane,
		activeKeyframe,
		activeMarker,
		speedKeyframe,
		volumeKeyframe,
	};
}
