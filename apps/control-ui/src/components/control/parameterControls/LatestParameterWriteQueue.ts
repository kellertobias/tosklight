interface ParameterWriteTask {
	key: string | null;
	fingerprint: string | null;
	run(): Promise<unknown>;
	resolve(value: unknown | null): void;
	reject(reason: unknown): void;
	promise: Promise<unknown | null>;
}

/** Keeps continuous controls responsive without building an unbounded HTTP FIFO. */
export class LatestParameterWriteQueue {
	private readonly pending: ParameterWriteTask[] = [];
	private active: ParameterWriteTask | null = null;
	private stopped = false;

	submitLatest<T>(key: string, fingerprint: string, run: () => Promise<T>) {
		if (this.stopped) return Promise.resolve(null);
		if (
			this.active?.key === key &&
			this.active.fingerprint === fingerprint &&
			this.pending.length === 0
		)
			return Promise.resolve(null);
		const task = this.task(key, fingerprint, run);
		this.replacePendingContinuousWrite(task);
		this.start();
		return task.promise as Promise<T | null>;
	}

	submitBarrier<T>(run: () => Promise<T>) {
		if (this.stopped) return Promise.resolve(null);
		const task = this.task(null, null, run);
		this.pending.push(task);
		this.start();
		return task.promise as Promise<T | null>;
	}

	stop() {
		this.stopped = true;
		for (const task of this.pending) task.resolve(null);
		this.pending.length = 0;
	}

	private start() {
		if (this.active) return;
		void this.drain();
	}

	private replacePendingContinuousWrite(task: ParameterWriteTask) {
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
	): ParameterWriteTask {
		let resolve!: (value: unknown | null) => void;
		let reject!: (reason: unknown) => void;
		const promise = new Promise<unknown | null>((settle, fail) => {
			resolve = settle;
			reject = fail;
		});
		return { key, fingerprint, run, resolve, reject, promise };
	}
}
