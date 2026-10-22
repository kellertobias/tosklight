import type { CueList } from "../../api/types";
import type { TimecodeCueListClip } from "../../api/types/timecode";
import type {
	CueClipTimingDefaults,
	CueClipTimingRow,
	CueFadeEdge,
	CueFadeKind,
} from "./cueClipTiming";
import { useCueClipBoundarySlots } from "./CueClipBoundaryContext";
import { TimecodeInspectorSection } from "./TimecodeInspectorSection";
import { ExactFrame } from "./TimecodeCueControls";
export function CueTimingInspector({
	selected,
	clip,
	canMoveTrigger,
	placeTrigger,
	source,
	saving,
	onSaveCueList,
	commit,
	timingDefaults,
	onSetClipFade,
}: {
	selected: CueClipTimingRow;
	clip: TimecodeCueListClip;
	canMoveTrigger: boolean;
	placeTrigger(frame: number): void;
	source: string;
	saving: boolean;
	onSaveCueList?(id: string, body: CueList): Promise<CueList>;
	commit(
		row: CueClipTimingRow,
		kind: CueFadeKind,
		edge: CueFadeEdge,
		frame: number,
	): void;
	timingDefaults: CueClipTimingDefaults;
	onSetClipFade?(id: string, kind: "in" | "out", frames: number): void;
}) {
	const boundaries = useCueClipBoundarySlots();
	return (
		<div className="timecode-cue-editor-details">
			<h3>
				Cue {selected.cue.number} · {selected.cue.name || "Untitled"}
			</h3>
			<ExactFrame
				label="Trigger"
				frame={selected.startFrame}
				disabled={!canMoveTrigger}
				onCommit={placeTrigger}
			/>
			<p>{source}</p>
			<p>
				Drag the yellow <strong>Trigger</strong> handle to move this Cue’s
				actual start. Automatic trigger times are derived; edit their trigger in
				the Cuelist.
			</p>
			{(["in", "out"] as const).map((kind) => {
				const range = kind === "in" ? selected.inFade : selected.outFade;
				return (
					<TimecodeInspectorSection key={kind} title={kind === "in" ? "Cue In" : "Cue Out"} defaultOpen>
						{kind === "in" && boundaries?.inControl}
						{(["start", "end"] as const).map((edge) => (
							<ExactFrame
								key={edge}
								label={`${kind === "in" ? "In" : "Out"} ${edge === "start" ? "start" : "end"}`}
								frame={edge === "start" ? range.startFrame : range.endFrame}
								disabled={saving || !onSaveCueList || Boolean(selected.diagnostic)}
								onCommit={(frame) => commit(selected, kind, edge, frame)}
							/>
						))}
						{kind === "out" && boundaries?.outControl}
						{kind === "in" && !selected.cue.fade_millis && (
							<p>In inherits the desk Cue Fade ({timingDefaults.sequenceFadeMillis} ms). Storing zero keeps that inheritance.</p>
						)}
						{kind === "out" && selected.outFadeReference && (
							<>
								<p>Cue Out applies at a later Cue handover. It is not played at this clip’s end.</p>
								<ExactFrame label="Clip Out duration" frame={clip.out_fade_frames} disabled={!onSetClipFade}
									onCommit={(frames) => onSetClipFade?.(clip.id, "out", Math.max(0, Math.min(clip.end_frame - clip.start_frame, frames)))} />
								<p>Clip Out fades gain before the clip ends. {clip.end_behavior === "hold"
									? "Hold retains the final Cue at its ending level."
									: "Release removes this clip’s ownership at its end."}</p>
							</>
						)}
					</TimecodeInspectorSection>
				);
			})}
		</div>
	);
}
