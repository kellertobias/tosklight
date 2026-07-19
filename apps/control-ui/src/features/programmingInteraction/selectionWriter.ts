import type {
	ProgrammingSnapshot,
	SelectionAction,
	SelectionActionOutcome,
	SelectionGestureSource,
	SelectionRule,
} from "./contracts";
import type { SelectionReducer } from "./selectionPrediction";
import {
	gestureSelectionPrediction,
	groupSelectionPrediction,
	replaceSelectionPrediction,
	ruleSelectionPrediction,
} from "./selectionPrediction";
import type { ProgrammingInteractionStore } from "./store";

type SelectionIntent =
	| { type: "replace"; fixtures: readonly string[] }
	| {
			type: "gesture";
			source: SelectionGestureSource;
			remove: boolean;
	  }
	| {
			type: "select_group";
			groupId: string;
			frozen: boolean;
			rule: SelectionRule;
	  }
	| { type: "apply_rule"; rule: SelectionRule };

interface QueuedSelectionWrite {
	requestId: string;
	intent: SelectionIntent;
	token: string;
	resolve: (outcome: SelectionActionOutcome | null) => void;
}

export interface ProgrammingSelectionReplacementIntent {
	resolvedFixtures: readonly string[];
}

export interface ProgrammingSelectionGestureIntent {
	source: SelectionGestureSource;
	resolvedFixtures: readonly string[];
	operation?: "add" | "remove";
}

export interface ProgrammingGroupSelectionIntent {
	groupId: string;
	resolvedFixtures: readonly string[];
	mode: "live" | "frozen";
	rule: SelectionRule;
	showRevision: number;
}

export interface ProgrammingSelectionWriterOptions {
	/** View-owned writers pass this explicitly; standalone writers may use an already-scoped store. */
	showId?: string;
	deskId: string;
	store: ProgrammingInteractionStore;
	apply(
		deskId: string,
		request: { requestId: string; action: SelectionAction },
	): Promise<SelectionActionOutcome>;
	loadSnapshot(): Promise<ProgrammingSnapshot>;
	onError?: (error: Error | null) => void;
}

/** Strict FIFO for semantic selection operations shared by every desk surface. */
export class ProgrammingSelectionWriter {
	private readonly queue: QueuedSelectionWrite[] = [];
	private readonly deferred: QueuedSelectionWrite[] = [];
	private readonly idleResolvers = new Set<(succeeded: boolean) => void>();
	private scope: number | null = null;
	private active: QueuedSelectionWrite | null = null;
	private running = false;
	private barrierRunning = false;
	private barrierPromise: Promise<unknown> | null = null;
	private drainSucceeded = true;
	private stopped = false;
	private readonly showId: string;

	constructor(private readonly options: ProgrammingSelectionWriterOptions) {
		const showId = options.showId ?? options.store.getSnapshot().showId;
		if (!showId) throw new Error("A selection writer requires an active show");
		this.showId = showId;
	}

	replace({ resolvedFixtures }: ProgrammingSelectionReplacementIntent) {
		return this.enqueue(
			{ type: "replace", fixtures: resolvedFixtures },
			() => replaceSelectionPrediction(resolvedFixtures),
		);
	}

	gesture({
		source,
		resolvedFixtures,
		operation = "add",
	}: ProgrammingSelectionGestureIntent) {
		const remove = operation === "remove";
		return this.enqueue(
			{ type: "gesture", source, remove },
			() => gestureSelectionPrediction(source, resolvedFixtures, remove),
		);
	}

	selectGroup({
		groupId,
		resolvedFixtures,
		mode,
		rule,
		showRevision,
	}: ProgrammingGroupSelectionIntent) {
		const frozen = mode === "frozen";
		return this.enqueue(
			{ type: "select_group", groupId, frozen, rule },
			() =>
				groupSelectionPrediction(
					groupId,
					resolvedFixtures,
					frozen,
					rule,
					showRevision,
				),
		);
	}

	applyRule(rule: SelectionRule) {
		return this.enqueue(
			{ type: "apply_rule", rule },
			() => ruleSelectionPrediction(rule),
		);
	}

	flush(): Promise<boolean> {
		if (this.stopped) return Promise.resolve(false);
		if (!this.running && this.queue.length === 0) return Promise.resolve(true);
		return new Promise((resolve) => this.idleResolvers.add(resolve));
	}

	runAfterPendingWrites<T>(run: () => Promise<T>, failed: T): Promise<T> {
		if (this.barrierPromise) return this.barrierPromise as Promise<T>;
		const barrier = this.runBarrier(run, failed);
		this.barrierPromise = barrier;
		const clear = () => {
			if (this.barrierPromise === barrier) this.barrierPromise = null;
		};
		void barrier.then(clear, clear);
		return barrier;
	}

	stop() {
		if (this.stopped) return;
		this.stopped = true;
		if (this.active) {
			this.options.store.commit(this.active.token, this.expectedScope());
			this.active.resolve(null);
			this.active = null;
		}
		for (const write of this.queue) {
			this.options.store.commit(write.token, this.expectedScope());
			write.resolve(null);
		}
		this.queue.length = 0;
		for (const write of this.deferred) {
			this.options.store.commit(write.token, this.expectedScope());
			write.resolve(null);
		}
		this.deferred.length = 0;
		this.resolveIdle(false);
	}

