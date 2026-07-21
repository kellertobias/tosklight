import type {
	ProgrammerPriorityActionOutcome,
	ProgrammerPriorityActionRequest,
	ProgrammerPriorityActions,
	ProgrammerPriorityScope,
	SetProgrammerPriorityInput,
} from "./contracts";
import { assertProgrammerPriority } from "./projectionValue";
import type {
	ProgrammerPrioritySettlement,
	ProgrammerPriorityStore,
} from "./store";
import type { ProgrammerPriorityTransport } from "./transport";
import {
	ProgrammerPriorityProtocolError,
	ProgrammerPriorityTransportError,
} from "./transport";

interface QueuedPriorityWrite {
	requestId: string;
	priority: number;
	resolve(outcome: ProgrammerPriorityActionOutcome | null): void;
}

export interface ProgrammerPriorityWriterOptions {
	scope: ProgrammerPriorityScope;
	store: ProgrammerPriorityStore;
	transport: ProgrammerPriorityTransport;
	repair(error: Error): Promise<void>;
	onError?: (error: Error | null) => void;
}

/** One optimistic replay-safe FIFO for the user's lightweight priority object. */
export class ProgrammerPriorityWriter implements ProgrammerPriorityActions {
	private readonly queue: QueuedPriorityWrite[] = [];
	private storeScope: number | null = null;
	private running = false;
	private stopped = false;

	constructor(private readonly options: ProgrammerPriorityWriterOptions) {}

	setPriority(input: SetProgrammerPriorityInput) {
		const requestId = input.requestId ?? crypto.randomUUID();
		if (this.stopped || !this.claimScope()) return Promise.resolve(null);
		try {
			assertProgrammerPriority(input.priority);
			if (
				!this.options.store.beginOptimistic(
					requestId,
					input.priority,
					this.expectedStoreScope(),
				)
			)
				return this.refuse("Authoritative Programmer priority is unavailable");
		} catch (reason) {
			return this.refuse(asError(reason).message);
		}
		return new Promise<ProgrammerPriorityActionOutcome | null>((resolve) => {
			this.queue.push({ requestId, priority: input.priority, resolve });
			this.start();
		});
	}

	stop() {
		this.stopped = true;
		for (const write of this.queue) {
			this.abandon(write.requestId);
			write.resolve(null);
		}
		this.queue.length = 0;
	}

	private start() {
		if (this.running) return;
		this.running = true;
		void this.drain();
	}

	private async drain() {
		while (!this.stopped && this.queue.length) {
			const write = this.queue[0];
			if (!write) break;
			const outcome = await this.send(write);
			if (this.queue[0] === write) this.queue.shift();
			write.resolve(outcome);
		}
		this.running = false;
	}

	private async send(write: QueuedPriorityWrite) {
		if (!this.isPending(write.requestId)) return this.abandon(write.requestId);
		try {
			const request = this.requestAtCurrentRevision(write);
			const outcome = await this.requestWithOneRetry(request);
			if (!this.isCurrent()) return this.abandon(write.requestId);
			this.assertOutcome(request, outcome);
			if (!(await this.settle(write.requestId, outcome))) return null;
			if (!this.isCurrent()) return null;
			this.options.onError?.(
				outcome.warning ? new Error(outcome.warning) : null,
			);
			return outcome;
		} catch (reason) {
			if (!this.isCurrent()) return this.abandon(write.requestId);
			const error = asError(reason);
			const reported = requiresRepair(reason)
				? await this.repairError(error)
				: error;
			if (!this.isCurrent()) return this.abandon(write.requestId);
			this.options.store.rollback(
				write.requestId,
				reported,
				this.expectedStoreScope(),
			);
			this.options.onError?.(reported);
			return null;
		}
	}

	private requestAtCurrentRevision(
		write: QueuedPriorityWrite,
	): ProgrammerPriorityActionRequest {
		const expectedRevision = this.options.store.authoritativeRevision(
			this.expectedStoreScope(),
		);
		if (expectedRevision === null)
			throw new Error("Authoritative Programmer priority is unavailable");
		return {
			requestId: write.requestId,
			expectedRevision,
			priority: write.priority,
		};
	}

	private async requestWithOneRetry(request: ProgrammerPriorityActionRequest) {
		try {
			return await this.options.transport.applyAction(
				this.options.scope,
				request,
			);
		} catch (reason) {
			if (!isRetryable(reason) || !this.isCurrent()) throw reason;
			return this.options.transport.applyAction(this.options.scope, request);
		}
	}

	private async settle(
		requestId: string,
		outcome: ProgrammerPriorityActionOutcome,
	) {
		let settlement = this.settleOutcome(requestId, outcome);
		if (settlement === "ignored") return false;
		if (settlement === "settled") return true;
		await this.options.repair(
			new ProgrammerPriorityProtocolError(
				"Programmer priority outcome requires snapshot repair",
			),
		);
		settlement = this.settleOutcome(requestId, outcome);
		if (settlement === "ignored") return false;
		if (settlement === "repair")
			throw new ProgrammerPriorityProtocolError(
				"Programmer priority outcome still conflicts after repair",
			);
		return true;
	}

	private settleOutcome(
		requestId: string,
		outcome: ProgrammerPriorityActionOutcome,
	): ProgrammerPrioritySettlement {
		return outcome.status === "changed"
			? this.options.store.settleChanged(
					requestId,
					outcome.projection,
					outcome.eventSequence,
					this.expectedStoreScope(),
				)
			: this.options.store.settleNoChange(
					requestId,
					outcome.projection,
					this.expectedStoreScope(),
				);
	}

	private assertOutcome(
		request: ProgrammerPriorityActionRequest,
		outcome: ProgrammerPriorityActionOutcome,
	) {
		if (outcome.requestId !== request.requestId)
			throw new ProgrammerPriorityProtocolError(
				"Programmer priority response request identity does not match",
			);
		if (outcome.projection.userId !== this.options.scope.userId)
			throw new ProgrammerPriorityProtocolError(
				"Programmer priority response belongs to another user",
			);
	}

	private async repairError(error: Error) {
		try {
			await this.options.repair(error);
			return error;
		} catch (reason) {
			return new Error(
				`Programmer priority repair failed: ${asError(reason).message}`,
			);
		}
	}

	private claimScope() {
		const state = this.options.store.getSnapshot();
		if (state.userId !== this.options.scope.userId) return false;
		this.storeScope ??= this.options.store.captureScope();
		return this.options.store.isScopeCurrent(this.expectedStoreScope());
	}

	private isCurrent() {
		return !this.stopped && this.claimScope();
	}

	private isPending(requestId: string) {
		return (
			this.isCurrent() &&
			this.options.store.hasOperation(requestId, this.expectedStoreScope())
		);
	}

	private abandon(requestId: string) {
		this.options.store.abandon(requestId, this.expectedStoreScope());
		return null;
	}

	private refuse(message: string) {
		this.options.onError?.(new Error(message));
		return Promise.resolve(null);
	}

	private expectedStoreScope() {
		return this.storeScope ?? -1;
	}
}

function isRetryable(reason: unknown) {
	return reason instanceof ProgrammerPriorityTransportError && reason.retryable;
}

function requiresRepair(reason: unknown) {
	return (
		reason instanceof ProgrammerPriorityProtocolError ||
		(reason instanceof ProgrammerPriorityTransportError &&
			reason.status === 409)
	);
}

function asError(reason: unknown) {
	return reason instanceof Error ? reason : new Error(String(reason));
}
