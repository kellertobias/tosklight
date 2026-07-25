import type { ShowObject } from "../showObjects/contracts";
import type { ShowObjectsStore } from "../showObjects/store";
import type {
	GroupManagementActions,
	GroupManagementOutcome,
	GroupManagementRequest,
	GroupManagementTransport,
	ManageGroupInput,
} from "./contracts";

export interface GroupManagementWriterOptions {
	showId: string;
	store: ShowObjectsStore;
	transport: GroupManagementTransport;
	loadGroup(
		showId: string,
		objectId: string,
	): Promise<ShowObject<"group"> | null>;
	onError?: (error: Error | null) => void;
}

/**
 * Serializes one Group management request at a time against the authoritative Show Objects store.
 *
 * Reconciliation is order-independent: `settlePending` installs the authoritative body only when
 * the owning Show event has not already been applied, so a response arriving before or after its
 * event converges on the same state.
 */
export class GroupManagementWriter implements GroupManagementActions {
	private stopped = false;
	private queue: Promise<unknown> = Promise.resolve();

	constructor(private readonly options: GroupManagementWriterOptions) {}

	manage(input: ManageGroupInput) {
		const run = this.queue.then(() => this.send(input));
		this.queue = run.catch(() => undefined);
		return run;
	}

	stop() {
		this.stopped = true;
	}

	private async send(input: ManageGroupInput) {
		if (this.stopped) return null;
		if (!this.options.store.isCollectionReady("group")) {
			this.options.onError?.(
				new Error("Authoritative Group collection is still loading"),
			);
			return null;
		}
		const generation = this.options.store.getSnapshot().authorityGeneration;
		const request = managementRequest(input);
		const token = this.options.store.beginPending(
			this.options.showId,
			"group",
			input.objectId,
		);
		try {
			const outcome = await this.dispatch(request);
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
			this.options.onError?.(
				outcome.persistenceWarning ? new Error(outcome.persistenceWarning) : null,
			);
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

	private async dispatch(request: GroupManagementRequest) {
		try {
			return await this.options.transport.manage(this.options.showId, request);
		} catch (reason) {
			if (!transportFailure(reason)?.retryable) throw reason;
			return this.options.transport.manage(this.options.showId, request);
		}
	}

	/** A revision conflict repairs from the authoritative object, never from bootstrap state. */
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

function managementRequest(input: ManageGroupInput): GroupManagementRequest {
	return {
		requestId: crypto.randomUUID(),
		groupId: input.objectId,
		operation: input.operation,
		expectedObjectRevision: input.expectedObjectRevision,
	};
}

function assertOutcome(
	request: GroupManagementRequest,
	outcome: GroupManagementOutcome,
) {
	if (outcome.requestId !== request.requestId)
		throw new Error("Group management response request ID does not match");
	if (outcome.group.id !== request.groupId)
		throw new Error("Group management response object ID does not match");
	const expectedRevision =
		outcome.status === "changed"
			? request.expectedObjectRevision + 1
			: request.expectedObjectRevision;
	if (outcome.group.revision !== expectedRevision)
		throw new Error("Group management response revision is inconsistent");
	if (request.operation.type === "undo" && outcome.status !== "changed")
		throw new Error("Group undo must report an authoritative change");
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
