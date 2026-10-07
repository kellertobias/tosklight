/**
 * Optional local cancellation identity for one control gesture. Callers reuse
 * the gesture's existing `ApplyIntent.undoGroup`; a fresh touch uses a new ID.
 */
export interface ProgrammerValuesWriteOptions {
	gesture?: string | null;
}

interface ProgrammerValuesWriteTask {
	key: string | null;
	fingerprint: string | null;
	gesture: string | null;
	run(): Promise<unknown>;
	resolve(value: unknown | null): void;
	reject(reason: unknown): void;
	promise: Promise<unknown | null>;
}

/** Keeps continuous Programmer controls responsive without an unbounded HTTP FIFO. */
export class LatestProgrammerValuesWriteQueue {
	private readonly pending: ProgrammerValuesWriteTask[] = [];
	private active: ProgrammerValuesWriteTask | null = null;
	private stopped = false;

	submitLatest<T>(
		key: string,
		fingerprint: string,
		run: () => Promise<T>,
		options?: ProgrammerValuesWriteOptions,
	) {
		if (this.stopped) return Promise.resolve(null);
		if (
			this.active?.key === key &&
			this.active.fingerprint === fingerprint &&
			this.pending.length === 0
		)
			return Promise.resolve(null);
		const task = this.task(key, fingerprint, run, options);
		this.replacePendingContinuousWrite(task);
		this.start();
		return task.promise as Promise<T | null>;
	}

	submitBarrier<T>(
		run: () => Promise<T>,
		options?: ProgrammerValuesWriteOptions,
	) {
		if (this.stopped) return Promise.resolve(null);
		const task = this.task(null, null, run, options);
		this.pending.push(task);
		this.start();
		return task.promise as Promise<T | null>;
	}

	stop() {
		this.stopped = true;
		for (const task of this.pending) task.resolve(null);
		this.pending.length = 0;
	}

	/**
	 * Drops only the not-yet-started tasks of one stopped gesture. The active
	 * task is never touched: it settles once with its own result. Other
	 * gestures and untagged barriers keep their FIFO order and the queue stays
	 * open. Removed tasks resolve quietly with `null`. Returns the drop count.
	 */
	cancelGesture(gesture: string) {
		if (!gesture) return 0;
		let removed = 0;
		for (let index = this.pending.length - 1; index >= 0; index--) {
			const pending = this.pending[index];
			if (!pending || pending.gesture !== gesture) continue;
			this.pending.splice(index, 1);
			pending.resolve(null);
			removed++;
		}
		return removed;
	}

	private start() {
		if (this.active) return;
		void this.drain();
	}

	private replacePendingContinuousWrite(task: ProgrammerValuesWriteTask) {
		for (let index = this.pending.length - 1; index >= 0; index--) {
			const pending = this.pending[index];
			if (!pending || pending.key === null) break;
			if (pending.key !== task.key) continue;
			pending.resolve(null);
			this.pending.splice(index, 1);
			break;
		}
		this.pending.push(task);
	}

	private async drain() {
		while (!this.stopped && this.pending.length) {
			const task = this.pending.shift();
			if (!task) break;
			this.active = task;
			try {
				task.resolve(await task.run());
			} catch (reason) {
				task.reject(reason);
			}
			this.active = null;
		}
		this.active = null;
	}

	private task<T>(
		key: string | null,
		fingerprint: string | null,
		run: () => Promise<T>,
		options?: ProgrammerValuesWriteOptions,
	): ProgrammerValuesWriteTask {
		let resolve!: (value: unknown | null) => void;
		let reject!: (reason: unknown) => void;
		const promise = new Promise<unknown | null>((settle, fail) => {
			resolve = settle;
			reject = fail;
		});
		const gesture = options?.gesture || null;
		return { key, fingerprint, gesture, run, resolve, reject, promise };
	}
}
