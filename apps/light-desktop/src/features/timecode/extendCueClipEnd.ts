import type { TimecodeCueListOption } from "./timecodeEditorShared";
import type { CueList } from "../../api/types";
import type {
	TimecodeCueListClip,
	TimecodeDefinition,
} from "../../api/types/timecode";
import { cueClipTimingRows, type CueClipTimingDefaults } from "./cueClipTiming";

export interface CueClipEndContext {
	cueList: CueList;
	timingDefaults: CueClipTimingDefaults;
}
export interface CueClipEndBounds {
	minimum: number;
	maximum: number;
}
function findClip(
	definition: TimecodeDefinition,
	laneId: string,
	clipId: string,
) {
	const lane = definition.lanes.find((candidate) => candidate.id === laneId);
	return lane?.content.kind === "cue_list"
		? {
				content: lane.content,
				clip: lane.content.clips.find((candidate) => candidate.id === clipId),
			}
		: null;
}
function lastTrigger(
	clip: TimecodeCueListClip,
	context?: CueClipEndContext,
): number | null {
	if (!context)
		return clip.start_cue_id === clip.end_cue_id ? clip.start_frame : null;
	const projected = cueClipTimingRows(
		clip,
		context.cueList,
		context.timingDefaults,
	);
	if (
		projected.error ||
		!projected.rows.length ||
		projected.rows.some((row) => row.diagnostic && !row.clipEndOverflow)
	)
		return null;
	return Math.max(...projected.rows.map((row) => row.startFrame));
}
/** Extending the ending leaves earlier Cue positions and shared Cue timings unchanged. */
export function cueClipEndBounds(
	definition: TimecodeDefinition,
	laneId: string,
	clipId: string,
	context?: CueClipEndContext,
): CueClipEndBounds | null {
	const found = findClip(definition, laneId, clipId);
	if (!found?.clip) return null;
	const clip = found.clip;
	const last = lastTrigger(clip, context);
	if (last === null) return null;
	const minimum = Math.max(clip.start_frame + 1, Math.ceil(last) + 1);
	const maximum = Math.min(
		definition.duration_frame ?? Number.MAX_SAFE_INTEGER,
		...found.content.clips
			.filter(
				(other) => other.id !== clip.id && other.start_frame > clip.start_frame,
			)
			.map((other) => other.start_frame),
	);
	return minimum <= maximum ? { minimum, maximum } : null;
}
/** Duration-only edit, distinct from the clip-edge operation that stretches all Cue timing. */
export function extendCueClipEnd(
	definition: TimecodeDefinition,
	laneId: string,
	clipId: string,
	frame: number,
	context?: CueClipEndContext,
): TimecodeDefinition {
	if (!Number.isFinite(frame)) return definition;
	const bounds = cueClipEndBounds(definition, laneId, clipId, context);
	if (!bounds) return definition;
	const endFrame = Math.max(
		bounds.minimum,
		Math.min(bounds.maximum, Math.round(frame)),
	);
	const found = findClip(definition, laneId, clipId);
	if (!found?.clip || endFrame === found.clip.end_frame) return definition;
	return {
		...definition,
		lanes: definition.lanes.map((lane) =>
			lane.id !== laneId || lane.content.kind !== "cue_list"
				? lane
				: {
						...lane,
						content: {
							...lane.content,
							clips: lane.content.clips.map((clip) =>
								clip.id !== clipId
									? clip
									: {
											...clip,
											end_frame: endFrame,
											in_fade_frames: Math.min(
												clip.in_fade_frames,
												endFrame - clip.start_frame,
											),
											out_fade_frames: Math.min(
												clip.out_fade_frames,
												endFrame - clip.start_frame,
											),
										},
							),
						},
					},
		),
	};
}

/** Bind the authoritative editor writer without duplicating Cuelist context resolution. */
export function createCueClipEndWriter({
	definition,
	cueLists,
	timingDefaults,
	onCommit,
}: {
	definition: TimecodeDefinition;
	cueLists: readonly TimecodeCueListOption[];
	timingDefaults: CueClipTimingDefaults;
	onCommit(value: TimecodeDefinition): void;
}) {
	return (laneId: string, clipId: string, frame: number) => {
		const lane = definition.lanes.find((candidate) => candidate.id === laneId);
		const listId =
			lane?.content.kind === "cue_list" ? lane.content.cue_list_id : undefined;
		const body = cueLists.find((list) => list.id === listId)?.body;
		const next = extendCueClipEnd(
			definition,
			laneId,
			clipId,
			frame,
			body ? { cueList: body, timingDefaults } : undefined,
		);
		if (next !== definition) onCommit(next);
	};
}
