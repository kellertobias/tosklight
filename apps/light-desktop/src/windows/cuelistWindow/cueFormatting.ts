import type { Cue } from "../../api/types";

export type CueTriggerKind = "go" | "follow" | "time" | "timecode" | "link";

export function cueTriggerKind(cue: Cue | null | undefined): CueTriggerKind {
	if (cue?.trigger.type === "manual") return "go";
	if (
		cue?.trigger.type === "follow" &&
		Number(cue.trigger.delay_millis ?? 0) === 0
	)
		return "follow";
	if (cue?.trigger.type === "link") return "link";
	if (cue?.trigger.type === "timecode") return "timecode";
	return "time";
}

export function cueDraftIdentity(cue: Cue | null | undefined): string | null {
	if (!cue) return null;
	return cue.id ?? `number:${cue.number}`;
}

export function formatCueSeconds(millis: number): string {
	return `${(millis / 1000).toFixed(3).replace(/\.?0+$/, "")} s`;
}

export function cueTrigger(
	kind: CueTriggerKind,
	delayMillis: number,
	destinationCueId?: string,
	timecodeFrame = 0,
) {
	if (kind === "go") return { type: "manual" };
	if (kind === "follow") return { type: "follow", delay_millis: 0 };
	if (kind === "link")
		return {
			type: "link",
			cue_id: destinationCueId ?? "",
			delay_millis: delayMillis,
		};
	if (kind === "timecode") return { type: "timecode", frame: timecodeFrame };
	return { type: "wait", delay_millis: delayMillis };
}
