import type {
	CommandLinePatch,
	CommandLineProjection,
	ProgrammingSnapshot,
} from "./contracts";
import type { ProgrammingInteractionStore } from "./store";

interface QueuedCommandLineWrite {
	token: string;
	text: string;
	resolve: (succeeded: boolean) => void;
}

export type CommandLineExecutionResult =
	| "executed"
	| "execution_unknown"
	| "write_failed"
	| "execution_failed";

export interface ProgrammingCommandLineWriterOptions {
	/** View-owned writers pass this explicitly; standalone writers may use an already-scoped store. */
	showId?: string;
	deskId: string;
	store: ProgrammingInteractionStore;
	replace(
		deskId: string,
		text: string,
		expectedRevision: number,
	): Promise<CommandLineProjection>;
	loadSnapshot(): Promise<ProgrammingSnapshot>;
	onError?: (error: Error | null) => void;
}

/**
 * Serializes revisioned command-line writes while keeping editing immediate.
 *
 * One request may be in flight. Further keystrokes collapse to the latest unsent
 * value, so slow transports cannot build an obsolete request backlog.
 */
export class ProgrammingCommandLineWriter {
	private readonly showId: string;
	private readonly deskId: string;
	private readonly store: ProgrammingInteractionStore;
	private readonly replaceRequest: ProgrammingCommandLineWriterOptions["replace"];
	private readonly loadSnapshot: ProgrammingCommandLineWriterOptions["loadSnapshot"];
	private readonly onError?: ProgrammingCommandLineWriterOptions["onError"];
	private scope: number | null = null;
	private readonly idleResolvers = new Set<(succeeded: boolean) => void>();
	private readonly drainResolvers = new Set<(succeeded: boolean) => void>();
	private queued: QueuedCommandLineWrite | null = null;
	private deferred: QueuedCommandLineWrite | null = null;
	private active: QueuedCommandLineWrite | null = null;
	private executionResetToken: string | null = null;
	private running = false;
	private executionRunning = false;
	private executionPromise: Promise<CommandLineExecutionResult> | null = null;
	private drainSucceeded = true;
	private stopped = false;

	constructor(options: ProgrammingCommandLineWriterOptions) {
		const showId = options.showId ?? options.store.getSnapshot().showId;
		if (!showId) throw new Error("A command-line writer requires an active show");
		this.showId = showId;
		this.deskId = options.deskId;
		this.store = options.store;
		this.replaceRequest = options.replace;
		this.loadSnapshot = options.loadSnapshot;
		this.onError = options.onError;
	}

	replace(text: string): Promise<boolean> {
		if (this.stopped || !this.scopeIsCurrent()) return Promise.resolve(false);
		const current = this.store.getSnapshot().commandLine;
		if (!current) return Promise.resolve(false);
		const normalized = normalizeCommandLine(text, current);
		const token = this.beginOptimisticCommandLine({
			text: normalized.text,
			pristine: normalized.pristine,
			pendingChoice: null,
		});
		if (!token) return Promise.resolve(true);
		return new Promise<boolean>((resolve) => {
			if (this.executionRunning) {
				this.supersedeDeferredWrite();
				this.deferred = { token, text, resolve };
				return;
			}
			this.supersedeQueuedWrite();
			this.queued = { token, text, resolve };
			this.start();
		});
	}

	flush(): Promise<boolean> {
		if (this.stopped) return Promise.resolve(false);
		if (!this.running && !this.queued && !this.executionRunning)
			return Promise.resolve(true);
		return new Promise((resolve) => this.idleResolvers.add(resolve));
	}

	executeAfterPendingWrites(
		execute: () => Promise<boolean>,
		optimisticReset: CommandLinePatch,
	): Promise<CommandLineExecutionResult> {
		if (this.executionPromise) return this.executionPromise;
		if (this.stopped || !this.scopeIsCurrent())
			return Promise.resolve("write_failed");
		const execution = this.runExecution(execute, optimisticReset);
		this.executionPromise = execution;
		const clear = () => {
			if (this.executionPromise === execution) this.executionPromise = null;
		};
		void execution.then(clear, clear);
		return execution;
	}

