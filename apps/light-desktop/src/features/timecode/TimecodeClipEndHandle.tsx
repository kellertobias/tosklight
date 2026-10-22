import type { TimecodeCueListClip } from "../../api/types/timecode";
import { END_GRIP_CENTER } from "./cueEditorLayout";
import { FrameHandle } from "./TimecodeCueControls";

/** The last Cue's duration ends with the clip; this grip does not scale its Cue timings. */
export function ClipEndHandle({
	clip,
	pixelsPerFrame,
	expanded,
	minimum,
	maximum,
	onCommit,
	onPreview,
}: {
	clip: TimecodeCueListClip;
	pixelsPerFrame: number;
	expanded: boolean;
	minimum?: number;
	maximum?: number;
	onCommit?(frame: number): void;
	onPreview?(frame: number | null): void;
}) {
	if (!expanded) return null;
	return (
		<FrameHandle
			label="Extend last Cue"
			caption="End"
			frame={clip.end_frame}
			minimum={minimum ?? clip.start_frame + 1}
			maximum={maximum ?? Number.MAX_SAFE_INTEGER}
			left={(clip.end_frame - clip.start_frame) * pixelsPerFrame - 22}
			anchorLeft={(clip.end_frame - clip.start_frame) * pixelsPerFrame}
			top={END_GRIP_CENTER}
			disabled={!onCommit}
			onCommit={(frame) => onCommit?.(frame)}
			onPreview={onPreview}
			className="timecode-clip-end-grab expanded"
		/>
	);
}
