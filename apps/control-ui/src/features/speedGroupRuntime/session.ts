import type {
	SpeedGroupEventMessage,
	SpeedGroupRuntimeScope,
	SpeedGroupSnapshot,
} from "./contracts";
import type { SpeedGroupRuntimeStore } from "./store";
import type {
	SpeedGroupEventStream,
	SpeedGroupRuntimeTransport,
} from "./transport";
import { SpeedGroupProtocolError } from "./transport";

export interface SpeedGroupSessionOptions {
	scope: SpeedGroupRuntimeScope;
	authorityKey: string;
	store: SpeedGroupRuntimeStore;
	transport: SpeedGroupRuntimeTransport;
	onError?: (error: Error | null) => void;
}

/** Reference-counted owner of the installation-global manual Speed Groups. */
export class SpeedGroupRuntimeSession {
	private references = 0;
	private lifecycle = 0;
	private hydrated = false;
	private stopped = false;
	private stream: SpeedGroupEventStream | null = null;
	private storeScope: number | null = null;
	private hydrationGeneration: number | null = null;
	private repairGeneration: number | null = null;
	private repairPromise: Promise<void> | null = null;
	private refreshQueued = false;
	private reconnectTimer: ReturnType<typeof globalThis.setTimeout> | null =
		null;

	constructor(private readonly options: SpeedGroupSessionOptions) {}

	activate() {
		if (!this.ensureStoreScope()) return () => {};
		const first = this.references === 0;
		this.references++;
		if (first) this.scheduleRefresh();
		let active = true;
		return () => {
			if (!active) return;
			active = false;
			this.references = Math.max(0, this.references - 1);
			if (this.references === 0) this.scheduleRefresh();
		};
	}

	stop() {
		this.stopped = true;
		this.references = 0;
		this.lifecycle++;
		this.hydrationGeneration = null;
		this.clearReconnect();
		this.closeStream();
		this.storeScope = null;
	}

	async repairAuthority(error: Error) {
		if (this.stopped || this.storeScope === null || this.references === 0)
			throw new Error("The Speed Group runtime view is unavailable");
		await this.repair(this.lifecycle, error);
		const state = this.options.store.getSnapshot();
		if (state.repairRequired)
			throw state.error ?? new Error("Speed Group runtime repair failed");
	}

	private scheduleRefresh(delay = 0) {
		if (this.stopped) return;
		if (delay === 0) this.clearReconnect();
		if (this.refreshQueued) return;
		this.refreshQueued = true;
		const refresh = () => {
			this.reconnectTimer = null;
			this.refreshQueued = false;
			if (!this.stopped) void this.refresh();
		};
		if (delay === 0) globalThis.queueMicrotask(refresh);
		else this.reconnectTimer = globalThis.setTimeout(refresh, delay);
	}

	private async refresh() {
		if (this.references === 0) {
			this.lifecycle++;
			this.hydrationGeneration = null;
			this.closeStream();
			return;
		}
		if (this.stream || this.hydrationGeneration !== null) return;
		const generation = ++this.lifecycle;
		const state = this.options.store.getSnapshot();
		if (this.hydrated && !state.repairRequired) {
			this.options.store.setReady(this.expectedStoreScope());
			this.options.onError?.(null);
			this.openStream(generation, state.eventSequence);
			return;
		}
		this.hydrationGeneration = generation;
		try {
			await this.hydrate(generation, state.repairRequired);
		} finally {
			if (this.hydrationGeneration === generation)
				this.hydrationGeneration = null;
		}
	}

	private async hydrate(generation: number, repair: boolean) {
		this.options.store.setLoading(this.expectedStoreScope());
		try {
			const snapshot = await this.loadSnapshot();
			if (!this.isCurrent(generation)) return;
			const installed = repair
				? this.options.store.installRepairSnapshot(
						snapshot,
						this.expectedStoreScope(),
					)
				: this.options.store.installSnapshot(
						snapshot,
						this.expectedStoreScope(),
					);
			if (!installed) throw this.scopeError("snapshot");
			this.hydrated = true;
			this.options.onError?.(null);
			this.openStream(generation, snapshot.cursor);
		} catch (reason) {
			if (this.isCurrent(generation)) this.fail(asError(reason), true);
		}
	}

