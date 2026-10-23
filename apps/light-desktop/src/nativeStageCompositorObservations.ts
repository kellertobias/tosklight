import type { StageCompositorObservation } from "./platform/desktop/types";

/** Bounded intervals, separate from the helper; samples never estimate total GPU cost. */
export class NativeStageCompositorObservations {
	private readonly scopes = new Map<string, { samples: number; microsSum: number; microsMax: number }>();
	private readonly attachments = new Map<string, number>();
	private readonly totals = { drawAttempts: 0, submissions: 0, occluded: 0, timeouts: 0,
		reconfigures: 0, errors: 0, gpuReadbackErrors: 0, gpuSampleOverflow: 0,
		cpuWallMicros: 0, acquireWallMicros: 0 };
	private unavailable = new Set<string>();
	private overflow = 0;
	private invalid = 0;
	private repeated = 0;

	record(observations: readonly StageCompositorObservation[]) {
		for (const observation of observations) {
			const key = JSON.stringify([observation.paneId, observation.attachmentId]);
			if (!this.attachments.has(key)) {
				if (this.attachments.size >= 64) { this.overflow++; continue; }
				this.attachments.set(key, -1);
			}
			const measurement = observation.measurement;
			for (const name of Object.keys(this.totals) as Array<keyof typeof this.totals>) {
				const value = measurement[name];
				if (Number.isFinite(value) && value >= 0) this.totals[name] += value;
				else this.invalid++;
			}
			if (measurement.gpuTimingUnavailable && this.unavailable.size < 64) this.unavailable.add(measurement.gpuTimingUnavailable.slice(0, 512));
			for (const sample of measurement.gpuSamples) {
				if (!Number.isSafeInteger(sample.sampleId) || sample.sampleId < 0 || !Number.isFinite(sample.gpuMicros) || sample.gpuMicros < 0) { this.invalid++; continue; }
				if (sample.sampleId <= (this.attachments.get(key) ?? -1)) { this.repeated++; continue; }
				this.attachments.set(key, sample.sampleId);
				const scopeKey = JSON.stringify([observation.paneId, observation.attachmentId,
					Math.floor(sample.submittedEpochMicros / 1_000_000), sample.scope]);
				let row = this.scopes.get(scopeKey);
				if (!row) {
					if (this.scopes.size >= 20_000) { this.overflow++; continue; }
					row = { samples: 0, microsSum: 0, microsMax: 0 };
					this.scopes.set(scopeKey, row);
				}
				row.samples++;
				row.microsSum += sample.gpuMicros;
				row.microsMax = Math.max(row.microsMax, sample.gpuMicros);
			}
		}
	}

	report() {
		return { totals: { ...this.totals }, overflow: this.overflow, invalid: this.invalid,
			repeatedSamples: this.repeated, unavailable: [...this.unavailable],
			actualDisplayedFrames: null,
			boundary: "Compositor render-pass samples and elapsed CPU wall time. Sparse samples are not total GPU time per second; submit/completion is not display.",
			rows: [...this.scopes].map(([key, value]) => ({ scope: JSON.parse(key), ...value,
				microsMean: value.microsSum / value.samples })) };
	}
}
