import type { TimecodeCueListClip } from "../../api/types/timecode";

/** Visible playback policy at each clip edge; it does not change transport state. */
export function ClipBehaviorLabels({ clip }: { clip: TimecodeCueListClip }) {
	return <span className="timecode-clip-behaviors">
		<span className="timecode-clip-start-behavior" aria-label={`Start behavior: ${clip.start_behavior === "state" ? "State Start" : "Cue Start"}`}>
			{clip.start_behavior === "state" ? "State Start" : "Cue Start"}
		</span>
		<span className="timecode-clip-end-behavior" aria-label={`End behavior: ${clip.end_behavior === "hold" ? "Hold" : "Release"}`}>
			{clip.end_behavior === "hold" ? "Hold" : "Release"}{clip.out_fade_frames > 0 ? " · Clip Out" : ""}
		</span>
	</span>;
}
