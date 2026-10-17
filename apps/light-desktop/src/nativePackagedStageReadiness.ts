import type {
	StagePaneBenchmarkHeartbeat,
	StagePaneBenchmarkObservations,
	StagePaneBenchmarkSample,
} from "./platform/desktop/types";

const LIFECYCLE_MILLIS = 17_000;
const FRESH_MILLIS = 5_000;
const MAX_WITNESSES = 512;
const key = (row: { paneId: string; attachmentId: string }) =>
	`${row.paneId}\0${row.attachmentId}`;
const integer = (value: number) => Number.isSafeInteger(value) && value >= 0;
const owned = (row: { paneId: string; attachmentId: string }) =>
	typeof row?.paneId === "string" &&
	row.paneId.startsWith("main:") &&
	row.paneId.length <= 256 &&
	typeof row.attachmentId === "string" &&
	row.attachmentId.length > 0 &&
	row.attachmentId.length <= 128;
const wellFormedFrame = (row: StagePaneBenchmarkSample) =>
	owned(row) &&
	integer(row.sequence) &&
	integer(row.sourceFrame) &&
	integer(row.presentedEpochMicros) &&
	integer(row.width) &&
	integer(row.height) &&
	integer(row.drawCalls) &&
	typeof row.followPreload === "boolean";
const validFrame = (row: StagePaneBenchmarkSample) =>
	wellFormedFrame(row) &&
	row.sequence > 0 &&
	row.sourceFrame > 0 &&
	row.width > 0 &&
	row.height > 0 &&
	row.drawCalls > 0;
const wellFormedHeartbeat = (row: StagePaneBenchmarkHeartbeat) =>
	owned(row) &&
	typeof row.followPreload === "boolean" &&
	integer(row.sequence) &&
	integer(row.lastPresentedSequence) &&
	integer(row.sourceFrame) &&
	integer(row.epochMicros) &&
	integer(row.receivedEpochMicros) &&
	row.receivedEpochMicros >= row.epochMicros &&
	integer(row.width) &&
	integer(row.height) &&
	typeof row.attached === "boolean" &&
	typeof row.visible === "boolean" &&
	typeof row.idleUnchanged === "boolean" &&
	typeof row.drawHealthy === "boolean";

export interface NativeStageReadyLane {
	frame: StagePaneBenchmarkSample;
	heartbeat: StagePaneBenchmarkHeartbeat;
	previousLiveFrame: StagePaneBenchmarkSample | null;
}

export interface NativeStageReadyEvidence {
	schemaVersion: 2;
	kind: "stage-ready";
	measurementSurface: "packaged-tauri-native-stage";
	lifecycleStartedEpochMillis: number;
	eligibleEpochMillis: number;
	readyEpochMillis: number;
	lanes: [NativeStageReadyLane, NativeStageReadyLane];
}

function matchedLane(
	frame: StagePaneBenchmarkSample,
	heartbeat: StagePaneBenchmarkHeartbeat,
	eligible: number,
	now: number,
): boolean {
	return (
		validFrame(frame) &&
		owned(heartbeat) &&
		key(frame) === key(heartbeat) &&
		heartbeat.attached === true &&
		heartbeat.visible === true &&
		heartbeat.drawHealthy === true &&
		integer(heartbeat.sequence) &&
		heartbeat.sequence > 0 &&
		heartbeat.lastPresentedSequence === frame.sequence &&
		heartbeat.sourceFrame === frame.sourceFrame &&
		heartbeat.followPreload === frame.followPreload &&
		heartbeat.width === frame.width &&
		heartbeat.height === frame.height &&
		integer(heartbeat.epochMicros) &&
		integer(heartbeat.receivedEpochMicros) &&
		heartbeat.epochMicros / 1000 >= eligible &&
		heartbeat.receivedEpochMicros >= heartbeat.epochMicros &&
		heartbeat.receivedEpochMicros - heartbeat.epochMicros <=
			FRESH_MILLIS * 1000 &&
		now >= heartbeat.receivedEpochMicros / 1000 &&
		now - heartbeat.receivedEpochMicros / 1000 <= FRESH_MILLIS &&
		frame.presentedEpochMicros <= heartbeat.epochMicros
	);
}

/** Passive, private one-shot readiness. It never requests a frame or changes Stage settings. */
export class NativeStageReadiness {
	private readonly latestBefore = new Map<string, StagePaneBenchmarkSample>();
	private readonly frames = new Map<string, StagePaneBenchmarkSample>();
	private readonly heartbeats = new Map<boolean, StagePaneBenchmarkHeartbeat>();
	private readonly frameHistory = new Map<string, StagePaneBenchmarkSample>();
	private readonly heartbeatHistory = new Map<
		string,
		StagePaneBenchmarkHeartbeat
	>();
	private rejected = false;
	private claimed = false;
	private overflowed = false;

