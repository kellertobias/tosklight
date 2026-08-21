import type { Cue, CueList } from "../../api/types";
import type { TimecodeCueListClip } from "../../api/types/timecode";

export const TIMECODE_FPS = 44;

export interface CueClipTimingDefaults {
	sequenceFadeMillis: number;
	releaseFadeMillis: number;
}

export interface CueClipTimingRange {
	startFrame: number;
	endFrame: number;
}

export interface CueClipTimingRow {
	cue: Cue;
	startFrame: number;
	inFade: CueClipTimingRange;
	outFade: CueClipTimingRange;
	diagnostic?: string;
	/// Set for a Cue whose start the Timecode lane owns because it waits for a manual GO.
	/// "placed" means the operator dropped a transition point, "default" means it follows
	/// its predecessor until one is placed.
	transition?: "placed" | "default";
}

export type CueFadeKind = "in" | "out";
export type CueFadeEdge = "start" | "end";

export function millisToTimecodeFrames(millis: number): number {
	return Math.round((Math.max(0, millis) * TIMECODE_FPS) / 1_000);
}

export function timecodeFramesToMillis(frames: number): number {
	return Math.round((Math.max(0, frames) * 1_000) / TIMECODE_FPS);
}

export function cueClipTimingRows(
	clip: TimecodeCueListClip,
	cueList: CueList,
	defaults: CueClipTimingDefaults,
): { rows: CueClipTimingRow[]; error?: string } {
	const startIndex = cueList.cues.findIndex(
		(cue) => cue.id === clip.start_cue_id,
	);
	const endIndex = cueList.cues.findIndex((cue) => cue.id === clip.end_cue_id);
	if (startIndex < 0)
		return { rows: [], error: "The clip start Cue no longer exists." };
	if (endIndex < 0)
		return { rows: [], error: "The clip end Cue no longer exists." };
	if (endIndex < startIndex)
		return { rows: [], error: "The clip end Cue is before its start Cue." };

	const range = cueList.cues.slice(startIndex, endIndex + 1);
	const first = range[0];
	if (!first?.id)
		return { rows: [], error: "The clip start Cue has no stable identity." };
	const scheduled = new Map<string, number>([[first.id, clip.start_frame]]);
	const diagnostics = new Map<string, string>();
	const transitions = new Map<string, "placed" | "default">();
	const visited = new Set<string>();
	let current: Cue | undefined = first;
	while (current) {
		if (!current.id || visited.has(current.id)) break;
		visited.add(current.id);
		const currentStart = scheduled.get(current.id) ?? clip.start_frame;
		const currentRow = timingRow(current, currentStart, defaults);
		let next: Cue | undefined = range[range.indexOf(current) + 1];
		let nextStart: number;
		if (current.trigger.type === "link") {
			const targetId: string = String(current.trigger.cue_id ?? "");
			next = range.find((cue): boolean => cue.id === targetId);
			if (!next) {
				diagnostics.set(
					current.id,
					`Cue ${current.number} links outside this clip or to a missing Cue.`,
				);
				break;
			}
			if (next.id && visited.has(next.id)) {
				diagnostics.set(
					current.id,
					`Cue ${current.number} creates a Link cycle.`,
				);
				break;
			}
			nextStart =
				Math.max(currentRow.inFade.endFrame, currentRow.outFade.endFrame) +
				millisToTimecodeFrames(Number(current.trigger.delay_millis ?? 0));
		} else {
			if (!next) break;
			const incomingDelay = millisToTimecodeFrames(
				Number(next.trigger.delay_millis ?? 0),
			);
			if (next.trigger.type === "timecode")
				nextStart =
					clip.start_frame + Math.round(Number(next.trigger.frame ?? 0));
			else if (next.trigger.type === "wait")
				nextStart = currentStart + incomingDelay;
			else if (next.trigger.type === "follow")
				nextStart =
					Math.max(currentRow.inFade.endFrame, currentRow.outFade.endFrame) +
					incomingDelay;
			else {
				const manualId = next.id;
				const placed = clip.cue_starts?.find(
					(candidate) => candidate.cue_id === manualId,
				);
				transitions.set(manualId ?? "", placed ? "placed" : "default");
				nextStart = placed
					? clip.start_frame + Math.round(placed.offset_frame)
					: Math.max(currentRow.inFade.endFrame, currentRow.outFade.endFrame);
			}
		}
		if (!next) break;
		if (!next.id) {
			diagnostics.set(current.id, "The next Cue has no stable identity.");
			break;
		}
		scheduled.set(next.id, nextStart);
		current = next;
	}

	const rows: CueClipTimingRow[] = [];
	for (const cue of range) {
		const previous = rows.at(-1);
		const startFrame =
			(cue.id ? scheduled.get(cue.id) : undefined) ??
			Math.max(
				previous?.inFade.endFrame ?? clip.start_frame,
				previous?.outFade.endFrame ?? clip.start_frame,
			);
		const inFadeMillis = effectiveInFadeMillis(cue, defaults);
		const inStart = startFrame + millisToTimecodeFrames(cue.delay_millis);
		const outStart =
			startFrame +
			millisToTimecodeFrames(effectiveOutDelayMillis(cue, inFadeMillis));
		const row: CueClipTimingRow = {
			cue,
			startFrame,
			inFade: {
				startFrame: inStart,
				endFrame: inStart + millisToTimecodeFrames(inFadeMillis),
			},
			outFade: {
				startFrame: outStart,
				endFrame:
					outStart +
					millisToTimecodeFrames(
						effectiveOutFadeMillis(cue, inFadeMillis, defaults),
					),
			},
		};
		const transition = cue.id ? transitions.get(cue.id) : undefined;
		if (transition) row.transition = transition;
		if (!cue.id) row.diagnostic = `Cue ${cue.number} has no stable identity.`;
		else if (diagnostics.has(cue.id)) row.diagnostic = diagnostics.get(cue.id);
		else if (!scheduled.has(cue.id))
			row.diagnostic = `Cue ${cue.number} is not reached by this clip's Cue order.`;
		else if (startFrame < clip.start_frame || startFrame > clip.end_frame)
			row.diagnostic = `Cue ${cue.number} starts outside this clip.`;
		else if (
			row.inFade.endFrame > clip.end_frame ||
			row.outFade.endFrame > clip.end_frame
		)
			row.diagnostic = `Cue ${cue.number} timing extends outside this clip.`;
		rows.push(row);
	}
	return { rows };
}

