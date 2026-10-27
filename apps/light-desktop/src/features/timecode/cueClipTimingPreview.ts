import type { CueList } from "../../api/types";
import type { TimecodeCueListClip } from "../../api/types/timecode";
import {
	cueClipTimingRows,
	type CueClipTimingDefaults,
	type CueClipTimingRow,
} from "./cueClipTiming";

/** Preview the same clip transition the release will store, including dependent Cue timing. */
export function cueClipTimingWithTriggerPreview(
	clip: TimecodeCueListClip,
	body: CueList,
	defaults: CueClipTimingDefaults,
	selectedCueId: string | null,
	frame: number | null,
) {
	const saved = cueClipTimingRows(clip, body, defaults);
	const selected = saved.rows.find((row) => row.cue.id === selectedCueId);
	if (
		frame === null ||
		!selected?.transition ||
		selected === saved.rows[0] ||
		!selectedCueId
	)
		return saved;
	return cueClipTimingRows(
		{
			...clip,
			cue_starts: [
				...(clip.cue_starts ?? []).filter(
					(start) => start.cue_id !== selectedCueId,
				),
				{
					cue_id: selectedCueId,
					offset_frame: Math.max(0, Math.round(frame) - clip.start_frame),
				},
			],
		},
		body,
		defaults,
	);
}

export function cueTriggerSource(
	selected: CueClipTimingRow | undefined,
	first: boolean,
): string {
	return first
		? "Clip start · moves the clip"
		: selected?.transition
			? selected.transition === "placed"
				? "Manual GO · placed in this clip"
				: "Manual GO · follows previous completion until placed"
			: `${selected?.cue.trigger.type ?? "Automatic"} · derived from Cuelist trigger settings`;
}
