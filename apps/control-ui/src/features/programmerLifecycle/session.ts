import type {
	ProgrammerLifecycleEventMessage,
	ProgrammerLifecycleSnapshot,
} from "./contracts";
import type { ProgrammerLifecycleStore } from "./store";
import {
	type ProgrammerLifecycleEventStream,
	type ProgrammerLifecycleEventTransport,
	ProgrammerLifecycleProtocolError,
} from "./transport";

export interface ProgrammerLifecycleSessionOptions {
	authorityKey: string;
	store: ProgrammerLifecycleStore;
	transport: ProgrammerLifecycleEventTransport | null;
	loadSnapshot(): Promise<ProgrammerLifecycleSnapshot>;
	onError?: (error: Error | null) => void;
}

export class ProgrammerLifecycleSession {
	private readonly authorityKey: string;
	private readonly store: ProgrammerLifecycleStore;
	private readonly transport: ProgrammerLifecycleEventTransport | null;
	private readonly loadSnapshot: ProgrammerLifecycleSessionOptions["loadSnapshot"];
	private readonly onError?: (error: Error | null) => void;
	private references = 0;
	private lifecycle = 0;
	private stopped = false;
	private stream: ProgrammerLifecycleEventStream | null = null;
	private storeScope: number | null = null;
	private hydrationGeneration: number | null = null;
	private repairGeneration: number | null = null;
	private repairPromise: Promise<void> | null = null;
	private refreshQueued = false;
	private reconnectTimer: ReturnType<typeof globalThis.setTimeout> | null =
		null;

	constructor(options: ProgrammerLifecycleSessionOptions) {
		this.authorityKey = options.authorityKey;
		this.store = options.store;
		this.transport = options.transport;
		this.loadSnapshot = options.loadSnapshot;
		this.onError = options.onError;
	}

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
			throw new Error("The Programmer lifecycle authority is unavailable");
		await this.repair(this.lifecycle, error);
		const state = this.store.getSnapshot();
		if (state.repairRequired)
			throw state.error ?? new Error("Programmer lifecycle repair failed");
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
		const state = this.store.getSnapshot();
		if (state.projection && !state.repairRequired) {
			this.store.setReady(this.expectedStoreScope());
			this.onError?.(null);
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
		this.store.setLoading(this.expectedStoreScope());
		try {
			const snapshot = await this.loadSnapshot();
			if (!this.isCurrent(generation)) return;
			const installed = repair
				? this.store.installRepairSnapshot(snapshot, this.expectedStoreScope())
				: this.store.installSnapshot(snapshot, this.expectedStoreScope());
			if (!installed) throw this.scopeError("snapshot");
			this.onError?.(null);
			this.openStream(generation, snapshot.cursor);
		} catch (reason) {
			if (this.isCurrent(generation)) this.fail(asError(reason));
		}
	}

	private openStream(generation: number, cursor: number | null) {
		if (!this.transport || !this.isCurrent(generation)) return;
		let stream: ProgrammerLifecycleEventStream;
		try {
			stream = this.transport.subscribe(cursor, {
				message: (message) => {
					if (this.isCurrent(generation))
						this.handleMessage(message, generation);
				},
				error: (error) => {
					if (!this.isCurrent(generation)) return;
					if (error instanceof ProgrammerLifecycleProtocolError)
						void this.repair(generation, error);
					else this.fail(error);
				},
				closed: () => this.connectionClosed(generation),
			});
		} catch (reason) {
			if (this.isCurrent(generation)) this.fail(asError(reason));
			return;
		}
		if (!this.isCurrent(generation)) stream.close();
		else this.stream = stream;
	}

	private handleMessage(
		message: ProgrammerLifecycleEventMessage,
		generation: number,
	) {
		if (message.type === "ready" || message.type === "repaired") {
			this.store.setReady(this.expectedStoreScope());
			this.onError?.(null);
			return;
		}
		if (message.type === "gap" || message.type === "error") {
			const error = new ProgrammerLifecycleProtocolError(
				message.type === "gap"
					? "Programmer lifecycle event history has a gap"
					: message.error,
				message.type === "gap" ? message.afterSequence : null,
			);
			void this.repair(generation, error);
			return;
		}
		try {
			this.store.applyChange(
				message.change,
				message.sequence,
				this.expectedStoreScope(),
			);
		} catch (reason) {
			void this.repair(generation, asError(reason));
		}
	}

	private repair(generation: number, error: Error) {
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
		this.store.setRepairRequired(error, this.expectedStoreScope());
		this.onError?.(error);
		try {
			const snapshot = await this.loadSnapshot();
			if (!this.isCurrent(generation)) return;
			if (
				!this.store.installRepairSnapshot(snapshot, this.expectedStoreScope())
			)
				throw this.scopeError("repair snapshot");
			this.stream?.repair(snapshot.cursor);
			this.onError?.(null);
		} catch (reason) {
			if (this.isCurrent(generation)) this.fail(asError(reason));
		}
	}

	private fail(error: Error) {
		this.hydrationGeneration = null;
		this.store.setError(error, this.expectedStoreScope());
		this.onError?.(error);
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
		const state = this.store.getSnapshot();
		if (state.authorityKey === null) this.store.reset(this.authorityKey);
		if (this.store.getSnapshot().authorityKey !== this.authorityKey) {
			this.onError?.(this.scopeError("session"));
			return false;
		}
		this.storeScope = this.store.captureScope();
		return true;
	}

	private scopeError(subject: string) {
		return new ProgrammerLifecycleProtocolError(
			`Programmer lifecycle ${subject} no longer matches the active authority`,
		);
	}

	private isCurrent(generation: number) {
		return (
			!this.stopped &&
			this.references > 0 &&
			generation === this.lifecycle &&
			this.store.isScopeCurrent(this.expectedStoreScope())
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

function asError(reason: unknown) {
	return reason instanceof Error ? reason : new Error(String(reason));
}
