import type { ProgrammerCaptureModeStore } from "../programmerCaptureMode/store";
import type {
	ApplyProgrammerPreloadValueIntentInput,
	BatchProgrammerPreloadValuesInput,
	FinishProgrammerPreloadValuesGestureInput,
	ProgrammerPreloadValuesActionOutcome,
	ProgrammerPreloadValuesActionRequest,
	ProgrammerPreloadValuesActions,
	ProgrammerPreloadValuesCommand,
	ProgrammerPreloadValuesFinishGestureAction,
	ProgrammerPreloadValuesRequestAction,
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
	preloadValuesError,
	preloadValuesReadinessError,
} from "./writerPolicy";

interface QueuedPreloadWrite {
	requestId: string;
	action: ProgrammerPreloadValuesRequestAction;
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
	/** The row whose request has been handed to `send`; cancellation never removes it. */
	private dispatched: QueuedPreloadWrite | null = null;
	private running = false;
	private stopped = false;

	constructor(private readonly options: ProgrammerPreloadValuesWriterOptions) {
		this.captureAuthority = new ProgrammerPreloadCaptureAuthority({
			scope: options.scope,
			store: options.captureModeStore,
			repair: options.repairCaptureMode,
		});
	}

	applyIntent(input: ApplyProgrammerPreloadValueIntentInput) {
		return this.enqueue(input.requestId, {
			action: "apply_intent",
			fixtureIds: input.fixtureIds,
			...(input.groupId ? { groupId: input.groupId } : {}),
			attribute: input.attribute,
			operation: input.operation,
			...(input.undoGroup ? { undoGroup: input.undoGroup } : {}),
			timing: input.timing,
			...(input.displayedSource
				? { displayedSource: input.displayedSource }
				: {}),
			...(input.colorAdoption ? { colorAdoption: input.colorAdoption } : {}),
		});
	}

