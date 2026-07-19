import type {
	BatchProgrammerValuesInput,
	ProgrammerValuesActionOutcome,
	ProgrammerValuesActionRequest,
	ProgrammerValuesActions,
	ProgrammerValuesCommand,
	ProgrammerValuesScope,
	ReleaseProgrammerFixtureValueInput,
	ReleaseProgrammerGroupValueInput,
	SetProgrammerFixtureValueInput,
	SetProgrammerGroupValueInput,
} from "./contracts";
import { predictProgrammerValues } from "./prediction";
import type { ProgrammerValuesStore } from "./store";
import { ProgrammerValuesProtocolError } from "./transport";

interface QueuedValuesWrite {
	requestId: string;
	action: ProgrammerValuesCommand;
	resolve(outcome: ProgrammerValuesActionOutcome | null): void;
}

export interface ProgrammerValuesWriterOptions {
	scope: ProgrammerValuesScope;
	store: ProgrammerValuesStore;
	applyAction(
		scope: ProgrammerValuesScope,
		request: ProgrammerValuesActionRequest,
	): Promise<ProgrammerValuesActionOutcome>;
	repair(error: Error): Promise<void>;
	onError?: (error: Error | null) => void;
}

/** One replay-safe FIFO for every normal Programmer values mutation surface. */
export class ProgrammerValuesWriter implements ProgrammerValuesActions {
	private readonly queue: QueuedValuesWrite[] = [];
	private storeScope: number | null = null;
	private running = false;
	private stopped = false;

	constructor(private readonly options: ProgrammerValuesWriterOptions) {}

	setFixtureValue(input: SetProgrammerFixtureValueInput) {
		return this.enqueue(input.requestId, {
			action: "set_fixture",
			fixtureId: input.fixtureId,
			attribute: input.attribute,
			value: input.value,
			timing: timing(input),
		});
	}

	releaseFixtureValue(input: ReleaseProgrammerFixtureValueInput) {
		return this.enqueue(input.requestId, {
			action: "release_fixture",
			fixtureId: input.fixtureId,
			attribute: input.attribute,
		});
	}

	setGroupValue(input: SetProgrammerGroupValueInput) {
		return this.enqueue(input.requestId, {
			action: "set_group",
			groupId: input.groupId,
			attribute: input.attribute,
			value: input.value,
			timing: timing(input),
		});
	}

	releaseGroupValue(input: ReleaseProgrammerGroupValueInput) {
		return this.enqueue(input.requestId, {
			action: "release_group",
			groupId: input.groupId,
			attribute: input.attribute,
		});
	}

	batch(input: BatchProgrammerValuesInput) {
		return this.enqueue(input.requestId, {
			action: "batch",
			mutations: input.mutations,
		});
	}

	clear(requestId: string) {
		return this.enqueue(requestId, { action: "clear" });
	}

	stop() {
		this.stopped = true;
		for (const write of this.queue) write.resolve(null);
		this.queue.length = 0;
	}

