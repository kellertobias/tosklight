import type { RuntimeOutputHealth } from "../apps/light-desktop/src/api/generated/light-wire";

export type OutputWindow = Pick<
	RuntimeOutputHealth,
	| "frames_sent"
	| "packets_sent"
	| "send_errors"
	| "deadline_misses"
	| "maximum_lateness_micros"
	| "last_tick_micros"
	| "maximum_tick_micros"
	| "scheduler_utilization"
	| "tick_duration_bucket_bounds_micros"
	| "tick_duration_bucket_counts"
>;

export function outputWindow(
	before: RuntimeOutputHealth,
	after: RuntimeOutputHealth,
): OutputWindow;

export function histogramPercentileMicros(
	window: OutputWindow,
	percentile: number,
): number | null;