	private async runExecution(
		execute: () => Promise<boolean>,
		optimisticReset: CommandLinePatch,
	): Promise<CommandLineExecutionResult> {
		this.executionRunning = true;
		this.executionResetToken = this.beginOptimisticCommandLine(optimisticReset);
		const barrier = this.queued;
		this.queued = null;
		const drained = await this.waitForDrain();
		const flushed = this.stopped
			? this.discardBarrier(barrier)
			: barrier
				? await this.sendBarrier(barrier)
				: drained;
		if (!flushed || this.stopped) {
			if (!this.stopped) await this.reconcileAfterExecution();
			this.discardDeferredWrite(
				new Error("The command line could not be synchronized"),
			);
			return this.finishExecution("write_failed");
		}
		let executed = false;
		let executionError: Error | null = null;
		try {
			executed = await execute();
		} catch (reason) {
			executionError = asError(reason);
			this.onError?.(executionError);
		}
		const reconciled = await this.reconcileAfterExecution(!executionError);
		if (!reconciled) {
			this.discardDeferredWrite();
			return this.finishExecution("execution_unknown");
		}
		return this.finishExecution(executed ? "executed" : "execution_failed");
	}

	stop() {
		if (this.stopped) return;
		this.stopped = true;
		if (this.active) {
			this.commit(this.active.token);
			this.active.resolve(false);
			this.active = null;
		}
		if (this.executionResetToken)
			this.commit(this.executionResetToken);
		this.executionResetToken = null;
		if (this.queued) {
			this.commit(this.queued.token);
			this.queued.resolve(false);
			this.queued = null;
		}
		if (this.deferred) {
			this.commit(this.deferred.token);
			this.deferred.resolve(false);
			this.deferred = null;
		}
		this.resolveDrain(false);
		this.resolveIdle(false, true);
	}

	private supersedeQueuedWrite() {
		if (!this.queued) return;
		this.commit(this.queued.token);
		this.queued.resolve(true);
		this.queued = null;
	}

	private supersedeDeferredWrite() {
		if (!this.deferred) return;
		this.commit(this.deferred.token);
		this.deferred.resolve(true);
		this.deferred = null;
	}

	private start() {
		if (this.running) return;
		this.running = true;
		this.drainSucceeded = true;
		void this.drain();
	}

	private async drain() {
		while (!this.stopped && this.queued) {
			const write = this.queued;
			this.queued = null;
			this.active = write;
			const succeeded = await this.send(write);
			this.drainSucceeded = succeeded;
			write.resolve(succeeded);
			if (this.active === write) this.active = null;
		}
		this.running = false;
		this.resolveDrain();
		this.resolveIdle();
	}

	private waitForDrain(): Promise<boolean> {
		if (!this.running) return Promise.resolve(true);
		return new Promise((resolve) => this.drainResolvers.add(resolve));
	}

	private async sendBarrier(write: QueuedCommandLineWrite | null) {
		if (!write) return true;
		this.active = write;
		const succeeded = await this.send(write);
		write.resolve(succeeded);
		if (this.active === write) this.active = null;
		return succeeded;
	}

	private discardBarrier(write: QueuedCommandLineWrite | null) {
		if (write) {
			this.commit(write.token);
			write.resolve(false);
		}
		return false;
	}

	private async send(write: QueuedCommandLineWrite) {
		if (this.stopped || !this.scopeIsCurrent()) return false;
		try {
			const response = await this.replaceAtCurrentRevision(write.text);
			if (this.stopped || !this.scopeIsCurrent()) return false;
			if (
				!this.store.commitCommandLine(
					write.token,
					response,
					this.expectedScope(),
				)
			)
				return false;
			this.onError?.(null);
			return true;
		} catch (reason) {
			if (this.stopped || !this.scopeIsCurrent()) return false;
			const error = asError(reason);
			this.rollback(write.token, error);
			this.onError?.(error);
			return false;
		}
	}

