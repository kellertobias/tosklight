export type FrontendWarmupPriority = "foreground" | "near-future" | "idle";

export interface FrontendWarmupTaskResult {
	release?: () => void;
	retainedBytes?: number;
}

export interface FrontendWarmupTask {
	key: string;
	priority: FrontendWarmupPriority;
	run(signal: AbortSignal): Promise<FrontendWarmupTaskResult | undefined>;
}

export interface FrontendWarmupDiagnosticTask {
	key: string;
	priority: FrontendWarmupPriority;
	status: "queued" | "running" | "ready" | "cancelled" | "error";
	queuedAt: number;
	startedAt: number | null;
	finishedAt: number | null;
	error: string | null;
	retainedBytes: number;
}

export interface FrontendWarmupDiagnostics {
	status: "idle" | "running" | "ready" | "cancelled" | "error";
	active: number;
	peakActive: number;
	concurrency: number;
	taskBudget: number;
	retainedBytes: number;
	retainedByteBudget: number;
	tasks: readonly FrontendWarmupDiagnosticTask[];
}

export interface FrontendWarmupCoordinatorOptions {
	concurrency?: number;
	taskBudget?: number;
	retainedByteBudget?: number;
	now?: () => number;
	yieldToMain?: () => Promise<void>;
	onDiagnostics?: (diagnostics: FrontendWarmupDiagnostics) => void;
}

const PRIORITY_ORDER: Record<FrontendWarmupPriority, number> = {
	foreground: 0,
	"near-future": 1,
	idle: 2,
};

/**
 * One authority-epoch warm-up queue. It owns completed leases until cancellation,
 * so mounted view lifetime never becomes cache lifetime.
 */
export class FrontendWarmupCoordinator {
	private readonly concurrency: number;
	private readonly retainedByteBudget: number;
	private readonly taskBudget: number;
	private readonly now: () => number;
	private readonly yieldToMain: () => Promise<void>;
	private readonly onDiagnostics?: (
		diagnostics: FrontendWarmupDiagnostics,
	) => void;
	private readonly controller = new AbortController();
	private readonly tasks = new Map<
		string,
		{ task: FrontendWarmupTask; diagnostics: FrontendWarmupDiagnosticTask }
	>();
	private readonly releases: Array<() => void> = [];
	private active = 0;
	private peakActive = 0;
	private retainedBytes = 0;
	private started = false;
	private cancelled = false;
	private pumpQueued = false;

	constructor(options: FrontendWarmupCoordinatorOptions = {}) {
		this.concurrency = Math.max(1, options.concurrency ?? 2);
		this.taskBudget = Math.max(1, options.taskBudget ?? 32);
		this.retainedByteBudget = Math.max(
			0,
			options.retainedByteBudget ?? 64 * 1024 * 1024,
		);
		this.now = options.now ?? defaultNow;
		this.yieldToMain = options.yieldToMain ?? defaultYield;
		this.onDiagnostics = options.onDiagnostics;
	}

	enqueue(task: FrontendWarmupTask) {
		if (
			this.cancelled ||
			this.tasks.has(task.key) ||
			this.tasks.size >= this.taskBudget
		)
			return false;
		this.tasks.set(task.key, {
			task,
			diagnostics: {
				key: task.key,
				priority: task.priority,
				status: "queued",
				queuedAt: this.now(),
				startedAt: null,
				finishedAt: null,
				error: null,
				retainedBytes: 0,
			},
		});
		this.publish();
		if (this.started) this.queuePump();
		return true;
	}

	start() {
		if (this.cancelled || this.started) return;
		this.started = true;
		this.publish();
		this.queuePump();
	}

	cancel() {
		if (this.cancelled) return;
		this.cancelled = true;
		this.controller.abort();
		for (const entry of this.tasks.values()) {
			if (
				entry.diagnostics.status === "queued" ||
				entry.diagnostics.status === "running"
			) {
				entry.diagnostics.status = "cancelled";
				entry.diagnostics.finishedAt = this.now();
			}
		}
		for (const release of this.releases.splice(0).reverse()) release();
		this.publish();
	}