export function cueWithDraggedFade(
	cue: Cue,
	row: CueClipTimingRow,
	clip: TimecodeCueListClip,
	kind: CueFadeKind,
	edge: CueFadeEdge,
	targetFrame: number,
): { cue?: Cue; error?: string } {
	if (!Number.isFinite(targetFrame))
		return { error: "Timing position is invalid." };
	const target = Math.round(targetFrame);
	const range = kind === "in" ? row.inFade : row.outFade;
	if (target < row.startFrame)
		return { error: `Cue ${cue.number} timing cannot start before the Cue.` };
	if (target > clip.end_frame)
		return { error: `Cue ${cue.number} timing must remain inside the clip.` };
	if (edge === "start" && target > range.endFrame)
		return { error: `Cue ${cue.number} fade start cannot cross its fade end.` };
	if (edge === "end" && target < range.startFrame)
		return { error: `Cue ${cue.number} fade end cannot cross its fade start.` };

	const delayFrames =
		edge === "start"
			? target - row.startFrame
			: range.startFrame - row.startFrame;
	const fadeFrames =
		edge === "start" ? range.endFrame - target : target - range.startFrame;
	const delayMillis = timecodeFramesToMillis(delayFrames);
	const fadeMillis = timecodeFramesToMillis(fadeFrames);
	if (!Number.isSafeInteger(delayMillis) || !Number.isSafeInteger(fadeMillis))
		return { error: "Timing is outside the supported range." };
	return {
		cue:
			kind === "in"
				? { ...cue, delay_millis: delayMillis, fade_millis: fadeMillis }
				: {
						...cue,
						out_delay_millis: delayMillis,
						out_fade_millis: fadeMillis,
						out_delay_link: undefined,
						out_fade_link: undefined,
					},
	};
}

function timingRow(
	cue: Cue,
	startFrame: number,
	defaults: CueClipTimingDefaults,
): CueClipTimingRow {
	const inFadeMillis = effectiveInFadeMillis(cue, defaults);
	const inStart = startFrame + millisToTimecodeFrames(cue.delay_millis);
	const outStart =
		startFrame +
		millisToTimecodeFrames(effectiveOutDelayMillis(cue, inFadeMillis));
	return {
		cue,
		startFrame,
		inFade: {
			startFrame: inStart,
			endFrame: inStart + millisToTimecodeFrames(inFadeMillis),
		},
		outFade: {
			startFrame: outStart,
			endFrame:
				outStart +
				millisToTimecodeFrames(
					effectiveOutFadeMillis(cue, inFadeMillis, defaults),
				),
		},
	};
}

function effectiveInFadeMillis(cue: Cue, defaults: CueClipTimingDefaults) {
	return cue.fade_millis || defaults.sequenceFadeMillis;
}

function effectiveOutDelayMillis(cue: Cue, effectiveInFade: number) {
	if (cue.out_delay_link === "in_fade") return effectiveInFade;
	return cue.out_delay_millis ?? cue.delay_millis;
}

function effectiveOutFadeMillis(
	cue: Cue,
	effectiveInFade: number,
	defaults: CueClipTimingDefaults,
) {
	if (cue.out_fade_link === "release") return defaults.releaseFadeMillis;
	return cue.out_fade_millis ?? effectiveInFade;
}

/// Total frame length of a Cuelist range whose Cues all schedule automatically.
/// Returns null when any Cue in the range needs a manual GO or cannot be reached,
/// so callers can fall back to a copied or full-lane clip length.
export function automatedCueClipLength(
	cueList: CueList,
	startCueId: string,
	endCueId: string,
	startFrame: number,
	defaults: CueClipTimingDefaults,
): number | null {
	const probe: TimecodeCueListClip = {
		id: "automated-length-probe",
		start_frame: startFrame,
		end_frame: startFrame + TIMECODE_FPS * 3_600 * 100,
		start_cue_id: startCueId,
		end_cue_id: endCueId,
		start_behavior: "state",
		end_behavior: "release",
		cue_starts: [],
	};
	const { rows, error } = cueClipTimingRows(probe, cueList, defaults);
	if (
		error ||
		!rows.length ||
		rows.some((row) => row.diagnostic || row.transition === "default")
	)
		return null;
	const last = rows.reduce(
		(latest, row) =>
			Math.max(latest, row.inFade.endFrame, row.outFade.endFrame),
		startFrame,
	);
	return Math.max(1, last - startFrame);
}
