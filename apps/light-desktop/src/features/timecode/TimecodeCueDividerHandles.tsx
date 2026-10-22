import type { CueList } from "../../api/types";
import type { TimecodeCueListClip } from "../../api/types/timecode";
import { cueTransitionFrameBounds, type CueClipTimingDefaults, type CueClipTimingRow } from "./cueClipTiming";
import { DIVIDER_CENTER } from "./cueEditorLayout";
import { cueTriggerEditing } from "./cueTriggerEditing";
import { FrameHandle } from "./TimecodeCueControls";

/** The paired grip marks straddle the real Cue boundary; dragging moves its trigger. */
export function CueDividerHandles({ rows, clip, body, defaults, position, disabled,
	onSelect, onPreviewTrigger, onPreviewMoveClip, onMoveClip, onPlaceCueStart,
}: {
	rows: CueClipTimingRow[];
	clip: TimecodeCueListClip;
	body: CueList;
	defaults: CueClipTimingDefaults;
	position(frame: number): number;
	disabled: boolean;
	onSelect?(cueId: string): void;
	onPreviewTrigger(frame: number | null): void;
	onPreviewMoveClip?(clipId: string, frame: number | null): void;
	onMoveClip?(clipId: string, frame: number): void;
	onPlaceCueStart?(clipId: string, cueId: string, offset: number): void;
}) {
	const { placeRow } = cueTriggerEditing(clip, body, defaults, rows, null, onMoveClip, onPlaceCueStart);
	return <span className="timecode-cue-divider-handles">
		{rows.map((row, index) => {
			if (!row.cue.id) return null;
			const first = index === 0;
			const bounds = cueTransitionFrameBounds(clip, body, defaults, row.cue.id);
			if (!first && !bounds) return null;
			return <span key={row.cue.id} onPointerDownCapture={() => onSelect?.(row.cue.id!)}
				onFocusCapture={() => onSelect?.(row.cue.id!)}>
				<FrameHandle
					label={`Move Cue ${row.cue.number} trigger`}
					frame={row.startFrame}
					minimum={first ? 0 : bounds!.minimum}
					maximum={first ? Number.MAX_SAFE_INTEGER : bounds!.maximum}
					left={position(row.startFrame)}
					top={DIVIDER_CENTER}
					disabled={disabled || Boolean(row.diagnostic) || (first ? !onMoveClip : !onPlaceCueStart)}
					onPreview={(frame) => {
						if (first && onPreviewMoveClip) onPreviewMoveClip(clip.id, frame);
						else onPreviewTrigger(frame);
					}}
					onCommit={(frame) => placeRow(row, first, frame)}
					className="cue-divider-grip"
				/>
			</span>;
		})}
	</span>;
}
