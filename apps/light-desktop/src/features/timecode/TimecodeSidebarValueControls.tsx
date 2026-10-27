import type { TimecodeDefinition } from "../../api/types/timecode";
import type { TimecodeEditorSelection } from "./editorModel";
import { laneWithKeyframeValue } from "./timecodeEncoderSlots";
import { laneWithVolumeCurve, SelectedKeyframeValue } from "./TimecodeKeyframeActions";
import { MarkerActionStrip } from "./TimecodeMarkerActions";
import type { TimelineItem } from "./timecodeEditorShared";
import "./TimecodeSidebarValueControls.css";

/** The existing touch controls move with the inspector; their writers and keypad paths stay shared. */
export function KeyframeSidebarValue({ definition, selection, onCommit }: {
	definition: TimecodeDefinition;
	selection: TimecodeEditorSelection;
	onCommit(definition: TimecodeDefinition): void;
}) {
	if (!("laneId" in selection)) return null;
	const lane = definition.lanes.find((candidate) => candidate.id === selection.laneId);
	if (!lane) return null;
	const speed = lane.content.kind === "speed_group" && selection.kind === "speed"
		? lane.content.keyframes.find((item) => item.id === selection.itemId) : undefined;
	const volume = lane.content.kind === "audio_volume" && selection.kind === "volume"
		? lane.content.keyframes.find((item) => item.id === selection.itemId) : undefined;
	return <SelectedKeyframeValue speed={speed} volume={volume}
		onValue={(value) => onCommit(laneWithKeyframeValue(definition, lane.id, selection, value))}
		onEasing={(curve) => onCommit(laneWithVolumeCurve(definition, lane.id, selection.itemId, curve))} />;
}

export function MarkerSidebarActions({ definition, selection, frame, fps, items, onSelect, onCommit, onScrub }: {
	definition: TimecodeDefinition;
	selection: Extract<TimecodeEditorSelection, { kind: "marker" }>;
	frame?: number;
	fps: number;
	items: readonly TimelineItem[];
	onSelect(selection: TimecodeEditorSelection): void;
	onCommit(definition: TimecodeDefinition): void;
	onScrub?(frame: number): void;
}) {
	const marker = definition.markers.find((item) => item.id === selection.itemId);
	if (!marker) return null;
	return <div className="timecode-marker-sidebar"><MarkerActionStrip definition={definition} selection={selection}
		frame={frame ?? marker.frame} fps={fps} items={items} onCommit={onCommit}
		onSelection={(item) => { onSelect(item.selection); onScrub?.(item.frame); }} /></div>;
}