	getDiagnostics(): FrontendWarmupDiagnostics {
		const tasks = [...this.tasks.values()]
			.map(({ diagnostics }) => ({ ...diagnostics }))
			.sort(
				(left, right) =>
					PRIORITY_ORDER[left.priority] - PRIORITY_ORDER[right.priority] ||
					left.queuedAt - right.queuedAt ||
					left.key.localeCompare(right.key),
			);
		const pending = tasks.some(
			(task) => task.status === "queued" || task.status === "running",
		);
		const failed = tasks.some((task) => task.status === "error");
		return {
			status: this.cancelled
				? "cancelled"
				: !this.started
					? "idle"
					: pending
						? "running"
						: failed
							? "error"
							: "ready",
			active: this.active,
			peakActive: this.peakActive,
			concurrency: this.concurrency,
			taskBudget: this.taskBudget,
			retainedBytes: this.retainedBytes,
			retainedByteBudget: this.retainedByteBudget,
			tasks,
		};
	}

	private queuePump() {
		if (this.pumpQueued || this.cancelled) return;
		this.pumpQueued = true;
		queueMicrotask(() => {
			this.pumpQueued = false;
			void this.pump();
		});
	}

	private async pump() {
		if (this.cancelled) return;
		while (this.active < this.concurrency) {
			const entry = this.nextTask();
			if (!entry) break;
			this.active++;
			this.peakActive = Math.max(this.peakActive, this.active);
			entry.diagnostics.status = "running";
			entry.diagnostics.startedAt = this.now();
			this.publish();
			void this.run(entry).finally(() => {
				this.active = Math.max(0, this.active - 1);
				this.publish();
				this.queuePump();
			});
			await this.yieldToMain();
			if (this.cancelled) return;
		}
	}

	private nextTask() {
		return [...this.tasks.values()]
			.filter(({ diagnostics }) => diagnostics.status === "queued")
			.sort(
				(left, right) =>
					PRIORITY_ORDER[left.task.priority] -
						PRIORITY_ORDER[right.task.priority] ||
					left.diagnostics.queuedAt - right.diagnostics.queuedAt ||
					left.task.key.localeCompare(right.task.key),
			)[0];
	}

	private async run(entry: {
		task: FrontendWarmupTask;
		diagnostics: FrontendWarmupDiagnosticTask;
	}) {
		try {
			const result = await entry.task.run(this.controller.signal);
			if (this.cancelled) {
				result?.release?.();
				return;
			}
			const retainedBytes = Math.max(0, result?.retainedBytes ?? 0);
			if (this.retainedBytes + retainedBytes > this.retainedByteBudget) {
				result?.release?.();
				throw new Error(
					`Frontend warm-up retained-byte budget exceeded by ${entry.task.key}`,
				);
			}
			if (result?.release) this.releases.push(result.release);
			this.retainedBytes += retainedBytes;
			entry.diagnostics.retainedBytes = retainedBytes;
			entry.diagnostics.status = "ready";
			entry.diagnostics.finishedAt = this.now();
		} catch (reason) {
			if (this.cancelled || this.controller.signal.aborted) {
				entry.diagnostics.status = "cancelled";
			} else {
				entry.diagnostics.status = "error";
				entry.diagnostics.error =
					reason instanceof Error ? reason.message : String(reason);
			}
			entry.diagnostics.finishedAt = this.now();
		}
	}

	private publish() {
		this.onDiagnostics?.(this.getDiagnostics());
	}
}

function defaultNow() {
	return globalThis.performance?.now() ?? Date.now();
}

function defaultYield() {
	return new Promise<void>((resolve) => globalThis.setTimeout(resolve, 0));
}