	private openStream(generation: number, cursor: number | null) {
		if (!this.isCurrent(generation)) return;
		let stream: SpeedGroupEventStream;
		try {
			stream = this.options.transport.subscribe(this.options.scope, cursor, {
				message: (message) => {
					if (this.isCurrent(generation))
						this.handleMessage(message, generation);
				},
				error: (error) => {
					if (!this.isCurrent(generation)) return;
					if (error instanceof SpeedGroupProtocolError)
						void this.repair(generation, error);
					else this.fail(error, false);
				},
				closed: () => this.connectionClosed(generation),
			});
		} catch (reason) {
			if (this.isCurrent(generation)) this.fail(asError(reason), false);
			return;
		}
		if (!this.isCurrent(generation)) stream.close();
		else this.stream = stream;
	}

	private handleMessage(message: SpeedGroupEventMessage, generation: number) {
		if (message.type === "ready" || message.type === "repaired") {
			this.options.store.setReady(this.expectedStoreScope());
			this.options.onError?.(null);
			return;
		}
		if (message.type === "gap" || message.type === "error") {
			void this.repair(
				generation,
				new SpeedGroupProtocolError(
					message.type === "gap"
						? "Speed Group event history has a gap"
						: message.error,
					message.type === "gap" ? message.afterSequence : null,
				),
			);
			return;
		}
		try {
			if (
				!this.options.store.applyChange(
					message.change,
					message.sequence,
					this.expectedStoreScope(),
				)
			)
				throw this.scopeError("event");
		} catch (reason) {
			void this.repair(generation, asError(reason));
		}
	}

	private repair(generation: number, error: Error): Promise<void> {
		if (this.repairGeneration === generation)
			return this.repairPromise ?? Promise.resolve();
		if (!this.isCurrent(generation)) return Promise.resolve();
		this.repairGeneration = generation;
		const repair = this.performRepair(generation, error).finally(() => {
			if (this.repairGeneration === generation) this.repairGeneration = null;
			if (this.repairPromise === repair) this.repairPromise = null;
		});
		this.repairPromise = repair;
		return repair;
	}

	private async performRepair(generation: number, error: Error) {
		this.options.store.setRepairRequired(error, this.expectedStoreScope());
		this.options.onError?.(error);
		try {
			const snapshot = await this.loadSnapshot();
			if (!this.isCurrent(generation)) return;
			if (
				!this.options.store.installRepairSnapshot(
					snapshot,
					this.expectedStoreScope(),
				)
			)
				throw this.scopeError("repair snapshot");
			this.hydrated = true;
			this.stream?.repair(snapshot.cursor);
			this.options.onError?.(null);
		} catch (reason) {
			if (this.isCurrent(generation)) this.fail(asError(reason), true);
		}
	}

	private loadSnapshot(): Promise<SpeedGroupSnapshot> {
		return this.options.transport.loadSnapshot(this.options.scope);
	}

	private fail(error: Error, invalidateHydration: boolean) {
		if (invalidateHydration) this.hydrated = false;
		this.hydrationGeneration = null;
		this.options.store.setError(error, this.expectedStoreScope());
		this.options.onError?.(error);
		this.lifecycle++;
		this.closeStream();
		this.scheduleRefresh(750);
	}

	private connectionClosed(generation: number) {
		if (!this.isCurrent(generation)) return;
		this.lifecycle++;
		this.stream = null;
		this.scheduleRefresh(750);
	}

	private ensureStoreScope() {
		if (this.stopped) return false;
		const state = this.options.store.getSnapshot();
		if (state.deskId === null)
			this.options.store.reset(
				this.options.scope.deskId,
				this.options.authorityKey,
			);
		const scoped = this.options.store.getSnapshot();
		if (
			!sameId(scoped.deskId, this.options.scope.deskId) ||
			scoped.authorityKey !== this.options.authorityKey
		) {
			this.options.onError?.(this.scopeError("session"));
			return false;
		}
		this.storeScope = this.options.store.captureScope();
		return true;
	}

	private scopeError(subject: string) {
		return new SpeedGroupProtocolError(
			`Speed Group ${subject} does not match the active desk authority`,
		);
	}

	private isCurrent(generation: number) {
		return (
			!this.stopped &&
			this.references > 0 &&
			generation === this.lifecycle &&
			this.options.store.isScopeCurrent(this.expectedStoreScope())
		);
	}

	private expectedStoreScope() {
		return this.storeScope ?? -1;
	}

	private closeStream() {
		this.stream?.close();
		this.stream = null;
	}

	private clearReconnect() {
		if (this.reconnectTimer !== null)
			globalThis.clearTimeout(this.reconnectTimer);
		this.reconnectTimer = null;
		this.refreshQueued = false;
	}
}

function sameId(left: string | null, right: string) {
	return left?.toLowerCase() === right.toLowerCase();
}

function asError(reason: unknown) {
	return reason instanceof Error ? reason : new Error(String(reason));
}
