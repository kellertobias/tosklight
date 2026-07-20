import type { ProgrammerCaptureModeStore } from "../programmerCaptureMode/store";
import type {
	BatchProgrammerPreloadValuesInput,
	ProgrammerPreloadValuesActionOutcome,
	ProgrammerPreloadValuesActionRequest,
	ProgrammerPreloadValuesActions,
	ProgrammerPreloadValuesCommand,
	ProgrammerPreloadValuesScope,
	ReleaseProgrammerPreloadFixtureValueInput,
	ReleaseProgrammerPreloadGroupValueInput,
	SetProgrammerPreloadFixtureValueInput,
	SetProgrammerPreloadGroupValueInput,
} from "./contracts";
import { predictProgrammerPreloadValues } from "./prediction";
import type { ProgrammerPreloadValuesStore } from "./store";
import { ProgrammerPreloadValuesProtocolError } from "./transport";
import {
	awaitPreloadAuthorityRepairs,
	ProgrammerPreloadCaptureAuthority,
} from "./writerCaptureAuthority";
import {
	isReplayablePreloadError,
	preloadValuesError,
	preloadValuesReadinessError,
	requiresPreloadAuthorityRepair,
} from "./writerPolicy";

interface QueuedPreloadWrite {
	requestId: string;
	action: ProgrammerPreloadValuesCommand;
	expectedCaptureModeRevision: number;
	resolve(outcome: ProgrammerPreloadValuesActionOutcome | null): void;
}

export interface ProgrammerPreloadValuesWriterOptions {
	scope: ProgrammerPreloadValuesScope;
	store: ProgrammerPreloadValuesStore;
	captureModeStore: ProgrammerCaptureModeStore;
	applyAction(
		scope: ProgrammerPreloadValuesScope,
		request: ProgrammerPreloadValuesActionRequest,
	): Promise<ProgrammerPreloadValuesActionOutcome>;
	repair(error: Error): Promise<void>;
	repairCaptureMode(error: Error): Promise<void>;
	onError?: (error: Error | null) => void;
}

