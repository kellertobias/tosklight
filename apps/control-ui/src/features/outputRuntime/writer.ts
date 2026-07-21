import type {
	OutputRuntimeActionOutcome,
	OutputRuntimeActionRequest,
	OutputRuntimeActions,
	OutputRuntimeScope,
	SetOutputRuntimeInput,
} from "./contracts";
import { assertOutputMutation, assertOutputRequestId } from "./projectionValue";
import type { OutputRuntimeSettlement, OutputRuntimeStore } from "./store";
import type { OutputRuntimeTransport } from "./transport";
import {
	OutputRuntimeProtocolError,
	OutputRuntimeTransportError,
} from "./transport";

interface QueuedOutputWrite {
	requestId: string;
	grandMaster?: number;
	blackout?: boolean;
	resolve(outcome: OutputRuntimeActionOutcome | null): void;
}

export interface OutputRuntimeWriterOptions {
	scope: OutputRuntimeScope;
	store: OutputRuntimeStore;
	transport: OutputRuntimeTransport;
	repair(error: Error): Promise<void>;
	onError?: (error: Error | null) => void;
}

/** One optimistic FIFO; combined Grand Master and blackout stays one action. */
export class OutputRuntimeWriter implements OutputRuntimeActions {
	private readonly queue: QueuedOutputWrite[] = [];
	private storeScope: number | null = null;
	private running = false;
	private stopped = false;

	constructor(private readonly options: OutputRuntimeWriterOptions) {}

	setOutput(input: SetOutputRuntimeInput) {
		const requestId = input.requestId ?? crypto.randomUUID();
		if (this.stopped || !this.claimScope()) return Promise.resolve(null);
		try {
			assertOutputRequestId(requestId);
			assertOutputMutation(input.grandMaster, input.blackout);
			if (
				!this.options.store.beginOptimistic(
					{
						requestId,
						grandMaster: input.grandMaster,
						blackout: input.blackout,
					},
					this.expectedStoreScope(),
				)
			)
				return this.refuse("Authoritative Output runtime is unavailable");
		} catch (reason) {
			return this.refuse(asError(reason).message);
		}
		return new Promise<OutputRuntimeActionOutcome | null>((resolve) => {
			this.queue.push({
				requestId,
				grandMaster: input.grandMaster,
				blackout: input.blackout,
				resolve,
			});
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

	private async send(write: QueuedOutputWrite) {
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
		write: QueuedOutputWrite,
	): OutputRuntimeActionRequest {
		const expectedRevision = this.options.store.authoritativeRevision(
			this.expectedStoreScope(),
		);
		if (expectedRevision === null)
			throw new Error("Authoritative Output runtime is unavailable");
		return {
			requestId: write.requestId,
			expectedShowId: this.options.scope.showId,
			expectedRevision,
			grandMaster: write.grandMaster,
			blackout: write.blackout,
		};
	}

	private async requestWithOneRetry(request: OutputRuntimeActionRequest) {
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

	private async settle(requestId: string, outcome: OutputRuntimeActionOutcome) {
		let settlement = this.settleOutcome(requestId, outcome);
		if (settlement === "ignored") return false;
		if (settlement === "settled") return true;
		await this.options.repair(
			new OutputRuntimeProtocolError(
				"Output runtime outcome requires snapshot repair",
			),
		);
		settlement = this.settleOutcome(requestId, outcome);
		if (settlement === "ignored") return false;
		if (settlement === "repair")
			throw new OutputRuntimeProtocolError(
				"Output runtime outcome still conflicts after repair",
			);
		return true;
	}

	private settleOutcome(
		requestId: string,
		outcome: OutputRuntimeActionOutcome,
	): OutputRuntimeSettlement {
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
		request: OutputRuntimeActionRequest,
		outcome: OutputRuntimeActionOutcome,
	) {
		if (outcome.requestId !== request.requestId)
			throw new OutputRuntimeProtocolError(
				"Output response request identity does not match",
			);
		if (!sameId(outcome.projection.showId, this.options.scope.showId))
			throw new OutputRuntimeProtocolError(
				"Output response belongs to another Show",
			);
	}

	private async repairError(error: Error) {
		try {
			await this.options.repair(error);
			return error;
		} catch (reason) {
			return new Error(
				`Output runtime repair failed: ${asError(reason).message}`,
			);
		}
	}

	private claimScope() {
		const state = this.options.store.getSnapshot();
		if (
			!sameId(state.showId, this.options.scope.showId) ||
			!sameId(state.deskId, this.options.scope.deskId)
		)
			return false;
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

function sameId(left: string | null, right: string) {
	return left?.toLowerCase() === right.toLowerCase();
}

function isRetryable(reason: unknown) {
	return reason instanceof OutputRuntimeTransportError && reason.retryable;
}

function requiresRepair(reason: unknown) {
	return (
		reason instanceof OutputRuntimeProtocolError ||
		(reason instanceof OutputRuntimeTransportError && reason.status === 409)
	);
}

function asError(reason: unknown) {
	return reason instanceof Error ? reason : new Error(String(reason));
}
