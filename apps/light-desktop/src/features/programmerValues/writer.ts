import type { ProgrammerCaptureModeStore } from "../programmerCaptureMode/store";
import type {
	BatchProgrammerValuesInput,
	FinishProgrammerValuesGestureInput,
	ProgrammerValuesActionOutcome,
	ProgrammerValuesActionRequest,
	ProgrammerValuesActions,
	ProgrammerValuesCommand,
	ProgrammerValuesFinishGestureAction,
	ProgrammerValuesRequestAction,
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
	programmerValuesError,
	programmerValuesReadinessError,
	requiresValuesAuthorityRepair,
} from "./writerPolicy";

interface QueuedValuesWrite {
	requestId: string;
	action: ProgrammerValuesRequestAction;
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

/** One single-send FIFO for every normal Programmer values mutation surface. */
export class ProgrammerValuesWriter implements ProgrammerValuesActions {
	private readonly queue: QueuedValuesWrite[] = [];
	private readonly captureAuthority: ProgrammerValuesCaptureAuthority;
	private storeScope: number | null = null;
	/** The row whose request has been handed to `send`; cancellation never removes it. */
	private dispatched: QueuedValuesWrite | null = null;
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

	applyIntent(input: {
		requestId: string;
		fixtureIds: readonly string[];
		groupId?: string | null;
		attribute: string;
		operation: import("../../api/programmingComponentEditWire").ProgrammerValueIntentOperation<
			import("../../api/types/playback").AttributeValue
		>;
		undoGroup?: string | null;
		timing: import("./contracts").ProgrammerValueTiming;
		displayedSource?: import("./displayedSource").DisplayedSource | null;
		colorAdoption?: import("../../api/colorAdoptionWire").ColorAdoptionInput | null;
	}) {
		return this.enqueue(input.requestId, {
			action: "apply_intent",
			fixtureIds: input.fixtureIds,
			groupId: input.groupId,
			attribute: input.attribute,
			operation: input.operation,
			undoGroup: input.undoGroup,
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
	 * value and settles nothing into the store.
	 */
	finishGesture(input: FinishProgrammerValuesGestureInput) {
		if (!input.requestId || !input.attribute || !input.undoGroup)
			return this.refuse(
				"A Programmer gesture finish needs a request ID, attribute and Undo group",
			);
		if (!this.scopesAreCurrent()) return Promise.resolve(null);
		if (this.queue.some((write) => write.requestId === input.requestId))
			return this.refuse(
				`Programmer values request ${input.requestId} is already pending`,
			);
		// A completed discrete step keeps its admitted edits: they are the operator's request.
		if (!input.keepAdmittedEdits) this.cancelGesture(input.undoGroup);
		const action: ProgrammerValuesFinishGestureAction = {
			action: "finish_gesture",
			attribute: input.attribute,
			undoGroup: input.undoGroup,
		};
		if (this.queue.some((write) => sameFinish(write.action, action)))
			return Promise.resolve(null);
		return new Promise<ProgrammerValuesActionOutcome | null>((resolve) => {
			this.queue.push({
				requestId: input.requestId,
				action,
				// Unused: Finish sends the capture-mode revision current at dispatch.
				expectedCaptureModeRevision: -1,
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

	/**
	 * Finish-only dispatch: stale request/capture revisions are permitted, so
	 * it sends the current ones and accepts the current revisions returned
	 * with `no_change` (or the exact replayed original) without settling the
	 * store. Late or failed responses of a replaced scope stay quiet; genuine
	 * failures in the live scope are reported.
	 */
	private async sendFinish(
		requestId: string,
		action: ProgrammerValuesFinishGestureAction,
	) {
		if (!this.scopesAreCurrent()) return null;
		const expectedRevision = this.options.store.authoritativeRevision(
			this.expectedStoreScope(),
		);
		const captureMode = this.options.captureModeStore.getSnapshot().projection;
		if (expectedRevision == null || !captureMode) return null;
		try {
			const outcome = await this.options.applyAction(this.options.scope, {
				requestId,
				expectedRevision,
				expectedCaptureModeRevision: captureMode.revision,
				action,
			});
			if (!this.scopesAreCurrent()) return null;
			if (outcome.requestId !== requestId)
				throw new ProgrammerValuesProtocolError(
					"Programmer gesture finish response request identity does not match",
				);
			if (outcome.status !== "no_change")
				throw new ProgrammerValuesProtocolError(
					"Programmer gesture finish must not change values",
				);
			if (outcome.warning) this.options.onError?.(new Error(outcome.warning));
			return outcome;
		} catch (reason) {
			if (this.scopesAreCurrent())
				this.options.onError?.(programmerValuesError(reason));
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
			captureModeState.showId !== this.options.scope.showId
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
		return programmerValuesReadinessError(
			this.options.store,
			this.expectedStoreScope(),
		);
	}

	private expectedStoreScope() {
		return this.storeScope ?? -1;
	}
}

function sameFinish(
	queued: ProgrammerValuesRequestAction,
	finish: ProgrammerValuesFinishGestureAction,
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
