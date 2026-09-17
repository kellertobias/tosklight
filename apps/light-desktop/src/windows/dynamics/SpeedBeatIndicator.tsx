import { useEffect, useRef } from "react";
import type {
	DynamicRuntimeSnapshotProjection,
	SpeedGroupId,
} from "../../api/types";

/** Portion of each beat, from its start, during which the indicator is lit. */
export const BEAT_FLASH_FRACTION = 0.2;
/** A sample older than this means the desk stopped answering; stop claiming a beat. */
export const BEAT_SAMPLE_STALE_MILLIS = 2_500;

export interface SpeedBeatSample {
	group: SpeedGroupId;
	/** Authoritative beat position in `[0, 1)` when the sample was received. */
	phase: number;
	bpm: number;
	advancing: boolean;
	/** `performance.now()` when the snapshot carrying this sample arrived. */
	receivedAt: number;
}

export type SpeedBeatState = "flash" | "dark" | "paused" | "unavailable";

const receipts = new WeakMap<object, number>();

/** Records when an authoritative runtime snapshot arrived, so its beat can be extrapolated. */
export function stampRuntimeReceipt<T extends object>(
	runtime: T,
	receivedAt = performance.now(),
): T {
	receipts.set(runtime, receivedAt);
	return runtime;
}

export function speedBeatSample(
	runtime: DynamicRuntimeSnapshotProjection | null,
	group: SpeedGroupId,
): SpeedBeatSample | null {
	const transport = runtime?.speed_groups?.find(
		(candidate) => candidate.group === group,
	);
	if (!runtime || !transport) return null;
	return {
		group,
		phase: transport.beat_phase,
		bpm: transport.effective_bpm,
		advancing: transport.phase_advancing,
		receivedAt: receipts.get(runtime) ?? performance.now(),
	};
}

/**
 * The beat position the desk reports now: the sampled phase advanced by the sampled rate.
 * Every new snapshot replaces the sample, so drift never outlives one poll.
 */
export function beatPhaseAt(sample: SpeedBeatSample, now: number) {
	if (!sample.advancing) return sample.phase;
	const elapsedBeats =
		(Math.max(0, now - sample.receivedAt) * sample.bpm) / 60_000;
	return (((sample.phase + elapsedBeats) % 1) + 1) % 1;
}

export function speedBeatState(
	sample: SpeedBeatSample | null,
	now: number,
): SpeedBeatState {
	if (!sample || now - sample.receivedAt > BEAT_SAMPLE_STALE_MILLIS)
		return "unavailable";
	if (!sample.advancing) return "paused";
	return beatPhaseAt(sample, now) < BEAT_FLASH_FRACTION ? "flash" : "dark";
}

const LABELS: Record<SpeedBeatState, string> = {
	flash: "beat",
	dark: "between beats",
	paused: "paused",
	unavailable: "unavailable",
};

/**
 * A circle that lights at the start of every authoritative Speed Group beat. It animates
 * outside React so a 60 Hz flash never re-renders the editor.
 */
export function SpeedBeatIndicator({
	sample,
}: {
	sample: SpeedBeatSample | null;
}) {
	const element = useRef<HTMLSpanElement>(null);
	useEffect(() => {
		let frame = 0;
		let shown: SpeedBeatState | null = null;
		const paint = () => {
			const state = speedBeatState(sample, performance.now());
			const node = element.current;
			if (node && state !== shown) {
				shown = state;
				node.dataset.state = state;
				node.setAttribute(
					"aria-label",
					sample
						? `Speed Group ${sample.group} beat, ${LABELS[state]}`
						: "Speed Group beat unavailable",
				);
			}
			frame = requestAnimationFrame(paint);
		};
		paint();
		return () => cancelAnimationFrame(frame);
	}, [sample]);
	return (
		<span
			ref={element}
			className="dynamic-beat-indicator"
			role="img"
			aria-label="Speed Group beat unavailable"
			data-state="unavailable"
		/>
	);
}
