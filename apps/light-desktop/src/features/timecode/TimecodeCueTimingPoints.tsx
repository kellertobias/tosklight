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
	begin,
	commit,
}: CueTimingPointsProps) {
	return (
		<span className="timecode-cue-edit-points">
			<span
				className="timecode-cue-trigger-label"
				style={{ left: position(triggerPreview ?? selected.startFrame) }}
			>
				Trigger ·{" "}
				{formatFrame(triggerPreview ?? selected.startFrame, TIMECODE_FPS)}
			</span>
			<FrameHandle
				label={`Cue ${selected.cue.number} Trigger`}
				frame={triggerPreview ?? selected.startFrame}
				minimum={first ? 0 : (triggerBounds?.minimum ?? selected.startFrame)}
				maximum={
					first
						? Number.MAX_SAFE_INTEGER
						: (triggerBounds?.maximum ?? selected.startFrame)
				}
				left={position(triggerPreview ?? selected.startFrame)}
				top={162}
				disabled={!canMoveTrigger}
				onPreview={setTriggerPreview}
				onCommit={placeTrigger}
				className="trigger"
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
									? "Cue Out · next handover reference"
									: kind === "in"
										? "In"
										: "Out"}
							</small>
							<span
								className="timecode-cue-edit-line"
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
								return (
									<FrameHandle
										key={edge}
										label={`Cue ${selected.cue.number} ${kind === "in" ? "In" : "Out"} ${edge === "start" ? "delay" : "fade"}`}
										frame={frame}
										minimum={bounds?.minimum ?? frame}
										maximum={bounds?.maximum ?? frame}
										left={x(frame)}
										top={
											range.startFrame === range.endFrame
												? edge === "start"
													? 10
													: 42
												: 24
										}
										disabled={
											saving ||
											!editable ||
											!bounds ||
											Boolean(selected.diagnostic)
										}
										onBegin={(event) => begin(event, selected, kind, edge)}
										onCommit={(target) => commit(selected, kind, edge, target)}
										className={`${kind} ${edge}`}
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
