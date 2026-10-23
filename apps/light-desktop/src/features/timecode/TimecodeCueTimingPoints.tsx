import { redirectCueVertexPointer } from "./cueVertexPointer";
import { useCueReferenceScroll } from "./useCueReferenceScroll";
import { HEADER, PEAK_Y, BASE_Y } from "./cueEditorLayout";
import type { PointerEvent as ReactPointerEvent } from "react";
import type { CueList } from "../../api/types";
import type { TimecodeCueListClip } from "../../api/types/timecode";
import {
	type CueClipTimingRow,
	type CueFadeKind,
	type CueFadeEdge,
	type CueTimingFrameBounds,
	cueFadeEditFrameBounds,
	type CueClipTimingDefaults,
	TIMECODE_FPS,
} from "./cueClipTiming";
import { FrameHandle } from "./TimecodeCueControls";
import { formatFrame } from "./timecodeEditorShared";
interface CueTimingPointsProps {
	selected: CueClipTimingRow;
	cueList: CueList;
	timingDefaults: CueClipTimingDefaults;
	clip: TimecodeCueListClip;
	position(frame: number): number;
	pixelsPerFrame: number;
	triggerPreview: number | null;
	first: boolean;
	triggerBounds: CueTimingFrameBounds | null;
	canMoveTrigger: boolean;
	setTriggerPreview(frame: number | null): void;
	placeTrigger(frame: number): void;
	saving: boolean;
	editable: boolean;
	dividerFrames?: readonly number[];
	begin(
		event: ReactPointerEvent,
		row: CueClipTimingRow,
		kind: CueFadeKind,
		edge: CueFadeEdge,
	): void;
	commit(
		row: CueClipTimingRow,
		kind: CueFadeKind,
		edge: CueFadeEdge,
		frame: number,
	): void;
}

/** Place touch targets without moving any time anchors or changing the lane height. */
function pointPlacement(
	selected: CueClipTimingRow,
	kind: CueFadeKind,
	edge: CueFadeEdge,
	pixelsPerFrame: number,
	dividerFrames: readonly number[],
	clip: TimecodeCueListClip,
	referenceScroll: number,
) {
	const anchorTop = (kind === "in") === (edge === "start") ? BASE_Y : PEAK_Y;
	const occupied: Array<{ x: number; y: number; target: number }> = [];
	const positions = new Map<string, number>();
	for (const [handleKind, handleEdge] of [
		["in", "end"],
		["out", "start"],
		["in", "start"],
		["out", "end"],
	] as const) {
		const range = handleKind === "in" ? selected.inFade : selected.outFade;
		const frame = handleEdge === "start" ? range.startFrame : range.endFrame;
		const isReference = handleKind === "out" && selected.outFadeReference;
		const x = isReference
			? (clip.start_frame +
					(clip.end_frame - clip.start_frame) / 2 +
					frame -
					selected.handoverFrame) *
					pixelsPerFrame +
				22 -
				referenceScroll
			: frame * pixelsPerFrame;
		const y = isReference
			? HEADER + (handleEdge === "start" ? 22 : 66)
			: (handleKind === "in") === (handleEdge === "start")
				? BASE_Y
				: PEAK_Y;
		// Only truly coincident corners need a separate labelled touch affordance.
		// Nearby ordinary vertices remain exactly on their fade slopes.
		let offset = 0;
		while (
			occupied.some(
				(point) =>
					Math.abs(point.target - (x + offset)) < 1 &&
					Math.abs(point.y - y) < 1,
			)
		)
			offset += 48;
		occupied.push({ x, y, target: x + offset });
		positions.set(`${handleKind}:${handleEdge}`, offset);
	}
	const offset = positions.get(`${kind}:${edge}`) ?? 0;
	const reference = kind === "out" && selected.outFadeReference;
	const trueTop = reference ? anchorTop - HEADER : anchorTop;
	const pointTop = reference ? (edge === "start" ? 22 : 66) : anchorTop;
	return {
		anchorTop: trueTop,
		pointTop,
		offset,
		displaced: offset !== 0 || pointTop !== trueTop,
	};
}