	private enqueue(requestId: string, action: ProgrammerValuesCommand) {
		if (this.stopped || !this.claimScope()) return Promise.resolve(null);
		if (!requestId) {
			this.options.onError?.(new Error("A Programmer values request ID is required"));
			return Promise.resolve(null);
		}
		try {
			if (
				!this.options.store.beginOptimistic(
					requestId,
					predictProgrammerValues(action),
					this.expectedScope(),
				)
			)
				return Promise.resolve(null);
		} catch (reason) {
			this.options.onError?.(asError(reason));
			return Promise.resolve(null);
		}
		return new Promise<ProgrammerValuesActionOutcome | null>((resolve) => {
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
			const write = this.queue.shift();
			if (!write) break;
			write.resolve(await this.send(write));
		}
		this.running = false;
	}

	private async send(write: QueuedValuesWrite) {
		if (!this.scopeIsCurrent()) return null;
		try {
			const request = this.requestAtCurrentRevision(write);
			const outcome = await this.requestWithOneReplay(request);
			if (!this.scopeIsCurrent()) return null;
			this.assertResponse(write.requestId, outcome);
			await this.settle(write.requestId, outcome);
			if (!this.scopeIsCurrent()) return null;
			this.options.onError?.(
				outcome.warning ? new Error(outcome.warning) : null,
			);
			return outcome;
		} catch (reason) {
			if (!this.scopeIsCurrent()) return null;
			const error = asError(reason);
			const reported = requiresRepair(reason)
				? await this.repairError(error)
				: error;
			this.options.store.rollback(
				write.requestId,
				reported,
				this.expectedScope(),
			);
			this.options.onError?.(reported);
			return null;
		}
	}

	private requestAtCurrentRevision(
		write: QueuedValuesWrite,
	): ProgrammerValuesActionRequest {
		const expectedRevision = this.options.store.authoritativeRevision(
			this.expectedScope(),
		);
		if (expectedRevision == null)
			throw new Error("Authoritative Programmer values are unavailable");
		return {
			requestId: write.requestId,
			expectedRevision,
			action: write.action,
		};
	}

	private async requestWithOneReplay(request: ProgrammerValuesActionRequest) {
		try {
			return await this.options.applyAction(this.options.scope, request);
		} catch (reason) {
			if (!isReplayable(reason) || !this.scopeIsCurrent()) throw reason;
			return this.options.applyAction(this.options.scope, request);
		}
	}

	private async settle(
		requestId: string,
		outcome: ProgrammerValuesActionOutcome,
	) {
		let settlement =
			outcome.status === "changed"
				? this.options.store.settleChanged(
						requestId,
						outcome.projection,
						outcome.eventSequence,
						this.expectedScope(),
					)
				: this.options.store.settleNoChange(
						requestId,
						outcome.revision,
						this.expectedScope(),
					);
		if (settlement !== "repair") return;
		await this.repair(new ProgrammerValuesProtocolError("Programmer values outcome requires repair"));
		settlement = this.settleAfterRepair(requestId, outcome);
		if (settlement === "repair")
			throw new ProgrammerValuesProtocolError(
				"Programmer values outcome still conflicts after repair",
			);
	}

	private settleAfterRepair(
		requestId: string,
		outcome: ProgrammerValuesActionOutcome,
	) {
		return outcome.status === "changed"
			? this.options.store.settleChanged(
					requestId,
					outcome.projection,
					outcome.eventSequence,
					this.expectedScope(),
				)
			: this.options.store.settleNoChange(
					requestId,
					outcome.revision,
					this.expectedScope(),
				);
	}

	private assertResponse(
		requestId: string,
		outcome: ProgrammerValuesActionOutcome,
	) {
		if (outcome.requestId !== requestId)
			throw new ProgrammerValuesProtocolError(
				"Programmer values response request identity does not match",
			);
		if (outcome.status === "changed" && outcome.projection.revision !== outcome.revision)
			throw new ProgrammerValuesProtocolError(
				"Programmer values response revisions do not match",
			);
	}

	private async repair(error: Error) {
		try {
			await this.options.repair(error);
		} catch (reason) {
			throw new Error(
				`Programmer values repair failed: ${asError(reason).message}`,
			);
		}
	}

	private async repairError(error: Error) {
		try {
			await this.options.repair(error);
			return error;
		} catch (reason) {
			return new Error(
				`Programmer values repair failed: ${asError(reason).message}`,
			);
		}
	}

	private claimScope() {
		const state = this.options.store.getSnapshot();
		if (
			state.showId !== this.options.scope.showId ||
			state.userId !== this.options.scope.userId
		)
			return false;
		this.storeScope ??= this.options.store.captureScope();
		return true;
	}

	private scopeIsCurrent() {
		return (
			!this.stopped &&
			this.claimScope() &&
			this.options.store.isScopeCurrent(this.expectedScope())
		);
	}

	private expectedScope() {
		return this.storeScope ?? -1;
	}
}

function timing(input: {
	fade: boolean;
	fadeMillis: number | null;
	delayMillis: number | null;
}) {
	return {
		fade: input.fade,
		fadeMillis: input.fadeMillis,
		delayMillis: input.delayMillis,
	};
}

function isReplayable(reason: unknown) {
	if (!reason || typeof reason !== "object") return true;
	if ("retryable" in reason) return (reason as { retryable?: unknown }).retryable === true;
	return !("status" in reason);
}

function requiresRepair(reason: unknown) {
	if (!reason || typeof reason !== "object") return true;
	const status = "status" in reason ? (reason as { status?: unknown }).status : null;
	return status === null || status === 408 || status === 409 || (typeof status === "number" && status >= 500);
}

function asError(reason: unknown) {
	return reason instanceof Error ? reason : new Error(String(reason));
}
