import type {
	PackagedStageBenchmarkConfig,
	StagePaneBenchmarkObservations,
} from "./platform/desktop/types";

export function benchmarkStageEnabled(
	config: PackagedStageBenchmarkConfig,
): boolean {
	if (typeof config.stageEnabled !== "boolean")
		throw new Error("Invalid packaged benchmark Stage mode");
	return config.stageEnabled;
}

/** Successful, actual host queries, including empty responses; failures never become zeros. */
export class NativeHostObservations {
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
		if (this.rows.length >= 10_000)
			throw new Error("Host Stage observation overflow");
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
