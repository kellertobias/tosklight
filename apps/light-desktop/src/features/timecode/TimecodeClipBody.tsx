import type { PointerEvent as ReactPointerEvent } from "react";
import type { CueList } from "../../api/types";
import type { TimecodeCueListClip } from "../../api/types/timecode";
import type { TimecodeEditorSelection } from "./editorModel";
import type { TimelineItem } from "./editorModel";
import { ClipFadeHandles } from "./TimecodeClipFadeHandles";
import { CueListClipContents } from "./TimecodeCueClipContents";
import type { CueClipTimingDefaults } from "./cueClipTiming";
import type { TimecodeCueListOption } from "./timecodeEditorShared";

export type { ClipFadeKind } from "./TimecodeClipFadeHandles";

/// Everything an operator can grab inside a Cuelist clip.
///
/// The clip's own drag handle, its level envelope, the Cue timings it holds and the scale grips
/// on either edge. It is one component because they share the clip they act on, and because the
/// clip body is what makes a Cuelist clip different from every other timeline item.
export function CueListClipBody({
	item,
	clip,
	cueList,
	timingDefaults,
	pixelsPerFrame,
	startVolume,
	startDrag,
	onSaveCueList,
	onPlaceCueStart,
	onSetClipFade,
	onCueTimingError,
}: {
	item: TimelineItem;
	clip: TimecodeCueListClip;
	cueList?: TimecodeCueListOption;
	timingDefaults?: CueClipTimingDefaults;
	pixelsPerFrame: number;
	startVolume?: number;
	startDrag(
		event: ReactPointerEvent,
		selection: TimecodeEditorSelection,
		frame: number,
		clipEdge?: "start" | "end",
		startVolume?: number,
	): void;
	onSaveCueList?(cueListId: string, body: CueList): Promise<CueList>;
	onPlaceCueStart?(clipId: string, cueId: string, offsetFrame: number): void;
	onSetClipFade?(
		clipId: string,
		kind: import("./TimecodeClipFadeHandles").ClipFadeKind,
		frames: number,
	): void;
	onCueTimingError?(message: string): void;
}) {
	return (
		<>
			<span
				className="timecode-clip-handle"
				onPointerDown={(event) => {
					event.stopPropagation();
					startDrag(event, item.selection, item.frame, undefined, startVolume);
				}}
			>
				<b>{item.label}</b>
				{cueList?.number !== undefined && <small>{cueList.number}</small>}
			</span>
			{onSetClipFade && (
				<ClipFadeHandles
					clip={clip}
					pixelsPerFrame={pixelsPerFrame}
					onCommit={(kind, frames) => onSetClipFade(clip.id, kind, frames)}
				/>
			)}
			{cueList?.body && timingDefaults && (
				<CueListClipContents
					cueList={{ ...cueList, body: cueList.body }}
					clip={clip}
					pixelsPerFrame={pixelsPerFrame}
					timingDefaults={timingDefaults}
					onSaveCueList={onSaveCueList}
					onPlaceCueStart={onPlaceCueStart}
					onError={onCueTimingError}
				/>
			)}
			<span
				className="timecode-clip-edge start"
				aria-hidden="true"
				title="Scale the clip and every Cue timing in it"
				onPointerDown={(event) => {
					event.stopPropagation();
					startDrag(event, item.selection, item.frame, "start");
				}}
			/>
			<span
				className="timecode-clip-edge end"
				aria-hidden="true"
				title="Scale the clip and every Cue timing in it"
				onPointerDown={(event) => {
					event.stopPropagation();
					startDrag(event, item.selection, item.endFrame ?? item.frame, "end");
				}}
			/>
		</>
	);
}
