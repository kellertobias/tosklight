import { useEffect, useMemo, useState } from "react";
import type { Cue, PlaybackSnapshot } from "../../api/types";
import type { CueTimingProgressByRow } from "./CueTable";

type CuelistRuntime = PlaybackSnapshot["active"][number] | undefined;

function timestampMillis(value: string | null | undefined): number | null {
	if (!value) return null;
	const parsed = Date.parse(value);
	return Number.isFinite(parsed) ? parsed : null;
}

function clampProgress(value: number): number {
	return Math.min(1, Math.max(0, value));
}

export function effectiveCueTimingNow(
	runtime: CuelistRuntime,
	wallNowMillis: number,
): number {
	if (!runtime?.paused) return wallNowMillis;
	return timestampMillis(runtime.paused_at) ?? wallNowMillis;
}

function phaseProgress(
	elapsedMillis: number,
	delayMillis: number,
	durationMillis: number,
): number {
	if (elapsedMillis < delayMillis) return 0;
	if (durationMillis === 0) return 1;
	return clampProgress((elapsedMillis - delayMillis) / durationMillis);
}

export function deriveCueTimingProgress(
	cues: readonly Cue[],
	runtime: CuelistRuntime,
	nowMillis: number,
): CueTimingProgressByRow {
	const timing = runtime?.cue_timing;
	const activatedAt = timestampMillis(runtime?.activated_at);
	if (!timing || activatedAt === null) return {};
	const timelineNow = effectiveCueTimingNow(runtime, nowMillis);

	const progress: CueTimingProgressByRow = {};
	const currentIndex = cues.findIndex((cue) => cue.id === timing.cue_id);
	if (currentIndex >= 0) {
		const elapsed = Math.max(0, timelineNow - activatedAt);
		progress[currentIndex] = {
			inDelay: phaseProgress(elapsed, 0, timing.in_delay_millis),
			inFade: phaseProgress(
				elapsed,
				timing.in_delay_millis,
				timing.in_fade_millis,
			),
		};
		const outgoingIndex = runtime?.previous_index;
		if (typeof outgoingIndex === "number" && cues[outgoingIndex]) {
			progress[outgoingIndex] = {
				...progress[outgoingIndex],
				outDelay: phaseProgress(elapsed, 0, timing.out_delay_millis),
				outFade: phaseProgress(
					elapsed,
					timing.out_delay_millis,
					timing.out_fade_millis,
				),
			};
		}
	}

	const trigger = timing.active_trigger;
	if (trigger) {
		const triggerIndex = cues.findIndex((cue) => cue.id === trigger.cue.id);
		const startedAt = timestampMillis(trigger.started_at);
		if (triggerIndex >= 0 && startedAt !== null) {
			progress[triggerIndex] = {
				...progress[triggerIndex],
				triggerTime:
					timing.completed_trigger_cue_id === trigger.cue.id
						? 1
						: phaseProgress(
								timelineNow - startedAt,
								0,
								trigger.duration_millis,
							),
			};
		}
	}

	if (timing.completed_trigger_cue_id) {
		const completedIndex = cues.findIndex(
			(cue) => cue.id === timing.completed_trigger_cue_id,
		);
		if (completedIndex >= 0)
			progress[completedIndex] = {
				...progress[completedIndex],
				triggerTime: 1,
			};
	}

	return progress;
}

function animationEndMillis(runtime: CuelistRuntime): number | null {
	const timing = runtime?.cue_timing;
	const activatedAt = timestampMillis(runtime?.activated_at);
	if (!timing || activatedAt === null) return null;
	const transitionEnd =
		activatedAt +
		Math.max(
			timing.completion_millis,
			timing.in_delay_millis + timing.in_fade_millis,
			timing.out_delay_millis + timing.out_fade_millis,
		);
	const triggerStartedAt = timestampMillis(timing.active_trigger?.started_at);
	const triggerEnd =
		triggerStartedAt === null
			? transitionEnd
			: triggerStartedAt + (timing.active_trigger?.duration_millis ?? 0);
	return Math.max(transitionEnd, triggerEnd);
}

function useCueTimingClock(runtime: CuelistRuntime): number {
	const pausedAt = runtime?.paused
		? timestampMillis(runtime.paused_at)
		: null;
	const [now, setNow] = useState(() => pausedAt ?? Date.now());
	const end = animationEndMillis(runtime);
	useEffect(() => {
		if (pausedAt !== null) {
			setNow(pausedAt);
			return;
		}
		const initial = Date.now();
		setNow(initial);
		if (end === null || initial >= end) return;
		let frame = 0;
		const tick = () => {
			const current = Date.now();
			setNow(current);
			if (current < end) frame = requestAnimationFrame(tick);
		};
		frame = requestAnimationFrame(tick);
		return () => cancelAnimationFrame(frame);
	}, [
		end,
		pausedAt,
		runtime?.cue_timing?.cue_id,
		runtime?.transition_ordinal,
	]);
	return pausedAt ?? now;
}

export function useCueTimingProgress(
	cues: readonly Cue[],
	runtime: CuelistRuntime,
): CueTimingProgressByRow {
	const now = useCueTimingClock(runtime);
	return useMemo(
		() => deriveCueTimingProgress(cues, runtime, now),
		[cues, now, runtime],
	);
}