	applyIndexedPreset(input: {
		requestId: string;
		expectedSelectionRevision: number;
		attribute: string;
		targets: ReadonlyArray<{
			fixtureId: string;
			functionId: string;
			expectedProfileRevision: number;
		}>;
	}) {
		return this.enqueue(input.requestId, {
			action: "apply_indexed_preset",
			expectedSelectionRevision: input.expectedSelectionRevision,
			attribute: input.attribute,
			targets: input.targets,
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

	/**
	 * Locally drops the unsent `apply_intent` rows of one stopped gesture,
	 * identified by its existing `undoGroup`. The dispatched row keeps its
	 * request ID, expected revisions and optimistic correlation and settles
	 * once with its own outcome. Other rows keep FIFO order and the writer
	 * stays open. Dropped rows remove only their speculative store entry and
	 * resolve quietly with `null`; no backend action or Undo change is made.
	 */
	cancelGesture(undoGroup: string) {
		if (!undoGroup) return 0;
		let removed = 0;
		for (let index = this.queue.length - 1; index >= 0; index--) {
			const write = this.queue[index];
			if (
				!write ||
				write === this.dispatched ||
				write.action.action !== "apply_intent" ||
				write.action.undoGroup !== undoGroup
			)
				continue;
			this.queue.splice(index, 1);
			this.discard(write.requestId);
			write.resolve(null);
			removed++;
		}
		return removed;
	}

	/**
	 * Ends one stopped gesture in this writer's captured desk/session/lane
	 * scope. It first drops the gesture's unsent `apply_intent` rows (kept for
	 * a completed discrete step, `keepAdmittedEdits`), then queues exactly one
	 * `finish_gesture` behind any in-flight or kept edit on this same FIFO;
	 * later fresh touches queue behind it. A gone or replaced
	 * scope abandons the cleanup quietly. The Finish authors no optimistic
	 * value and settles nothing into the store, even after disarm.
	 */
	finishGesture(input: FinishProgrammerPreloadValuesGestureInput) {
		if (!input.requestId || !input.attribute || !input.undoGroup)
			return this.refuse(
				"A Preload gesture finish needs a request ID, attribute and Undo group",
			);
		if (!this.scopesAreCurrent()) return Promise.resolve(null);
		if (this.queue.some((write) => write.requestId === input.requestId))
			return this.refuse(
				`Preload Programmer values request ${input.requestId} is already pending`,
			);
		// A completed discrete step keeps its admitted edits: they are the operator's request.
		if (!input.keepAdmittedEdits) this.cancelGesture(input.undoGroup);
		const action: ProgrammerPreloadValuesFinishGestureAction = {
			action: "finish_gesture",
			attribute: input.attribute,
			undoGroup: input.undoGroup,
		};
		if (this.queue.some((write) => sameFinish(write.action, action)))
			return Promise.resolve(null);
		return new Promise<ProgrammerPreloadValuesActionOutcome | null>(
			(resolve) => {
				this.queue.push({
					requestId: input.requestId,
					action,
					// Unused: Finish sends the capture-mode revision current at dispatch.
					expectedCaptureModeRevision: -1,
					resolve,
				});
				this.start();
			},
		);
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
			this.dispatched = write;
			const send =
				write.action.action === "finish_gesture"
					? this.sendFinish(write.requestId, write.action)
					: this.send(write);
			const outcome = await send.finally(() => {
				this.dispatched = null;
			});
			const index = this.queue.indexOf(write);
			if (index >= 0) this.queue.splice(index, 1);
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
			const outcome = await this.options.applyAction(
				this.options.scope,
				request,
			);
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
			const reported = await this.repairError(error);
			if (!this.scopesAreCurrent()) return this.abandon(write.requestId);
			return this.rollback(write.requestId, reported);
		}
	}

	/**
	 * Finish-only dispatch: stale request/capture revisions are permitted, so
	 * it sends the current ones and accepts the current revisions returned
	 * with `no_change` (or the exact replayed original) without settling the
	 * store. Late or failed responses of a replaced scope stay quiet; genuine
	 * failures in the live scope are reported.
	 */
	private async sendFinish(
		requestId: string,
		action: ProgrammerPreloadValuesFinishGestureAction,
	) {
		if (!this.scopesAreCurrent()) return null;
		const expectedPreloadRevision = this.options.store.authoritativeRevision(
			this.expectedStoreScope(),
		);
		const captureMode = this.options.captureModeStore.getSnapshot().projection;
		if (expectedPreloadRevision == null || !captureMode) return null;
		try {
			const outcome = await this.options.applyAction(this.options.scope, {
				requestId,
				expectedPreloadRevision,
				expectedCaptureModeRevision: captureMode.revision,
				action,
			});
			if (!this.scopesAreCurrent()) return null;
			if (outcome.requestId !== requestId)
				throw this.protocolError(
					"gesture finish response request identity does not match",
				);
			if (outcome.status !== "no_change")
				throw this.protocolError("gesture finish must not change values");
			if (outcome.warning) this.options.onError?.(new Error(outcome.warning));
			return outcome;
		} catch (reason) {
			if (this.scopesAreCurrent())
				this.options.onError?.(preloadValuesError(reason));
			return null;
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
			captureState.showId !== this.options.scope.showId
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

	/** Removes one unsent speculative entry without clearing an unrelated store error. */
	private discard(requestId: string) {
		const scope = this.expectedStoreScope();
		if (!this.options.store.isScopeCurrent(scope)) return;
		const error = this.options.store.getSnapshot().error;
		if (error) this.options.store.rollback(requestId, error, scope);
		else this.options.store.commit(requestId, undefined, scope);
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

function sameFinish(
	queued: ProgrammerPreloadValuesRequestAction,
	finish: ProgrammerPreloadValuesFinishGestureAction,
) {
	return (
		queued.action === "finish_gesture" &&
		queued.undoGroup === finish.undoGroup &&
		queued.attribute === finish.attribute
	);
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
