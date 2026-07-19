import type { Cue } from "../../api/types";

export type CueTriggerKind = "go" | "follow" | "time";

export function cueTriggerKind(cue: Cue | null | undefined): CueTriggerKind {
	if (cue?.trigger.type === "manual") return "go";
	if (
		cue?.trigger.type === "follow" &&
		Number(cue.trigger.delay_millis ?? 0) === 0
	)
		return "follow";
	return "time";
}

export function cueDraftIdentity(cue: Cue | null | undefined): string | null {
	if (!cue) return null;
	return cue.id ?? `number:${cue.number}`;
}

export function formatCueSeconds(millis: number): string {
	return `${(millis / 1000).toFixed(3).replace(/\.?0+$/, "")} s`;
}

export function cueTrigger(kind: CueTriggerKind, delayMillis: number) {
	if (kind === "go") return { type: "manual" };
	if (kind === "follow") return { type: "follow", delay_millis: 0 };
	return { type: "wait", delay_millis: delayMillis };
}