	private async replaceAtCurrentRevision(text: string) {
		const revision = this.store.authoritativeCommandLineRevision(
			this.expectedScope(),
		);
		if (revision == null)
			throw new Error("The authoritative command line is unavailable");
		try {
			return await this.replaceRequest(this.deskId, text, revision);
		} catch (reason) {
			if (!isRevisionConflict(reason)) throw reason;
			if (this.stopped || !this.scopeIsCurrent()) throw reason;
			const snapshot = await this.loadSnapshot();
			if (this.stopped || !this.scopeIsCurrent()) throw reason;
			if (
				!this.store.installSnapshot(snapshot, {
					updateSessionState: false,
					expectedScope: this.expectedScope(),
				})
			)
				throw new Error("The repaired command line belongs to another desk");
			// A whole-line replacement cannot be safely rebased over a concurrent OSC
			// or desk edit. The repaired authority wins; a later explicit local edit may
			// submit against its revision.
			throw reason;
		}
	}

	private async reconcileAfterExecution(clearError = true) {
		try {
			const snapshot = await this.loadSnapshot();
			if (this.stopped || !this.scopeIsCurrent()) return false;
			if (
				!this.store.installSnapshot(snapshot, {
					updateSessionState: false,
					expectedScope: this.expectedScope(),
				})
			)
				throw new Error("The executed command belongs to another desk");
			this.commit(this.executionResetToken);
			this.executionResetToken = null;
			if (clearError) this.onError?.(null);
			return true;
		} catch (reason) {
			if (this.stopped || !this.scopeIsCurrent()) return false;
			const error = asError(reason);
			this.rollback(this.executionResetToken, error);
			this.executionResetToken = null;
			this.onError?.(error);
			return false;
		}
	}

	private discardDeferredWrite(error = new Error("Command synchronization failed")) {
		if (!this.deferred) return;
		this.rollback(this.deferred.token, error);
		this.deferred.resolve(false);
		this.deferred = null;
	}

	private finishExecution(
		result: CommandLineExecutionResult,
	): CommandLineExecutionResult {
		this.executionRunning = false;
		this.executionResetToken = null;
		if (!this.stopped && this.deferred) {
			this.queued = this.deferred;
			this.deferred = null;
			this.start();
		}
		this.resolveIdle();
		return result;
	}

	private beginOptimisticCommandLine(patch: CommandLinePatch) {
		return this.store.beginOptimisticCommandLine(patch, this.expectedScope());
	}

	private commit(token: string | null) {
		return this.store.commit(token, this.expectedScope());
	}

	private rollback(token: string | null, error: Error) {
		return this.store.rollback(token, error, this.expectedScope());
	}

	private scopeIsCurrent() {
		return this.claimScope() && this.store.isScopeCurrent(this.expectedScope());
	}

	private claimScope() {
		const state = this.store.getSnapshot();
		if (state.showId !== this.showId || state.deskId !== this.deskId)
			return false;
		this.scope ??= this.store.captureScope();
		return true;
	}

	private expectedScope() {
		return this.scope ?? -1;
	}

	private resolveDrain(succeeded = this.drainSucceeded) {
		for (const resolve of this.drainResolvers) resolve(succeeded);
		this.drainResolvers.clear();
	}

	private resolveIdle(succeeded = this.drainSucceeded, force = false) {
		if (!force && (this.running || this.queued || this.executionRunning)) return;
		for (const resolve of this.idleResolvers) resolve(succeeded);
		this.idleResolvers.clear();
	}
}

function normalizeCommandLine(
	text: string,
	current: CommandLineProjection,
) {
	const trimmed = text.trim();
	const pristine =
		!trimmed || trimmed.toUpperCase() === current.target;
	return {
		text: pristine ? current.target : text,
		pristine,
	};
}

function isRevisionConflict(reason: unknown) {
	return (
		typeof reason === "object" &&
		reason !== null &&
		"status" in reason &&
		(reason as { status?: unknown }).status === 409
	);
}

function asError(reason: unknown) {
	return reason instanceof Error ? reason : new Error(String(reason));
}
