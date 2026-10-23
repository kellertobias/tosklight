import type { TimecodeDefinition } from "../../api/types/timecode";
import type { TimecodeEditorSelection } from "./editorModel";

export function dragItemSignature(
	definition: TimecodeDefinition,
	selection: TimecodeEditorSelection,
) {
	if (selection.kind === "marker")
		return JSON.stringify(
			definition.markers.find((item) => item.id === selection.itemId),
		);
	const content = definition.lanes.find(
		(lane) => lane.id === selection.laneId,
	)?.content;
	return JSON.stringify(
		content &&
			("clips" in content ? content.clips : content.keyframes).find(
				(item) => item.id === selection.itemId,
			),
	);
}
export function setDragVolume(
	definition: TimecodeDefinition,
	selection: TimecodeEditorSelection,
	value: number,
): TimecodeDefinition {
	if (selection.kind !== "volume") return definition;
	return {
		...definition,
		lanes: definition.lanes.map((lane) =>
			lane.id !== selection.laneId || lane.content.kind !== "audio_volume"
				? lane
				: {
						...lane,
						content: {
							...lane.content,
							keyframes: lane.content.keyframes.map((item) =>
								item.id === selection.itemId ? { ...item, value } : item,
							),
						},
					},
		),
	};
}
/** Cancel only the item being dragged, preserving unrelated authoritative changes. */
export function restoreDragItem(
	current: TimecodeDefinition,
	original: TimecodeDefinition,
	selection: TimecodeEditorSelection,
): TimecodeDefinition {
	const restore = <T extends { id: string }>(
		items: readonly T[],
		before: readonly T[],
	) =>
		items.map((item) =>
			item.id === selection.itemId
				? (before.find((old) => old.id === item.id) ?? item)
				: item,
		);
	if (selection.kind === "marker")
		return { ...current, markers: restore(current.markers, original.markers) };
	const before = original.lanes.find(
		(lane) => lane.id === selection.laneId,
	)?.content;
	return {
		...current,
		lanes: current.lanes.map((lane) => {
			if (lane.id !== selection.laneId || !before) return lane;
			const content = lane.content;
			switch (content.kind) {
				case "cue_list":
					return before.kind === "cue_list"
						? {
								...lane,
								content: {
									...content,
									clips: restore(content.clips, before.clips),
								},
							}
						: lane;
				case "audio_player":
					return before.kind === "audio_player"
						? {
								...lane,
								content: {
									...content,
									clips: restore(content.clips, before.clips),
								},
							}
						: lane;
				case "audio_volume":
					return before.kind === "audio_volume"
						? {
								...lane,
								content: {
									...content,
									keyframes: restore(content.keyframes, before.keyframes),
								},
							}
						: lane;
				case "speed_group":
					return before.kind === "speed_group"
						? {
								...lane,
								content: {
									...content,
									keyframes: restore(content.keyframes, before.keyframes),
								},
							}
						: lane;
			}
		}),
	};
}