	observe(
		batch: StagePaneBenchmarkObservations,
		lifecycleStarted: number | null,
		now: number,
		failed: boolean,
	): NativeStageReadyEvidence | null {
		if (
			this.claimed ||
			this.rejected ||
			this.overflowed ||
			failed ||
			lifecycleStarted === null ||
			!integer(lifecycleStarted) ||
			!integer(now)
		)
			return null;
		const eligible = lifecycleStarted + LIFECYCLE_MILLIS;
		for (const frame of batch.frames) {
			if (!this.rememberFrame(frame, eligible, now)) continue;
			const identity = key(frame);
			if (frame.presentedEpochMicros / 1000 < eligible) {
				this.latestBefore.set(identity, frame);
				if (this.latestBefore.size > 4) this.overflowed = true;
			} else {
				this.frames.set(`${identity}\0${frame.sequence}`, frame);
			}
		}
		for (const heartbeat of batch.heartbeats) {
			if (!this.rememberHeartbeat(heartbeat, eligible, now)) continue;
			this.heartbeats.set(heartbeat.followPreload, heartbeat);
		}
		if (now < eligible) return null;
		for (const frame of this.latestBefore.values())
			this.frames.set(`${key(frame)}\0${frame.sequence}`, frame);
		if (this.frames.size > MAX_WITNESSES) this.overflowed = true;
		if (this.overflowed || this.rejected) return null;
		const live = this.lane(false, eligible, now),
			preload = this.lane(true, eligible, now);
		if (
			!live ||
			!preload ||
			live.frame.paneId === preload.frame.paneId ||
			live.frame.attachmentId === preload.frame.attachmentId
		)
			return null;
		this.claimed = true;
		return {
			schemaVersion: 2,
			kind: "stage-ready",
			measurementSurface: "packaged-tauri-native-stage",
			lifecycleStartedEpochMillis: lifecycleStarted,
			eligibleEpochMillis: eligible,
			readyEpochMillis: now,
			lanes: [live, preload],
		};
	}

	private rememberFrame(
		frame: StagePaneBenchmarkSample,
		eligible: number,
		now: number,
	): boolean {
		const warmup =
			owned(frame) &&
			typeof frame.followPreload === "boolean" &&
			integer(frame.presentedEpochMicros) &&
			frame.presentedEpochMicros / 1000 < eligible;
		if (
			!wellFormedFrame(frame) ||
			frame.presentedEpochMicros > (now + 1) * 1000
		) {
			if (now >= eligible && !warmup) this.rejected = true;
			return false;
		}
		const identity = key(frame),
			previous = this.frameHistory.get(identity);
		if (
			previous &&
			((frame.sequence <= previous.sequence &&
				(frame.sequence > 0 || previous.sequence > 0)) ||
				frame.sourceFrame < previous.sourceFrame ||
				frame.presentedEpochMicros <= previous.presentedEpochMicros ||
				frame.followPreload !== previous.followPreload)
		) {
			if (!warmup || previous.presentedEpochMicros / 1000 >= eligible)
				this.rejected = true;
			return false;
		}
		this.frameHistory.set(identity, frame);
		if (this.frameHistory.size > 4) this.overflowed = true;
		// A well-formed initial blank is history, never a painted or advancing witness.
		return validFrame(frame);
	}

	private rememberHeartbeat(
		heartbeat: StagePaneBenchmarkHeartbeat,
		eligible: number,
		now: number,
	): boolean {
		const warmup =
			owned(heartbeat) &&
			typeof heartbeat.followPreload === "boolean" &&
			integer(heartbeat.epochMicros) &&
			integer(heartbeat.receivedEpochMicros) &&
			heartbeat.epochMicros <= heartbeat.receivedEpochMicros &&
			heartbeat.receivedEpochMicros / 1000 < eligible;
		if (
			!wellFormedHeartbeat(heartbeat) ||
			heartbeat.receivedEpochMicros > (now + 1) * 1000
		) {
			if (now >= eligible && !warmup) this.rejected = true;
			return false;
		}
		const identity = key(heartbeat),
			previous = this.heartbeatHistory.get(identity);
		if (
			previous &&
			((heartbeat.sequence <= previous.sequence &&
				(heartbeat.sequence > 0 || previous.sequence > 0)) ||
				heartbeat.sourceFrame < previous.sourceFrame ||
				heartbeat.lastPresentedSequence < previous.lastPresentedSequence ||
				heartbeat.epochMicros <= previous.epochMicros ||
				heartbeat.receivedEpochMicros < previous.receivedEpochMicros ||
				heartbeat.followPreload !== previous.followPreload)
		) {
			if (!warmup || previous.receivedEpochMicros / 1000 >= eligible)
				this.rejected = true;
			return false;
		}
		this.heartbeatHistory.set(identity, heartbeat);
		if (this.heartbeatHistory.size > 4) this.overflowed = true;
		return true;
	}

	private lane(
		preload: boolean,
		eligible: number,
		now: number,
	): NativeStageReadyLane | null {
		const heartbeat = this.heartbeats.get(preload);
		if (!heartbeat || heartbeat.followPreload !== preload) return null;
		const frame = this.frames.get(
			`${key(heartbeat)}\0${heartbeat.lastPresentedSequence}`,
		);
		if (!frame || !matchedLane(frame, heartbeat, eligible, now)) return null;
		const freshPresentation =
			frame.presentedEpochMicros / 1000 >= eligible &&
			now >= frame.presentedEpochMicros / 1000 &&
			now - frame.presentedEpochMicros / 1000 <= FRESH_MILLIS;
		if (preload)
			return freshPresentation || heartbeat.idleUnchanged === true
				? { frame, heartbeat, previousLiveFrame: null }
				: null;
		if (!freshPresentation) return null;
		const previous = [...this.frames.values()].find(
			(row) =>
				key(row) === key(frame) &&
				row.followPreload === false &&
				row.sequence < frame.sequence &&
				row.sourceFrame < frame.sourceFrame &&
				row.presentedEpochMicros / 1000 >= eligible &&
				row.presentedEpochMicros < frame.presentedEpochMicros,
		);
		return previous ? { frame, heartbeat, previousLiveFrame: previous } : null;
	}
}
