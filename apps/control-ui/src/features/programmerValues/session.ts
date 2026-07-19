import type {
	ProgrammerValuesEventMessage,
	ProgrammerValuesScope,
	ProgrammerValuesSnapshot,
} from "./contracts";
import type { ProgrammerValuesStore } from "./store";
import {
	type ProgrammerValuesEventStream,
	type ProgrammerValuesEventTransport,
	ProgrammerValuesProtocolError,
} from "./transport";

export interface ProgrammerValuesSessionOptions {
	showId: string;
	userId: string;
	store: ProgrammerValuesStore;
	transport: ProgrammerValuesEventTransport | null;
	loadSnapshot(): Promise<ProgrammerValuesSnapshot>;
	onError?: (error: Error | null) => void;
}

export class ProgrammerValuesSession {
	private readonly eventScope: ProgrammerValuesScope;
	private readonly store: ProgrammerValuesStore;
	private readonly transport: ProgrammerValuesEventTransport | null;
	private readonly loadSnapshot: ProgrammerValuesSessionOptions["loadSnapshot"];
	private readonly onError?: (error: Error | null) => void;
	private references = 0;
	private lifecycle = 0;
	private hydrated = false;
	private stopped = false;
	private stream: ProgrammerValuesEventStream | null = null;
	private storeScope: number | null = null;
	private hydrationGeneration: number | null = null;
	private repairGeneration: number | null = null;
	private refreshQueued = false;
	private reconnectTimer: ReturnType<typeof globalThis.setTimeout> | null = null;

	constructor(options: ProgrammerValuesSessionOptions) {
		this.eventScope = { showId: options.showId, userId: options.userId };
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
		if (this.hydrated && state.projection && !state.repairRequired) {
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
			this.assertSnapshotUser(snapshot);
			const installed = repair
				? this.store.installRepairSnapshot(
						snapshot,
						this.expectedStoreScope(),
					)
				: this.store.installSnapshot(snapshot, {
						expectedScope: this.expectedStoreScope(),
					});
			if (!installed) throw this.scopeError("snapshot");
			this.hydrated = true;
			this.store.setReady(this.expectedStoreScope());
			this.onError?.(null);
			this.openStream(generation, this.store.getSnapshot().eventSequence);
		} catch (reason) {
			if (this.isCurrent(generation)) this.fail(asError(reason), true);
		}
	}

	private openStream(generation: number, cursor: number | null) {
		if (!this.transport || !this.isCurrent(generation)) return;
		let stream: ProgrammerValuesEventStream;
		try {
			stream = this.transport.subscribe(this.eventScope, cursor, {
				message: (message) => {
					if (this.isCurrent(generation))
						this.handleMessage(message, generation);
				},
				error: (error) => {
					if (!this.isCurrent(generation)) return;
					if (error instanceof ProgrammerValuesProtocolError)
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

	private handleMessage(
		message: ProgrammerValuesEventMessage,
		generation: number,
	) {
		if (message.type === "ready" || message.type === "repaired") {
			this.store.setReady(this.expectedStoreScope());
			this.onError?.(null);
			return;
		}
		if (message.type === "gap") {
			void this.repair(
				generation,
				new ProgrammerValuesProtocolError(
					"Programmer values event history has a gap",
					message.afterSequence,
				),
			);
			return;
		}
		if (message.type === "error") {
			void this.repair(
				generation,
				new ProgrammerValuesProtocolError(message.error),
			);
			return;
		}
		if (message.projection.userId !== this.eventScope.userId) return;
		try {
			this.store.applyProjection(
				message.projection,
				message.sequence,
				this.expectedStoreScope(),
			);
		} catch (reason) {
			void this.repair(generation, asError(reason));
		}
	}

	private async repair(generation: number, error: Error) {
		if (this.repairGeneration === generation || !this.isCurrent(generation))
			return;
		this.repairGeneration = generation;
		this.store.setRepairRequired(error, this.expectedStoreScope());
		this.onError?.(error);
		try {
			const snapshot = await this.loadSnapshot();
			if (!this.isCurrent(generation)) return;
			this.assertSnapshotUser(snapshot);
			if (
				!this.store.installRepairSnapshot(
					snapshot,
					this.expectedStoreScope(),
				)
			)
				throw this.scopeError("repair snapshot");
			this.hydrated = true;
			this.stream?.repair(snapshot.cursor);
			this.onError?.(null);
		} catch (reason) {
			if (this.isCurrent(generation)) this.fail(asError(reason), true);
		} finally {
			if (this.repairGeneration === generation)
				this.repairGeneration = null;
		}
	}

	private fail(error: Error, invalidateHydration: boolean) {
		if (invalidateHydration) this.hydrated = false;
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
		if (state.showId === null && state.userId === null)
			this.store.reset(this.eventScope.showId, this.eventScope.userId);
		const scoped = this.store.getSnapshot();
		if (
			scoped.showId !== this.eventScope.showId ||
			scoped.userId !== this.eventScope.userId
		) {
			this.onError?.(this.scopeError("session"));
			return false;
		}
		this.storeScope = this.store.captureScope();
		return true;
	}

	private assertSnapshotUser(snapshot: ProgrammerValuesSnapshot) {
		if (snapshot.projection.userId !== this.eventScope.userId)
			throw this.scopeError("snapshot user");
	}

	private scopeError(subject: string) {
		return new ProgrammerValuesProtocolError(
			`Programmer values ${subject} does not match the active user view`,
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
