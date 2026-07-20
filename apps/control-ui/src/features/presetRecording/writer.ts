import type { ShowObject } from "../showObjects/contracts";
import type { ShowObjectsStore } from "../showObjects/store";
import type {
	PresetRecordingActions,
	PresetRecordingOutcome,
	PresetRecordingRequest,
	PresetRecordingTransport,
	RecordPresetInput,
} from "./contracts";

export interface PresetRecordingWriterOptions {
	showId: string;
	store: ShowObjectsStore;
	transport: PresetRecordingTransport;
	loadPreset(showId: string, objectId: string): Promise<ShowObject<"preset"> | null>;
	onError?: (error: Error | null) => void;
}

/** Replays one action request without ever materializing Programmer values. */
export class PresetRecordingWriter implements PresetRecordingActions {
	private stopped = false;

	constructor(private readonly options: PresetRecordingWriterOptions) {}

	async record(input: RecordPresetInput) {
		if (this.stopped) return null;
		if (!this.options.store.isCollectionReady("preset")) {
			this.options.onError?.(
				new Error("Authoritative Preset collection is still loading"),
			);
			return null;
		}
		const generation = this.options.store.getSnapshot().authorityGeneration;
		const request = recordingRequest(input);
		const token = this.options.store.beginPending(
			this.options.showId,
			"preset",
			input.objectId,
		);
		try {
			const outcome = await this.send(request);
			if (this.stopped) return this.abandon(token);
			assertOutcome(request, outcome);
			const settled = this.options.store.settlePending(
				token,
				outcome.preset,
				outcome.showRevision,
				outcome.status === "changed" ? outcome.eventSequence : null,
				generation,
			);
			if (!settled) return null;
			this.options.onError?.(null);
			return outcome;
		} catch (reason) {
			if (
				this.stopped ||
				this.options.store.getSnapshot().authorityGeneration !== generation
			)
				return this.abandon(token);
			const error = asError(reason);
			this.options.store.abandon(token);
			await this.repairConflict(error, input.objectId, generation);
			this.options.onError?.(error);
			return null;
		}
	}

	stop() {
		this.stopped = true;
	}

	private async send(request: PresetRecordingRequest) {
		try {
			return await this.options.transport.record(this.options.showId, request);
		} catch (reason) {
			if (!transportFailure(reason)?.retryable) throw reason;
			return this.options.transport.record(this.options.showId, request);
		}
	}

	private abandon(token: string): null {
		this.options.store.abandon(token);
		return null;
	}

	private async repairConflict(
		error: Error,
		objectId: string,
		generation: number,
	) {
		if (transportFailure(error)?.status !== 409) return;
		const before = this.options.store.getSnapshot();
		if (before.authorityGeneration !== generation) return;
		try {
			const preset = await this.options.loadPreset(this.options.showId, objectId);
			const after = this.options.store.getSnapshot();
			if (
				after.authorityGeneration !== generation ||
				after.eventSequence !== before.eventSequence
			)
				return;
			this.options.store.installObject(
				this.options.showId,
				"preset",
				preset,
				null,
				objectId,
			);
		} catch {
			// The original revision conflict remains the actionable error.
		}
	}
}

function recordingRequest(input: RecordPresetInput): PresetRecordingRequest {
	return {
		requestId: crypto.randomUUID(),
		address: input.address,
		name: input.name,
		mode: input.mode,
		expectedObjectRevision: input.expectedObjectRevision,
	};
}

function assertOutcome(
	request: PresetRecordingRequest,
	outcome: PresetRecordingOutcome,
) {
	if (outcome.requestId !== request.requestId)
		throw new Error("Preset recording response request ID does not match");
}

function asError(reason: unknown) {
	return reason instanceof Error ? reason : new Error(String(reason));
}

function transportFailure(reason: unknown) {
	if (!(reason instanceof Error)) return null;
	const failure = reason as Error & { status?: unknown; retryable?: unknown };
	if (typeof failure.status !== "number" || typeof failure.retryable !== "boolean")
		return null;
	return { status: failure.status, retryable: failure.retryable };
}
