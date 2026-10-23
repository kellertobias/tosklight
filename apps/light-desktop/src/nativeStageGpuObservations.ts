import { NativeStageCompositorObservations } from "./nativeStageCompositorObservations";
import type { StageCompositorObservation, StagePaneBenchmarkSample } from "./platform/desktop/types";

export const nativeStageMeasurementBoundaries = {
	helperSubmissions: "Helper submission observations, including held-source redraws; not GPU completion or displayed frames.",
	gpuTimings: "Unique completed helper GPU timing samples with matching scope only; sparse samples do not estimate total GPU time per second or a reduction pass.",
	cpuMicros: "Elapsed wall time, not thread CPU time.",
	display: "Compositor timing is recorded separately; actual displayed presentation intervals require external trace validation.",
};

interface GpuObservationRow {
	windowEpochSecond: number;
	paneId: string;
	attachmentId: string;
	quality: StagePaneBenchmarkSample["quality"];
	followPreload: boolean;
	mode: string;
	width: number;
	height: number;
	transport: string;
	helperSubmissionCount: number;
	cpuWallMicrosSum: number;
	acquireWallMicrosSum: number;
	gpuSampleCount: number;
	gpuSampleMicrosSum: number;
	gpuSampleMicrosMax: number;
}

const natural = (value: number) => Number.isSafeInteger(value) && value >= 0;
const identifier = (value: string) => typeof value === "string" && value.length > 0 && value.length <= 512;
const duration = (value: number) => Number.isFinite(value) && value >= 0;

/** Bounded aggregate. IDs are monotonic within a host-minted attachment generation. */
export class NativeStageGpuObservations {
	private readonly compositor = new NativeStageCompositorObservations();
	private readonly rows = new Map<string, GpuObservationRow>();
	private readonly attachments = new Map<string, { sequence: number; gpuSampleId: number }>();
	readonly counts = {
		received: 0, helperSubmissions: 0, duplicateSubmissions: 0,
		invalidFrames: 0, attachmentOverflow: 0, rowOverflow: 0,
		repeatedOrStaleGpuSamples: 0, rejectedGpuProvenance: 0,
		invalidGpuSamples: 0, unavailableGpuSamples: 0,
		unavailableMode: 0, unavailableTransport: 0,
	};

	constructor(private readonly limits = { rows: 20_000, attachments: 64 }) {
		if (!natural(limits.rows) || !natural(limits.attachments)) throw new Error("Observation limits must be nonnegative integers");
	}

	record(samples: readonly StagePaneBenchmarkSample[], compositors: readonly StageCompositorObservation[] = []) {
		this.compositor.record(compositors);
		for (const frame of samples) this.recordFrame(frame);
	}

	private recordFrame(frame: StagePaneBenchmarkSample) {
		this.counts.received++;
		if (!identifier(frame.paneId) || !identifier(frame.attachmentId) ||
			!natural(frame.sequence) || !natural(frame.presentedEpochMicros) || frame.presentedEpochMicros === 0 ||
			!natural(frame.width) || !natural(frame.height) || frame.width === 0 || frame.height === 0 ||
			!duration(frame.cpuMicros) || !duration(frame.acquireMicros) ||
			!["draft", "standard", "high", "ultra"].includes(frame.quality) || typeof frame.followPreload !== "boolean" ||
			(frame.transport !== undefined && frame.transport !== "shared" && frame.transport !== "copy") ||
			(frame.mode !== undefined && !identifier(frame.mode))) {
			this.counts.invalidFrames++;
			return;
		}
		const attachmentKey = JSON.stringify([frame.paneId, frame.attachmentId]);
		let attachment = this.attachments.get(attachmentKey);
		if (!attachment) {
			if (this.attachments.size >= this.limits.attachments) { this.counts.attachmentOverflow++; return; }
			attachment = { sequence: -1, gpuSampleId: -1 };
			this.attachments.set(attachmentKey, attachment);
		}
		if (frame.sequence <= attachment.sequence) {
			if (frame.sequence === attachment.sequence) this.counts.duplicateSubmissions++;
			else this.counts.invalidFrames++;
			return;
		}
		attachment.sequence = frame.sequence;
		this.counts.helperSubmissions++;
		const transport = frame.transport ?? "unavailable";
		const mode = frame.mode ?? "unavailable";
		if (mode === "unavailable") this.counts.unavailableMode++;
		if (transport === "unavailable") this.counts.unavailableTransport++;
		const windowEpochSecond = Math.floor(frame.presentedEpochMicros / 1_000_000);
		const key = JSON.stringify([windowEpochSecond, frame.paneId, frame.attachmentId, frame.quality, frame.followPreload, mode, frame.width, frame.height, transport]);
		let row = this.rows.get(key);
		if (!row && this.rows.size < this.limits.rows) {
			row = { windowEpochSecond, paneId: frame.paneId, attachmentId: frame.attachmentId,
				quality: frame.quality, followPreload: frame.followPreload, mode, width: frame.width, height: frame.height, transport,
				helperSubmissionCount: 0, cpuWallMicrosSum: 0, acquireWallMicrosSum: 0,
				gpuSampleCount: 0, gpuSampleMicrosSum: 0, gpuSampleMicrosMax: 0 };
			this.rows.set(key, row);
		}
		if (row) {
			row.helperSubmissionCount++;
			row.cpuWallMicrosSum += frame.cpuMicros;
			row.acquireWallMicrosSum += frame.acquireMicros;
		} else this.counts.rowOverflow++;
		const id = frame.gpuSampleId;
		if (id === undefined || id === null) { this.counts.unavailableGpuSamples++; return; }
		if (!natural(id)) { this.counts.invalidGpuSamples++; return; }
		if (id <= attachment.gpuSampleId) { this.counts.repeatedOrStaleGpuSamples++; return; }
		// Consume even a mismatching sample so another mode/window cannot later claim it.
		attachment.gpuSampleId = id;
		if (frame.gpuSampleMatchesFrame !== true) { this.counts.rejectedGpuProvenance++; return; }
		if (frame.gpuMicros === null) { this.counts.unavailableGpuSamples++; return; }
		if (!duration(frame.gpuMicros)) { this.counts.invalidGpuSamples++; return; }
		if (row) {
			row.gpuSampleCount++;
			row.gpuSampleMicrosSum += frame.gpuMicros;
			row.gpuSampleMicrosMax = Math.max(row.gpuSampleMicrosMax, frame.gpuMicros);
		}
	}

	report() {
		return {
			compositor: this.compositor.report(),
			limits: { ...this.limits }, counts: { ...this.counts }, attachmentCount: this.attachments.size,
			measurementBoundaries: nativeStageMeasurementBoundaries,
			rows: [...this.rows.values()].map((row) => ({ ...row,
				gpuSampleMicrosMean: row.gpuSampleCount ? row.gpuSampleMicrosSum / row.gpuSampleCount : null })),
		};
	}
}
