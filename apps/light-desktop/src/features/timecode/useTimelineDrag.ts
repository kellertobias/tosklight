import {
	useEffect,
	useRef,
	type PointerEvent as ReactPointerEvent,
} from "react";
import type {
	TimecodeCueListClip,
	TimecodeDefinition,
} from "../../api/types/timecode";
import { volumeDragTravel } from "./audioLaneGeometry";
import {
	cueListClipScale,
	moveTimelineItem,
	resizeTimelineClip,
	snapTimelineFrame,
	type TimecodeEditorSelection,
} from "./editorModel";
import {
	dragItemSignature,
	restoreDragItem,
	setDragVolume,
} from "./timelineDragModel";
const DRAG_THRESHOLD = 3;
interface DragState {
	selection: TimecodeEditorSelection;
	pointerId?: number;
	startX: number;
	startY: number;
	startFrame: number;
	clipEdge?: "start" | "end";
	startVolume?: number;
	volumeTravelPx: number;
	scale: number;
	original: TimecodeDefinition;
	originalClip?: TimecodeCueListClip;
	started: boolean;
}
interface Options {
	definition: TimecodeDefinition;
	pixelsPerFrame: number;
	onPreview(value: TimecodeDefinition): void;
	onBeginGesture(): void;
	onEndGesture(): void;
	setSelection(value: TimecodeEditorSelection): void;
	onScaleCueTimings?(
		selection: TimecodeEditorSelection & { kind: "clip" },
		ratio: number,
	): void;
}
export function useTimelineDrag(options: Options) {
	const latest = useRef(options);
	latest.current = options;
	const drag = useRef<DragState | null>(null);
	useEffect(() => {
		const move = (event: PointerEvent) => {
			const active = drag.current;
			if (
				!active ||
				(active.pointerId !== undefined && event.pointerId !== active.pointerId)
			)
				return;
			const dx = event.clientX - active.startX,
				dy = event.clientY - active.startY;
			if (
				!active.started &&
				Math.max(
					Math.abs(dx),
					active.startVolume === undefined ? 0 : Math.abs(dy),
				) < DRAG_THRESHOLD
			)
				return;
			const current = latest.current;
			const frame =
				Math.abs(dx) < DRAG_THRESHOLD
					? active.startFrame
					: snapTimelineFrame(
							active.startFrame + dx / active.scale,
							current.definition,
							active.scale,
							12,
							{
								selection: active.selection,
								movingClip:
									active.selection.kind === "clip" &&
									active.clipEdge === undefined,
							},
						);
			let next =
				active.clipEdge && active.selection.kind === "clip"
					? resizeTimelineClip(
							current.definition,
							{ ...active.selection, kind: "clip" },
							active.clipEdge,
							frame,
						)
					: moveTimelineItem(current.definition, active.selection, frame);
			if (active.startVolume !== undefined)
				next = setDragVolume(
					next,
					active.selection,
					Math.max(
						0,
						Math.min(1, active.startVolume - dy / active.volumeTravelPx),
					),
				);
			if (
				dragItemSignature(next, active.selection) ===
				dragItemSignature(current.definition, active.selection)
			)
				return;
			if (!active.started) {
				active.started = true;
				current.onBeginGesture();
			}
			latest.current = { ...current, definition: next };
			current.onPreview(next);
		};
		const finish = (event: PointerEvent, cancel = false) => {
			const active = drag.current;
			if (
				!active ||
				(active.pointerId !== undefined && event.pointerId !== active.pointerId)
			)
				return;
			drag.current = null;
			const current = latest.current;
			if (active.started) {
				if (cancel)
					current.onPreview(
						restoreDragItem(
							current.definition,
							active.original,
							active.selection,
						),
					);
				else if (
					active.clipEdge &&
					active.selection.kind === "clip" &&
					active.originalClip
				) {
					const clip = cueListClipOf(current.definition, active.selection);
					if (clip) {
						const ratio = cueListClipScale(
							active.originalClip,
							clip.start_frame,
							clip.end_frame,
						);
						if (ratio !== 1)
							current.onScaleCueTimings?.({ ...active.selection, kind: "clip" }, ratio);
					}
				}
				current.onEndGesture();
			}
			if (!cancel) current.setSelection(active.selection);
		};
		const up = (event: PointerEvent) => finish(event);
		const cancel = (event: PointerEvent) => finish(event, true);
		window.addEventListener("pointermove", move);
		window.addEventListener("pointerup", up);
		window.addEventListener("pointercancel", cancel);
		return () => {
			window.removeEventListener("pointermove", move);
			window.removeEventListener("pointerup", up);
			window.removeEventListener("pointercancel", cancel);
		};
	}, []);
	return (
		event: ReactPointerEvent,
		selection: TimecodeEditorSelection,
		startFrame: number,
		clipEdge?: "start" | "end",
		startVolume?: number,
	) => {
		event.preventDefault();
		drag.current = {
			selection,
			pointerId: event.pointerId,
			startX: event.clientX,
			startY: event.clientY,
			startFrame,
			clipEdge,
			startVolume,
			volumeTravelPx: volumeDragTravel(
				event.currentTarget.closest?.<HTMLElement>(".timecode-editor-lane")
					?.clientHeight ?? 0,
			),
			scale: options.pixelsPerFrame,
			original: options.definition,
			originalClip: clipEdge
				? cueListClipOf(options.definition, selection)
				: undefined,
			started: false,
		};
		event.currentTarget.setPointerCapture?.(event.pointerId);
	};
}
function cueListClipOf(
	definition: TimecodeDefinition,
	selection: TimecodeEditorSelection,
) {
	if (selection.kind !== "clip") return undefined;
	const content = definition.lanes.find(
		(lane) => lane.id === selection.laneId,
	)?.content;
	return content?.kind === "cue_list"
		? content.clips.find((clip) => clip.id === selection.itemId)
		: undefined;
}