export function CueTimingPoints({
	selected,
	cueList,
	timingDefaults,
	clip,
	position,
	pixelsPerFrame,
	triggerPreview,
	first,
	triggerBounds,
	canMoveTrigger,
	setTriggerPreview,
	placeTrigger,
	saving,
	editable,
	dividerFrames = [selected.startFrame],
	begin,
	commit,
}: CueTimingPointsProps) {
	const { referenceRail, referenceScroll, onReferenceScroll } =
		useCueReferenceScroll(clip.id, selected.cue.id);
	return (
		<span
			className="timecode-cue-edit-points"
			onPointerDownCapture={(event) =>
				redirectCueVertexPointer(event, selected, begin)
			}
		>
			<CueTriggerPoint
				{...{
					selected,
					triggerPreview,
					first,
					triggerBounds,
					canMoveTrigger,
					setTriggerPreview,
					placeTrigger,
					position,
					clip,
					pixelsPerFrame,
				}}
			/>
			{(["in", "out"] as const).map((kind) => {
				const range = kind === "in" ? selected.inFade : selected.outFade;
				const reference = kind === "out" && selected.outFadeReference;
				const anchor = reference ? selected.handoverFrame : clip.start_frame;
				const x = (frame: number) =>
					(frame - anchor) * pixelsPerFrame + (reference ? 22 : 0);
				return (
					<span
						key={kind}
						className={`timecode-cue-edit-rail ${kind} ${reference ? "reference" : ""}`}
						ref={reference ? referenceRail : undefined}
						onScroll={reference ? onReferenceScroll : undefined}
						data-reference-scroll={reference ? referenceScroll : undefined}
					>
						<span
							className="timecode-cue-reference-axis"
							style={
								reference
									? {
											width: Math.max(
												(clip.end_frame - clip.start_frame) * pixelsPerFrame,
												x(range.endFrame) + 44,
											),
										}
									: undefined
							}
						>
							<small>
								{reference
									? "Out · not executed"
									: kind === "in"
										? "In"
										: "Out"}
							</small>
							<span
								className="timecode-cue-edit-line"
								aria-hidden="true"
								style={{
									left: x(range.startFrame),
									width: Math.max(
										2,
										(range.endFrame - range.startFrame) * pixelsPerFrame,
									),
								}}
							/>
							{(["start", "end"] as const).map((edge) => {
								const frame =
									edge === "start" ? range.startFrame : range.endFrame;
								const bounds = cueFadeEditFrameBounds(
									clip,
									cueList,
									timingDefaults,
									selected,
									kind,
									edge,
								);
								const { anchorTop, pointTop, offset, displaced } =
									pointPlacement(
										selected,
										kind,
										edge,
										pixelsPerFrame,
										dividerFrames,
										clip,
										referenceScroll,
									);

								return (
									<FrameHandle
										key={edge}
										label={`Cue ${selected.cue.number} ${kind === "in" ? "In" : "Out"} ${edge === "start" ? "delay" : "fade"}`}
										frame={frame}
										minimum={bounds?.minimum ?? frame}
										maximum={bounds?.maximum ?? frame}
										left={x(frame) + offset}
										anchorLeft={x(frame)}
										top={pointTop}
										anchorTop={anchorTop}
										disabled={
											saving ||
											!editable ||
											!bounds ||
											Boolean(selected.diagnostic)
										}
										onBegin={(event) => begin(event, selected, kind, edge)}
										onCommit={(target) => commit(selected, kind, edge, target)}
										className={`${kind} ${edge} anchored-vertex ${displaced ? "displaced" : ""}`}
										anchorVisual
										caption={`${kind === "in" ? "In" : "Out"} ${edge === "start" ? "start" : "end"}`}
									/>
								);
							})}
						</span>
					</span>
				);
			})}
		</span>
	);
}

function CueTriggerPoint({
	selected,
	triggerPreview,
	first,
	triggerBounds,
	canMoveTrigger,
	setTriggerPreview,
	placeTrigger,
	position,
	clip,
	pixelsPerFrame,
}: Pick<
	CueTimingPointsProps,
	| "selected"
	| "triggerPreview"
	| "first"
	| "triggerBounds"
	| "canMoveTrigger"
	| "setTriggerPreview"
	| "placeTrigger"
	| "position"
	| "clip"
	| "pixelsPerFrame"
>) {
	return (
		<>
			{" "}
			<span
				className="timecode-cue-trigger-label"
				hidden={(clip.end_frame - clip.start_frame) * pixelsPerFrame < 220}
			>
				Trigger ·{" "}
				{formatFrame(triggerPreview ?? selected.startFrame, TIMECODE_FPS)}
			</span>
			<FrameHandle
				label={`Cue ${selected.cue.number} Trigger`}
				caption="Trigger"
				frame={triggerPreview ?? selected.startFrame}
				minimum={first ? 0 : (triggerBounds?.minimum ?? selected.startFrame)}
				maximum={
					first
						? Number.MAX_SAFE_INTEGER
						: (triggerBounds?.maximum ?? selected.startFrame)
				}
				left={position(triggerPreview ?? selected.startFrame)}
				top={8}
				disabled={!canMoveTrigger}
				onPreview={setTriggerPreview}
				onCommit={placeTrigger}
				className="trigger"
			/>
		</>
	);
}
