import type {
	ProgrammingCapability,
	ProgrammingInteractionEventMessage,
	ProgrammingSnapshot,
} from "./contracts";
import { ProgrammingViewScope } from "./scope";
import type { ProgrammingInteractionStore } from "./store";
import {
	type ProgrammingEventStream,
	type ProgrammingEventTransport,
	ProgrammingProtocolError,
} from "./transport";

export interface ProgrammingInteractionSessionOptions {
	showId: string;
	deskId: string;
	store: ProgrammingInteractionStore;
	transport: ProgrammingEventTransport | null;
	loadSnapshot(): Promise<ProgrammingSnapshot>;
	onError?: (error: Error | null) => void;
}

export class ProgrammingInteractionSession {
	private readonly scope = new ProgrammingViewScope();
	private readonly showId: string;
	private readonly deskId: string;
	private readonly store: ProgrammingInteractionStore;
	private readonly transport: ProgrammingEventTransport | null;
	private readonly loadSnapshot: ProgrammingInteractionSessionOptions["loadSnapshot"];
	private readonly onError?: (error: Error | null) => void;
	private stream: ProgrammingEventStream | null = null;
	private reconnectTimer: ReturnType<typeof globalThis.setTimeout> | null = null;
	private refreshScheduled = false;
	private repairGeneration: number | null = null;
	private hydratedScopeKey: string | null = null;
	private lifecycle = 0;
	private stopped = false;

	constructor(options: ProgrammingInteractionSessionOptions) {
		this.showId = options.showId;
		this.deskId = options.deskId;
		this.store = options.store;
		this.transport = options.transport;
		this.loadSnapshot = options.loadSnapshot;
		this.onError = options.onError;
		this.store.reset(this.showId, this.deskId);
	}

	activate(capability: ProgrammingCapability) {
		if (this.scope.activate(capability)) this.scheduleRefresh();
		let active = true;
		return () => {
			if (!active) return;
			active = false;
			if (this.scope.deactivate(capability)) this.scheduleRefresh();
		};
	}

	stop() {
		this.stopped = true;
		this.scope.clear();
		this.lifecycle++;
		this.hydratedScopeKey = null;
		this.clearReconnect();
		this.closeStream();
	}

	private scheduleRefresh(delay = 0) {
		if (this.stopped) return;
		if (delay === 0 && this.reconnectTimer != null) {
			globalThis.clearTimeout(this.reconnectTimer);
			this.reconnectTimer = null;
			this.refreshScheduled = false;
		}
		if (this.refreshScheduled) return;
		this.refreshScheduled = true;
		const refresh = () => {
			this.reconnectTimer = null;
			this.refreshScheduled = false;
			if (!this.stopped) void this.refresh();
		};
		if (delay === 0) globalThis.queueMicrotask(refresh);
		else this.reconnectTimer = globalThis.setTimeout(refresh, delay);
	}

	private async refresh() {
		const key = this.scope.key();
		if (this.hydratedScopeKey === key) return;
		const generation = ++this.lifecycle;
		this.clearReconnect();
		this.closeStream();
		this.hydratedScopeKey = key;
		if (!this.scope.hasViews()) {
			this.store.setReady();
			return;
		}
		this.store.setLoading();
		let snapshot: ProgrammingSnapshot;
		try {
			snapshot = await this.loadSnapshot();
			if (!this.isCurrent(generation, key)) return;
			this.assertSnapshotDesk(snapshot);
			this.store.installSnapshot(snapshot);
			this.onError?.(null);
		} catch (reason) {
			if (this.isCurrent(generation, key)) this.fail(asError(reason));
			return;
		}
		if (!this.isCurrent(generation, key) || !this.transport) return;
		this.openStream(generation, key, snapshot.cursor);
	}

	private openStream(generation: number, key: string, cursor: number) {
		if (!this.transport) return;
		let stream: ProgrammingEventStream;
		try {
			stream = this.transport.subscribe(
				this.deskId,
				this.scope.subscription(),
				cursor,
				{
					message: (message) => {
						if (this.isCurrent(generation, key))
							this.handleMessage(message, generation, key);
					},
					error: (error) => {
						if (!this.isCurrent(generation, key)) return;
						if (error instanceof ProgrammingProtocolError)
							this.protocolReset(error);
						else this.fail(error);
					},
					closed: () => this.connectionClosed(generation, key),
				},
			);
		} catch (reason) {
			if (this.isCurrent(generation, key)) this.fail(asError(reason));
			return;
		}
		if (!this.isCurrent(generation, key)) {
			stream.close();
			return;
		}
		this.stream = stream;
	}

	private handleMessage(
		message: ProgrammingInteractionEventMessage,
		generation: number,
		key: string,
	) {
		if (message.type === "ready" || message.type === "repaired") {
			this.onError?.(null);
			return;
		}
		if (message.type === "error") {
			this.protocolReset(new Error(message.error));
			return;
		}
		if (message.type === "gap") {
			void this.repair(generation, key);
			return;
		}
		if (!this.scope.includesChange(message.change)) return;
		try {
			this.store.applyChange(message.change, message.sequence);
		} catch (reason) {
			this.protocolReset(asError(reason));
		}
	}

	private async repair(generation: number, key: string) {
		if (this.repairGeneration === generation) return;
		this.repairGeneration = generation;
		try {
			const snapshot = await this.loadSnapshot();
			if (!this.isCurrent(generation, key)) return;
			this.assertSnapshotDesk(snapshot);
			this.store.installSnapshot(snapshot);
			this.stream?.repair(snapshot.cursor);
			this.onError?.(null);
		} catch (reason) {
			if (this.isCurrent(generation, key))
				this.protocolReset(asError(reason));
		} finally {
			if (this.repairGeneration === generation)
				this.repairGeneration = null;
		}
	}

	private assertSnapshotDesk(snapshot: ProgrammingSnapshot) {
		if (snapshot.projection.deskId !== this.deskId)
			throw new ProgrammingProtocolError(
				"Programming interaction snapshot does not match the active desk",
			);
	}

	private protocolReset(error: Error) {
		this.invalidateConnection();
		this.store.setError(error);
		this.onError?.(error);
		this.closeStream();
		this.scheduleRefresh();
	}

	private fail(error: Error) {
		this.invalidateConnection();
		this.store.setError(error);
		this.onError?.(error);
		this.closeStream();
		this.scheduleReconnect();
	}

	private connectionClosed(generation: number, key: string) {
		if (!this.isCurrent(generation, key)) return;
		this.invalidateConnection();
		this.stream = null;
		this.scheduleReconnect();
	}

	private invalidateConnection() {
		this.lifecycle++;
		this.hydratedScopeKey = null;
	}

	private scheduleReconnect() {
		this.clearReconnect();
		this.scheduleRefresh(750);
	}

	private isCurrent(generation: number, key: string) {
		return (
			!this.stopped &&
			generation === this.lifecycle &&
			key === this.scope.key()
		);
	}

	private closeStream() {
		this.stream?.close();
		this.stream = null;
	}

	private clearReconnect() {
		if (this.reconnectTimer != null)
			globalThis.clearTimeout(this.reconnectTimer);
		this.reconnectTimer = null;
	}
}

function asError(reason: unknown) {
	return reason instanceof Error ? reason : new Error(String(reason));
}
