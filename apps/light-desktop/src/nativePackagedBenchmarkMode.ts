import type {
	PackagedStageBenchmarkConfig,
	StagePaneBenchmarkObservations,
} from "./platform/desktop/types";

export const nativeBenchmarkRetentionLimits = Object.freeze({
	host: 32_000, activity: 64_000, heartbeats: 24_000,
	frames: 100_000, timeline: 32_000,
});

export function nativeBenchmarkObservationPolicy(config: PackagedStageBenchmarkConfig) {
	if (!Number.isInteger(config.durationSeconds) || config.durationSeconds <= 0 || config.durationSeconds > 3_600)
		throw new Error("Native packaged benchmark duration must be 1..3600 seconds");
	return {
		maximumDurationSeconds: 3_600,
		pollingIntervalMillis: config.stageEnabled === false ? 1_000 :
			config.durationSeconds >= 1_800 && !config.baselinePhases && !config.captureExercise ? 250 : 100,
		retentionLimits: nativeBenchmarkRetentionLimits,
		observations: "Polls drain native event batches before source deduplication; poll frequency is not rendered or displayed FPS. Source-frame history is deduplicated; GPU submission aggregates consume every drained frame.",
		memoryBoundary: "Main-process RSS is not whole-desk memory: it excludes separate WebView/driver processes and GPU allocations. JavaScript histories are not GPU resource ownership; no collector-cost subtraction is performed.",
	};
}

/** Preserve a bounded prefix; callers must count every rejected row in completion evidence. */
export function retainNativeHistory<T>(target: T[], value: T, limit: number): boolean {
	if (target.length >= limit) return false;
	target.push(value);
	return true;
}

export function benchmarkStageEnabled(
	config: PackagedStageBenchmarkConfig,
): boolean {
	if (typeof config.stageEnabled !== "boolean")
		throw new Error("Invalid packaged benchmark Stage mode");
	return config.stageEnabled;
}

/** Successful, actual host queries, including empty responses; failures never become zeros. */
export class NativeHostObservations {
	overflow = 0;
	readonly rows: Array<{
		recordedAt: number;
		frames: number;
		heartbeats: number;
	}> = [];
	record(batch: StagePaneBenchmarkObservations, recordedAt = Date.now()) {
		if (
			!Array.isArray(batch.frames) ||
			!Array.isArray(batch.heartbeats) ||
			!Number.isFinite(recordedAt)
		)
			throw new Error("Invalid host Stage observation");
		if (this.rows.length >= nativeBenchmarkRetentionLimits.host) {
			this.overflow += 1;
			throw new Error("Host Stage observation overflow");
		}
		this.rows.push({
			recordedAt,
			frames: batch.frames.length,
			heartbeats: batch.heartbeats.length,
		});
	}
}

export function benchmarkSurfaces(
	config: PackagedStageBenchmarkConfig,
): string[] {
	return [
		...(benchmarkStageEnabled(config)
			? ["stage-native-live", "stage-native-preload"]
			: []),
		...(config.fixtureSheet ? ["fixture-sheet"] : []),
	];
}
