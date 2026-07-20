import type { ShowObject } from "../showObjects/contracts";
import type { ShowObjectsStore } from "../showObjects/store";
import type {
	GroupRecordingActions,
	GroupRecordingOutcome,
	GroupRecordingRequest,
	GroupRecordingTransport,
	RecordGroupInput,
} from "./contracts";

export interface GroupRecordingWriterOptions {
	showId: string;
	store: ShowObjectsStore;
	transport: GroupRecordingTransport;
	loadGroup(
		showId: string,
		objectId: string,
	): Promise<ShowObject<"group"> | null>;
	onError?: (error: Error | null) => void;
}

/** Replays one action request without reading or serializing Programmer state. */
export class GroupRecordingWriter implements GroupRecordingActions {
	private stopped = false;

	constructor(private readonly options: GroupRecordingWriterOptions) {}

	async record(input: RecordGroupInput) {
		if (this.stopped) return null;
		if (!this.options.store.isCollectionReady("group")) {
			this.options.onError?.(
				new Error("Authoritative Group collection is still loading"),
			);
			return null;
		}
		const generation = this.options.store.getSnapshot().authorityGeneration;
		const request = recordingRequest(input);
		const token = this.options.store.beginPending(
			this.options.showId,
			"group",
			input.objectId,
		);
		try {
			const outcome = await this.send(request);
			if (this.stopped) return this.abandon(token);
			assertOutcome(request, outcome);
			const settled = this.options.store.settlePending(
				token,
				{
					objectId: outcome.group.id,
					revision: outcome.group.revision,
					object: outcome.group.object,
				},
				outcome.showRevision,
				outcome.status === "changed" ? outcome.eventSequence : null,
				generation,
			);
			if (!settled) return null;
			this.options.onError?.(null);
			return outcome;
		} catch (reason) {
			if (!this.isCurrent(generation)) return this.abandon(token);
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

	private async send(request: GroupRecordingRequest) {
		try {
			return await this.options.transport.record(this.options.showId, request);
		} catch (reason) {
			if (!transportFailure(reason)?.retryable) throw reason;
			return this.options.transport.record(this.options.showId, request);
		}
	}

	private async repairConflict(
		error: Error,
		objectId: string,
		generation: number,
	) {
		if (transportFailure(error)?.status !== 409) return;
		const stamp = this.options.store.captureObjectAuthority(
			this.options.showId,
			"group",
			objectId,
		);
		if (!stamp || stamp.authorityGeneration !== generation) return;
		try {
			const group = await this.options.loadGroup(this.options.showId, objectId);
			this.options.store.installObjectIfAuthorityUnchanged(stamp, group);
		} catch {
			// The original revision conflict remains the actionable error.
		}
	}

	private isCurrent(generation: number) {
		return (
			!this.stopped &&
			this.options.store.getSnapshot().authorityGeneration === generation
		);
	}

	private abandon(token: string): null {
		this.options.store.abandon(token);
		return null;
	}
}

function recordingRequest(input: RecordGroupInput): GroupRecordingRequest {
	return {
		requestId: crypto.randomUUID(),
		groupId: input.objectId,
		operation: input.operation,
		expectedObjectRevision: input.expectedObjectRevision,
	};
}

function assertOutcome(
	request: GroupRecordingRequest,
	outcome: GroupRecordingOutcome,
) {
	if (outcome.requestId !== request.requestId)
		throw new Error("Group recording response request ID does not match");
	if (outcome.group.id !== request.groupId)
		throw new Error("Group recording response object ID does not match");
	if (outcome.status === "no_change" && outcome.group.state !== "stored")
		throw new Error("Group no-change outcome must retain a stored projection");
	if (
		request.operation !== "subtract" &&
		(request.operation === "delete") !== (outcome.group.state === "deleted")
	)
		throw new Error("Group recording projection does not match its operation");
	const expectedRevision =
		outcome.status === "changed"
			? request.expectedObjectRevision + 1
			: request.expectedObjectRevision;
	if (outcome.group.revision !== expectedRevision)
		throw new Error("Group recording response revision is inconsistent");
}

function asError(reason: unknown) {
	return reason instanceof Error ? reason : new Error(String(reason));
}

function transportFailure(reason: unknown) {
	if (!(reason instanceof Error)) return null;
	const failure = reason as Error & { status?: unknown; retryable?: unknown };
	if (
		typeof failure.status !== "number" ||
		typeof failure.retryable !== "boolean"
	)
		return null;
	return { status: failure.status, retryable: failure.retryable };
}
