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
> & {
    tick_duration_fine_histogram?: FineOutputHistogram;
    fine_histogram_coverage?: { status: "complete" | "unavailable" | "invalid"; reasons: string[] };
};

export function outputWindow(
	before: RuntimeOutputHealth,
	after: RuntimeOutputHealth,
): OutputWindow;

export function histogramPercentileMicros(
	window: OutputWindow,
	percentile: number,
): number | null;

export interface OutputWindowAccumulator {
	observe(snapshot: RuntimeOutputHealth, observedAt?: number): void;
	take(snapshot: RuntimeOutputHealth): OutputWindow & {
		counter_resets: number;
		late_intervals: Array<{
			observedAt: string;
			deadline_misses: number;
			send_errors: number;
			counter_reset: boolean;
		}>;
	};
}

export function createOutputWindowAccumulator(
	start: RuntimeOutputHealth,
): OutputWindowAccumulator;

export interface FineOutputHistogram {
    schema_version: 1;
    bounds_micros: number[];
    counts: number[];
    sample_count: number;
}
export interface OutputComparison {
    /** Historical coarse upper-bound metrics, retained even when fine timing selects the gate. */
    noStageP99TickMicros: number | null;
    stageP99TickMicros: number | null;
    allowedP99RegressionMicros: number | null;
    p99RegressionMicros: number | null;
    coarseBoundedWindowGatePassed: boolean;
    boundedWindowGatePassed: boolean;
    p99ComparisonSource: "coarse_histogram_fallback" | "fine_histogram_v1" | "fine_histogram_rejected";
    fineP99Comparison: {
        status: "complete" | "unavailable" | "invalid";
        decision: "pass" | "fail" | "indeterminate" | "unavailable";
        reasons: string[];
        noStageIntervalMicros: [number,number|null] | null;
        stageIntervalMicros: [number,number|null] | null;
        regressionIntervalMicros: [number,number] | null;
        allowedRegressionIntervalMicros: [number,number] | null;
        nominalFineBucketMicros: 250;
        intervalConvention: string;
    };
}
export function compareOutputWindows(noStage: OutputWindow, stage: OutputWindow): OutputComparison;