/** One replay-safe FIFO for active Preload capture mutations. */
export class ProgrammerPreloadValuesWriter
	implements ProgrammerPreloadValuesActions
{
	private readonly queue: QueuedPreloadWrite[] = [];
	private readonly captureAuthority: ProgrammerPreloadCaptureAuthority;
	private storeScope: number | null = null;
	private running = false;
	private stopped = false;

	constructor(private readonly options: ProgrammerPreloadValuesWriterOptions) {
		this.captureAuthority = new ProgrammerPreloadCaptureAuthority({
			scope: options.scope,
			store: options.captureModeStore,
			repair: options.repairCaptureMode,
		});
	}

	setFixtureValue(input: SetProgrammerPreloadFixtureValueInput) {
		return this.enqueue(input.requestId, {
			action: "set_fixture",
			fixtureId: input.fixtureId,
			attribute: input.attribute,
			value: input.value,
			timing: timing(input),
		});
	}

	releaseFixtureValue(input: ReleaseProgrammerPreloadFixtureValueInput) {
		return this.enqueue(input.requestId, {
			action: "release_fixture",
			fixtureId: input.fixtureId,
			attribute: input.attribute,
		});
	}

	setGroupValue(input: SetProgrammerPreloadGroupValueInput) {
		return this.enqueue(input.requestId, {
			action: "set_group",
			groupId: input.groupId,
			attribute: input.attribute,
			value: input.value,
			timing: timing(input),
		});
	}

	releaseGroupValue(input: ReleaseProgrammerPreloadGroupValueInput) {
		return this.enqueue(input.requestId, {
			action: "release_group",
			groupId: input.groupId,
			attribute: input.attribute,
		});
	}

	batch(input: BatchProgrammerPreloadValuesInput) {
		return this.enqueue(input.requestId, {
			action: "batch",
			mutations: input.mutations,
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

	private enqueue(requestId: string, action: ProgrammerPreloadValuesCommand) {
		if (this.stopped || !this.claimScopes()) return Promise.resolve(null);
		if (!requestId)
			return this.refuse("A Preload Programmer values request ID is required");
		const valuesError = this.valuesReadinessError();
		if (valuesError) return this.refuse(valuesError.message);
		const captureMode = this.captureAuthority.readyProjection();
		if (!captureMode)
			return this.refuse(
				"Authoritative Programmer capture mode is unavailable",
			);
		const captureError = this.captureAuthority.preconditionError(
			captureMode.revision,
		);
		if (captureError) return this.refuse(captureError.message);
		try {
			if (
				!this.options.store.beginOptimistic(
					requestId,
					predictProgrammerPreloadValues(action),
					this.expectedStoreScope(),
				)
			)
				return Promise.resolve(null);
		} catch (reason) {
			this.options.onError?.(preloadValuesError(reason));
			return Promise.resolve(null);
		}
		return new Promise<ProgrammerPreloadValuesActionOutcome | null>(
			(resolve) => {
				this.queue.push({
					requestId,
					action,
					expectedCaptureModeRevision: captureMode.revision,
					resolve,
				});
				this.start();
			},
		);
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

	private async send(write: QueuedPreloadWrite) {
		if (!this.scopesAreCurrent()) return this.abandon(write.requestId);
		const precondition =
			this.valuesReadinessError() ??
			this.captureAuthority.preconditionError(
				write.expectedCaptureModeRevision,
			);
		if (precondition) return this.rollback(write.requestId, precondition);
		try {
			const request = this.requestAtCurrentRevision(write);
			const outcome = await this.requestWithOneReplay(request);
			if (!this.scopesAreCurrent()) return this.abandon(write.requestId);
			this.assertResponse(request, outcome);
			await this.settle(write.requestId, outcome);
			if (!this.scopesAreCurrent()) return null;
			this.options.onError?.(
				outcome.warning ? new Error(outcome.warning) : null,
			);
			return outcome;
		} catch (reason) {
			if (!this.scopesAreCurrent()) return this.abandon(write.requestId);
			const error = preloadValuesError(reason);
			const reported = requiresPreloadAuthorityRepair(reason)
				? await this.repairError(error)
				: error;
			if (!this.scopesAreCurrent()) return this.abandon(write.requestId);
			return this.rollback(write.requestId, reported);
		}
	}

	private requestAtCurrentRevision(
		write: QueuedPreloadWrite,
	): ProgrammerPreloadValuesActionRequest {
		const expectedPreloadRevision = this.options.store.authoritativeRevision(
			this.expectedStoreScope(),
		);
		if (expectedPreloadRevision == null)
			throw new Error(
				"Authoritative Preload Programmer values are unavailable",
			);
		return {
			requestId: write.requestId,
			expectedPreloadRevision,
			expectedCaptureModeRevision: write.expectedCaptureModeRevision,
			action: write.action,
		};
	}

	private async requestWithOneReplay(
		request: ProgrammerPreloadValuesActionRequest,
	) {
		try {
			return await this.options.applyAction(this.options.scope, request);
		} catch (reason) {
			if (!isReplayablePreloadError(reason) || !this.scopesAreCurrent())
				throw reason;
			return this.options.applyAction(this.options.scope, request);
		}
	}

	private async settle(
		requestId: string,
		outcome: ProgrammerPreloadValuesActionOutcome,
	) {
		let settlement = this.settleOutcome(requestId, outcome);
		if (settlement === "settled") return;
		if (settlement === "ignored")
			throw new ProgrammerPreloadValuesProtocolError(
				"Preload Programmer values outcome lost its pending request",
			);
		await this.repairAuthorities(
			new ProgrammerPreloadValuesProtocolError(
				"Preload Programmer values outcome requires repair",
			),
		);
		settlement = this.settleOutcome(requestId, outcome);
		if (settlement !== "settled")
			throw new ProgrammerPreloadValuesProtocolError(
				"Preload Programmer values outcome still conflicts after repair",
			);
	}

	private settleOutcome(
		requestId: string,
		outcome: ProgrammerPreloadValuesActionOutcome,
	) {
		return outcome.status === "changed"
			? this.options.store.settleChanged(
					requestId,
					outcome.projection,
					outcome.eventSequence,
					this.expectedStoreScope(),
				)
			: this.options.store.settleNoChange(
					requestId,
					outcome.preloadRevision,
					this.expectedStoreScope(),
				);
	}

	private assertResponse(
		request: ProgrammerPreloadValuesActionRequest,
		outcome: ProgrammerPreloadValuesActionOutcome,
	) {
		if (outcome.requestId !== request.requestId)
			throw this.protocolError("response request identity does not match");
		if (outcome.captureModeRevision !== request.expectedCaptureModeRevision)
			throw this.protocolError("response capture-mode revision does not match");
		if (
			outcome.status === "changed" &&
			outcome.projection.userId !== this.options.scope.userId
		)
			throw this.protocolError("response user does not match the active view");
		if (
			outcome.status === "changed" &&
			outcome.projection.revision !== outcome.preloadRevision
		)
			throw this.protocolError("response revisions do not match");
	}

	private protocolError(subject: string) {
		return new ProgrammerPreloadValuesProtocolError(
			`Preload Programmer values ${subject}`,
		);
	}

	private async repairAuthorities(error: Error) {
		await awaitPreloadAuthorityRepairs([
			this.options.repair(error),
			this.captureAuthority.repair(error),
		]);
	}

	private async repairError(error: Error) {
		try {
			await this.repairAuthorities(error);
			return error;
		} catch (reason) {
			return new Error(
				`Programmer authority repair failed: ${preloadValuesError(reason).message}`,
			);
		}
	}

	private claimScopes() {
		const state = this.options.store.getSnapshot();
		const captureState = this.options.captureModeStore.getSnapshot();
		if (
			state.showId !== this.options.scope.showId ||
			state.userId !== this.options.scope.userId ||
			captureState.showId !== this.options.scope.showId ||
			captureState.userId !== this.options.scope.userId
		)
			return false;
		this.storeScope ??= this.options.store.captureScope();
		return this.captureAuthority.claimScope();
	}

	private rollback(requestId: string, error: Error) {
		this.options.store.rollback(requestId, error, this.expectedStoreScope());
		this.options.onError?.(error);
		return null;
	}

	private refuse(message: string) {
		const error = new Error(message);
		this.options.onError?.(error);
		return Promise.resolve(null);
	}

	private abandon(requestId: string) {
		if (this.options.store.isScopeCurrent(this.expectedStoreScope()))
			this.options.store.commit(
				requestId,
				undefined,
				this.expectedStoreScope(),
			);
		return null;
	}

	private scopesAreCurrent() {
		return (
			!this.stopped &&
			this.claimScopes() &&
			this.options.store.isScopeCurrent(this.expectedStoreScope()) &&
			this.captureAuthority.isScopeCurrent()
		);
	}

	private valuesReadinessError() {
		return preloadValuesReadinessError(
			this.options.store,
			this.expectedStoreScope(),
		);
	}

	private expectedStoreScope() {
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
