import type { ProgrammerCaptureModeStore } from "../programmerCaptureMode/store";
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
import {
	awaitProgrammerAuthorityRepairs,
	ProgrammerValuesCaptureAuthority,
} from "./writerCaptureAuthority";
import {
	isReplayableValuesError,
	programmerValuesError,
	programmerValuesReadinessError,
	requiresValuesAuthorityRepair,
} from "./writerPolicy";

interface QueuedValuesWrite {
	requestId: string;
	action: ProgrammerValuesCommand;
	expectedCaptureModeRevision: number;
	resolve(outcome: ProgrammerValuesActionOutcome | null): void;
}

export interface ProgrammerValuesWriterOptions {
	scope: ProgrammerValuesScope;
	store: ProgrammerValuesStore;
	captureModeStore: ProgrammerCaptureModeStore;
	applyAction(
		scope: ProgrammerValuesScope,
		request: ProgrammerValuesActionRequest,
	): Promise<ProgrammerValuesActionOutcome>;
	repair(error: Error): Promise<void>;
	repairCaptureMode(error: Error): Promise<void>;
	onError?: (error: Error | null) => void;
}

/** One replay-safe FIFO for every normal Programmer values mutation surface. */
export class ProgrammerValuesWriter implements ProgrammerValuesActions {
	private readonly queue: QueuedValuesWrite[] = [];
	private readonly captureAuthority: ProgrammerValuesCaptureAuthority;
	private storeScope: number | null = null;
	private running = false;
	private stopped = false;
	constructor(private readonly options: ProgrammerValuesWriterOptions) {
		this.captureAuthority = new ProgrammerValuesCaptureAuthority({
			scope: options.scope,
			store: options.captureModeStore,
			repair: options.repairCaptureMode,
		});
	}

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
		for (const write of this.queue) {
			this.abandon(write.requestId);
			write.resolve(null);
		}
		this.queue.length = 0;
	}

	private enqueue(requestId: string, action: ProgrammerValuesCommand) {
		if (this.stopped || !this.claimScopes()) return Promise.resolve(null);
		if (!requestId) {
			this.options.onError?.(
				new Error("A Programmer values request ID is required"),
			);
			return Promise.resolve(null);
		}
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
					predictProgrammerValues(action),
					this.expectedStoreScope(),
				)
			)
				return Promise.resolve(null);
		} catch (reason) {
			this.options.onError?.(programmerValuesError(reason));
			return Promise.resolve(null);
		}
		return new Promise<ProgrammerValuesActionOutcome | null>((resolve) => {
			this.queue.push({
				requestId,
				action,
				expectedCaptureModeRevision: captureMode.revision,
				resolve,
			});
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

	private async send(write: QueuedValuesWrite) {
		if (!this.scopesAreCurrent()) return this.abandon(write.requestId);
		const precondition =
			this.valuesReadinessError() ??
			this.captureAuthority.preconditionError(
				write.expectedCaptureModeRevision,
			);
		if (precondition) {
			this.options.store.rollback(
				write.requestId,
				precondition,
				this.expectedStoreScope(),
			);
			this.options.onError?.(precondition);
			return null;
		}
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
			const error = programmerValuesError(reason);
			const reported = requiresValuesAuthorityRepair(reason)
				? await this.repairError(error)
				: error;
			if (!this.scopesAreCurrent()) return this.abandon(write.requestId);
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
		write: QueuedValuesWrite,
	): ProgrammerValuesActionRequest {
		const expectedRevision = this.options.store.authoritativeRevision(
			this.expectedStoreScope(),
		);
		if (expectedRevision == null)
			throw new Error("Authoritative Programmer values are unavailable");
		return {
			requestId: write.requestId,
			expectedRevision,
			expectedCaptureModeRevision: write.expectedCaptureModeRevision,
			action: write.action,
		};
	}
	private async requestWithOneReplay(request: ProgrammerValuesActionRequest) {
		try {
			return await this.options.applyAction(this.options.scope, request);
		} catch (reason) {
			if (!isReplayableValuesError(reason) || !this.scopesAreCurrent())
				throw reason;
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
						this.expectedStoreScope(),
					)
				: this.options.store.settleNoChange(
						requestId,
						outcome.revision,
						this.expectedStoreScope(),
					);
		if (settlement !== "repair") return;
		await this.repairAuthorities(
			new ProgrammerValuesProtocolError(
				"Programmer values outcome requires repair",
			),
		);
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
					this.expectedStoreScope(),
				)
			: this.options.store.settleNoChange(
					requestId,
					outcome.revision,
					this.expectedStoreScope(),
				);
	}

	private assertResponse(
		request: ProgrammerValuesActionRequest,
		outcome: ProgrammerValuesActionOutcome,
	) {
		if (outcome.requestId !== request.requestId)
			throw new ProgrammerValuesProtocolError(
				"Programmer values response request identity does not match",
			);
		if (outcome.captureModeRevision !== request.expectedCaptureModeRevision)
			throw new ProgrammerValuesProtocolError(
				"Programmer values response capture-mode revision does not match",
			);
		if (
			outcome.status === "changed" &&
			outcome.projection.revision !== outcome.revision
		)
			throw new ProgrammerValuesProtocolError(
				"Programmer values response revisions do not match",
			);
	}

	private async repairAuthorities(error: Error) {
		await awaitProgrammerAuthorityRepairs([
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
				`Programmer authority repair failed: ${programmerValuesError(reason).message}`,
			);
		}
	}

	private claimScopes() {
		const state = this.options.store.getSnapshot();
		const captureModeState = this.options.captureModeStore.getSnapshot();
		if (
			state.showId !== this.options.scope.showId ||
			state.userId !== this.options.scope.userId ||
			captureModeState.showId !== this.options.scope.showId ||
			captureModeState.userId !== this.options.scope.userId
		)
			return false;
		this.storeScope ??= this.options.store.captureScope();
		return this.captureAuthority.claimScope();
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
		return programmerValuesReadinessError(
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