	private enqueue(intent: SelectionIntent, prediction: () => SelectionReducer) {
		if (this.stopped || !this.scopeIsCurrent())
			return Promise.resolve(null);
		let token: string | null;
		try {
			token = this.options.store.beginOptimisticSelectionUpdate(
				prediction(),
				this.expectedScope(),
			);
		} catch (reason) {
			this.options.onError?.(asError(reason));
			return Promise.resolve(null);
		}
		if (!token) return Promise.resolve(null);
		return new Promise<SelectionActionOutcome | null>((resolve) => {
			const queue = this.barrierRunning ? this.deferred : this.queue;
			queue.push({
				requestId: crypto.randomUUID(),
				intent,
				token,
				resolve,
			});
			if (!this.barrierRunning) this.start();
		});
	}

	private start() {
		if (this.running) return;
		this.running = true;
		this.drainSucceeded = true;
		void this.drain();
	}

	private async drain() {
		while (!this.stopped && this.queue.length > 0) {
			const write = this.queue.shift();
			if (!write) break;
			this.active = write;
			const outcome = await this.send(write);
			this.drainSucceeded &&= outcome !== null;
			write.resolve(outcome);
			this.active = null;
		}
		this.running = false;
		this.resolveIdle(this.drainSucceeded);
	}

	private async runBarrier<T>(run: () => Promise<T>, failed: T) {
		if (this.stopped || !this.scopeIsCurrent())
			return failed;
		this.barrierRunning = true;
		try {
			if (!(await this.flush()) || this.stopped) return failed;
			return await run();
		} finally {
			this.barrierRunning = false;
			if (!this.stopped && this.deferred.length > 0) {
				this.queue.push(...this.deferred.splice(0));
				this.start();
			}
		}
	}

	private resolveIdle(succeeded: boolean) {
		for (const resolve of this.idleResolvers) resolve(succeeded);
		this.idleResolvers.clear();
	}

	private async send(write: QueuedSelectionWrite) {
		if (this.stopped || !this.scopeIsCurrent())
			return null;
		try {
			const action = this.actionAtCurrentRevision(write.intent);
			const request = { requestId: write.requestId, action };
			const outcome = await this.requestWithOneNetworkRetry(request);
			if (this.stopped || !this.scopeIsCurrent())
				return null;
			if (
				!this.options.store.commitSelection(
					write.token,
					outcome.selection,
					this.expectedScope(),
				)
			)
				return null;
			this.options.onError?.(
				outcome.warning ? new Error(outcome.warning) : null,
			);
			return outcome;
		} catch (reason) {
			if (this.stopped || !this.scopeIsCurrent())
				return null;
			const error = asError(reason);
			const reported = needsAuthoritativeRepair(reason)
				? await this.repair(write.token, error)
				: this.rollback(write.token, error);
			if (this.stopped || !this.scopeIsCurrent())
				return null;
			this.options.onError?.(reported);
			return null;
		}
	}

	private actionAtCurrentRevision(intent: SelectionIntent): SelectionAction {
		const revision = this.options.store.authoritativeSelectionRevision(
			this.expectedScope(),
		);
		if (revision == null)
			throw new Error("The authoritative selection is unavailable");
		switch (intent.type) {
			case "replace":
				return { ...intent, expectedRevision: revision };
			case "select_group":
				return { ...intent, expectedRevision: revision };
			case "gesture":
			case "apply_rule":
				return intent;
		}
	}

	private async requestWithOneNetworkRetry(request: {
		requestId: string;
		action: SelectionAction;
	}) {
		try {
			return await this.options.apply(this.options.deskId, request);
		} catch (reason) {
			if (hasHttpStatus(reason)) throw reason;
			if (this.stopped || !this.scopeIsCurrent())
				throw reason;
			return this.options.apply(this.options.deskId, request);
		}
	}

	private async repair(token: string, original: Error): Promise<Error> {
		try {
			const snapshot = await this.options.loadSnapshot();
			if (this.stopped || !this.scopeIsCurrent())
				return original;
			if (
				!this.options.store.installSelectionRepair(
					token,
					this.expectedScope(),
					snapshot,
				)
			)
				throw new Error("The repaired selection no longer belongs to this view");
			return original;
		} catch (reason) {
			this.options.store.rollback(token, original, this.expectedScope());
			return new Error(`Selection repair failed: ${asError(reason).message}`);
		}
	}

	private rollback(token: string, error: Error) {
		this.options.store.rollback(token, error, this.expectedScope());
		return error;
	}

	private scopeIsCurrent() {
		return (
			this.claimScope() &&
			this.options.store.isScopeCurrent(this.expectedScope())
		);
	}

	private claimScope() {
		const state = this.options.store.getSnapshot();
		if (
			state.showId !== this.showId ||
			state.deskId !== this.options.deskId
		)
			return false;
		this.scope ??= this.options.store.captureScope();
		return true;
	}

	private expectedScope() {
		return this.scope ?? -1;
	}
}

function httpStatus(reason: unknown) {
	if (typeof reason !== "object" || reason === null || !("status" in reason))
		return null;
	const status = (reason as { status?: unknown }).status;
	return typeof status === "number" ? status : null;
}

function hasHttpStatus(reason: unknown) {
	return httpStatus(reason) !== null;
}

function needsAuthoritativeRepair(reason: unknown) {
	const status = httpStatus(reason);
	return status === null || status === 408 || status === 409 || status >= 500;
}

function asError(reason: unknown) {
	return reason instanceof Error ? reason : new Error(String(reason));
}
