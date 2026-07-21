import type {
	SpeedGroupAction,
	SpeedGroupActionOutcome,
	SpeedGroupActionRequest,
	SpeedGroupId,
	SpeedGroupRuntimeActions,
	SpeedGroupRuntimeScope,
} from "./contracts";
import { assertAction, assertRequestId } from "./projectionValue";
import type { SpeedGroupRuntimeStore } from "./store";
import type { SpeedGroupSettlement } from "./storeState";
import type { SpeedGroupRuntimeTransport } from "./transport";
import { SpeedGroupProtocolError, SpeedGroupTransportError } from "./transport";

interface QueuedSpeedGroupWrite {
	requestId: string;
	action: SpeedGroupAction;
	resolve(outcome: SpeedGroupActionOutcome | null): void;
}

export interface SpeedGroupWriterOptions {
	scope: SpeedGroupRuntimeScope;
	store: SpeedGroupRuntimeStore;
	transport: SpeedGroupRuntimeTransport;
	repair(error: Error): Promise<void>;
	onError?: (error: Error | null) => void;
}

/** One optimistic FIFO for all five installation-global manual groups. */
export class SpeedGroupRuntimeWriter implements SpeedGroupRuntimeActions {
	private readonly queue: QueuedSpeedGroupWrite[] = [];
	private storeScope: number | null = null;
	private running = false;
	private stopped = false;

	constructor(private readonly options: SpeedGroupWriterOptions) {}

	setBpm(group: SpeedGroupId, bpm: number, requestId?: string) {
		return this.enqueue({ type: "set_bpm", group, bpm }, requestId);
	}

	adjustBpm(group: SpeedGroupId, deltaBpm: number, requestId?: string) {
		return this.enqueue({ type: "adjust_bpm", group, deltaBpm }, requestId);
	}

	synchronize(source: SpeedGroupId, target: SpeedGroupId, requestId?: string) {
		return this.enqueue({ type: "synchronize", source, target }, requestId);
	}

	stop() {
		this.stopped = true;
		for (const write of this.queue) {
			this.abandon(write.requestId);
			write.resolve(null);
		}
		this.queue.length = 0;
	}

	private enqueue(action: SpeedGroupAction, suppliedRequestId?: string) {
		const requestId = suppliedRequestId ?? crypto.randomUUID();
		if (this.stopped || !this.claimScope()) return Promise.resolve(null);
		try {
			assertRequestId(requestId);
			assertAction(action);
			if (
				!this.options.store.beginOptimistic(
					{ requestId, action },
					this.expectedStoreScope(),
				)
			)
				return this.refuse("Authoritative Speed Groups are unavailable");
		} catch (reason) {
			return this.refuse(asError(reason).message);
		}
		return new Promise<SpeedGroupActionOutcome | null>((resolve) => {
			this.queue.push({ requestId, action, resolve });
			this.start();
		});
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

	private async send(write: QueuedSpeedGroupWrite) {
		if (!this.isPending(write.requestId)) return this.abandon(write.requestId);
		try {
			const request = this.requestAtCurrentAuthority(write);
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

	private requestAtCurrentAuthority(
		write: QueuedSpeedGroupWrite,
	): SpeedGroupActionRequest {
		const authority = this.options.store.authority(this.expectedStoreScope());
		if (!authority)
			throw new Error("Authoritative Speed Groups are unavailable");
		return {
			requestId: write.requestId,
			expectedAuthorityId: authority.authorityId,
			expectedRevision: authority.revision,
			action: write.action,
		};
	}

	private async requestWithOneRetry(request: SpeedGroupActionRequest) {
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

	private async settle(requestId: string, outcome: SpeedGroupActionOutcome) {
		let settlement = this.settleOutcome(requestId, outcome);
		if (settlement === "ignored") return false;
		if (settlement === "settled") return true;
		await this.options.repair(
			new SpeedGroupProtocolError(
				"Speed Group outcome requires snapshot repair",
			),
		);
		settlement = this.settleOutcome(requestId, outcome);
		if (settlement === "ignored") return false;
		if (settlement === "repair")
			throw new SpeedGroupProtocolError(
				"Speed Group outcome still conflicts after repair",
			);
		return true;
	}

	private settleOutcome(
		requestId: string,
		outcome: SpeedGroupActionOutcome,
	): SpeedGroupSettlement {
		return outcome.status === "changed"
			? this.options.store.settleChanged(
					requestId,
					outcome,
					this.expectedStoreScope(),
				)
			: this.options.store.settleNoChange(
					requestId,
					outcome,
					this.expectedStoreScope(),
				);
	}

	private assertOutcome(
		request: SpeedGroupActionRequest,
		outcome: SpeedGroupActionOutcome,
	) {
		if (outcome.requestId !== request.requestId)
			throw new SpeedGroupProtocolError(
				"Speed Group response request identity does not match",
			);
		if (!sameId(outcome.authorityId, request.expectedAuthorityId))
			throw new SpeedGroupProtocolError(
				"Speed Group response belongs to another authority",
			);
	}

	private async repairError(error: Error) {
		try {
			await this.options.repair(error);
			return error;
		} catch (reason) {
			return new Error(`Speed Group repair failed: ${asError(reason).message}`);
		}
	}

	private claimScope() {
		const state = this.options.store.getSnapshot();
		if (!sameId(state.deskId, this.options.scope.deskId)) return false;
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
	return reason instanceof SpeedGroupTransportError && reason.retryable;
}

function requiresRepair(reason: unknown) {
	return (
		reason instanceof SpeedGroupProtocolError ||
		(reason instanceof SpeedGroupTransportError && reason.status === 409)
	);
}

function asError(reason: unknown) {
	return reason instanceof Error ? reason : new Error(String(reason));
}
