import type {
	PlaybackIdentity,
	PlaybackRuntimeEventMessage,
	PlaybackSnapshot,
} from "./contracts";
import { identityKey, projectionKeys } from "./contracts";
import { PlaybackViewScope } from "./scope";
import type { PlaybackRuntimeStore } from "./store";
import {
	type PlaybackEventStream,
	type PlaybackEventTransport,
	PlaybackProtocolError,
} from "./transport";

export interface PlaybackRuntimeSessionOptions {
	showId: string;
	deskId: string;
	authorityKey: string;
	store: PlaybackRuntimeStore;
	transport: PlaybackEventTransport | null;
	loadSnapshot(identities: PlaybackIdentity[]): Promise<PlaybackSnapshot>;
	onError?: (error: Error | null) => void;
}

export class PlaybackRuntimeSession {
	private readonly scope = new PlaybackViewScope();
	private readonly showId: string;
	private readonly deskId: string;
	private readonly storeScope: number;
	private readonly store: PlaybackRuntimeStore;
	private readonly transport: PlaybackEventTransport | null;
	private readonly loadSnapshot: PlaybackRuntimeSessionOptions["loadSnapshot"];
	private readonly onError?: (error: Error | null) => void;
	private stream: PlaybackEventStream | null = null;
	private refreshScheduled = false;
	private lifecycle = 0;
	private reconnectTimer: ReturnType<typeof globalThis.setTimeout> | null =
		null;
	private repairRunning = false;

	constructor(options: PlaybackRuntimeSessionOptions) {
		this.showId = options.showId;
		this.deskId = options.deskId;
		this.store = options.store;
		this.transport = options.transport;
		this.loadSnapshot = options.loadSnapshot;
		this.onError = options.onError;
		this.store.reset(this.showId, this.deskId, options.authorityKey);
		this.storeScope = this.store.captureScope();
	}

	activate(identity: PlaybackIdentity) {
		if (this.scope.activate(identity)) this.scheduleRefresh();
		let active = true;
		return () => {
			if (!active) return;
			active = false;
			if (this.scope.deactivate(identity)) this.scheduleRefresh();
		};
	}

	activateDesk() {
		if (this.scope.activateDesk()) this.scheduleRefresh();
		let active = true;
		return () => {
			if (!active) return;
			active = false;
			if (this.scope.deactivateDesk()) this.scheduleRefresh();
		};
	}

	stop() {
		this.scope.clear();
		this.lifecycle++;
		this.clearReconnect();
		this.closeStream();
	}

	private scheduleRefresh(delay = 0) {
		if (this.refreshScheduled) return;
		this.refreshScheduled = true;
		const refresh = () => {
			this.refreshScheduled = false;
			void this.refresh();
		};
		if (delay === 0) globalThis.queueMicrotask(refresh);
		else this.reconnectTimer = globalThis.setTimeout(refresh, delay);
	}

	private async refresh() {
		if (!this.store.isScopeCurrent(this.storeScope)) return;
		const generation = ++this.lifecycle;
		this.clearReconnect();
		this.closeStream();
		if (!this.scope.hasViews()) {
			this.store.setReady();
			return;
		}
		const key = this.scope.key();
		const identities = this.scope.values();
		this.store.setLoading();
		let cursor: number;
		try {
			const snapshots = await this.loadSnapshots(identities);
			if (!this.isCurrent(generation, key)) return;
			for (const { snapshot, identities: batch } of snapshots) {
				this.assertSnapshotScope(snapshot, batch);
				this.store.installSnapshot(snapshot, batch);
			}
			cursor = Math.min(
				...snapshots.map(({ snapshot }) => snapshot.cursor.sequence),
			);
			this.onError?.(null);
		} catch (reason) {
			if (!this.isCurrent(generation, key)) return;
			this.fail(asError(reason));
			return;
		}
		if (!this.isCurrent(generation, key) || !this.transport) return;
		this.openStream(generation, key, cursor);
	}

	private openStream(generation: number, key: string, cursor: number | null) {
		const scope = this.scope.subscription();
		this.stream =
			this.transport?.subscribe(this.deskId, scope, cursor, {
				message: (message) => {
					if (this.isCurrent(generation, key))
						this.handleMessage(message, generation);
				},
				error: (error) => {
					if (!this.isCurrent(generation, key)) return;
					if (error instanceof PlaybackProtocolError) this.protocolReset(error);
					else this.fail(error);
				},
				closed: () => {
					if (this.isCurrent(generation, key)) this.scheduleReconnect();
				},
			}) ?? null;
	}

	private handleMessage(
		message: PlaybackRuntimeEventMessage,
		generation: number,
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
			void this.repair(generation);
			return;
		}
		if (message.payload.type === "runtime") {
			const projection = message.payload.projection;
			if (
				projection.scope.show_id === this.showId &&
				this.scope.includesProjection(projection)
			)
				this.store.applyProjection(projection, message.sequence);
			return;
		}
		const projection = message.payload.projection;
		if (
			this.scope.subscription().desk &&
			projection.scope.show_id === this.showId &&
			projection.desk_id === this.deskId
		)
			this.store.applyDesk(projection, message.sequence);
	}

	private async repair(generation: number) {
		if (this.repairRunning) return;
		const identities = this.scope.values();
		this.repairRunning = true;
		try {
			const snapshots = await this.loadSnapshots(identities);
			if (
				generation !== this.lifecycle ||
				!this.store.isScopeCurrent(this.storeScope)
			)
				return;
			for (const { snapshot, identities: batch } of snapshots) {
				this.assertSnapshotScope(snapshot, batch);
				this.store.installSnapshot(snapshot, batch);
			}
			this.stream?.repair(
				Math.min(...snapshots.map(({ snapshot }) => snapshot.cursor.sequence)),
			);
			this.onError?.(null);
		} catch (reason) {
			if (generation === this.lifecycle) this.protocolReset(asError(reason));
		} finally {
			this.repairRunning = false;
		}
	}

	private async loadSnapshots(identities: PlaybackIdentity[]) {
		const batches = identities.length ? chunk(identities, 256) : [[]];
		const snapshots = [];
		for (const batch of batches)
			snapshots.push({
				identities: batch,
				snapshot: await this.loadSnapshot(batch),
			});
		return snapshots;
	}

	private assertSnapshotScope(
		snapshot: PlaybackSnapshot,
		identities: readonly PlaybackIdentity[],
	) {
		if (
			snapshot.desk.scope.show_id !== this.showId ||
			snapshot.desk.desk_id !== this.deskId
		)
			throw new PlaybackProtocolError(
				"Playback snapshot does not match the active show and desk",
			);
		const requested = new Set(identities.map(identityKey));
		for (const projection of snapshot.projections) {
			if (projection.scope.show_id !== this.showId)
				throw new PlaybackProtocolError(
					"Playback snapshot contains a projection from another show",
				);
			if (!projectionKeys(projection).some((key) => requested.has(key)))
				throw new PlaybackProtocolError(
					"Playback snapshot contains an unrequested projection",
				);
		}
	}

	private protocolReset(error: Error) {
		this.store.setError(error);
		this.onError?.(error);
		this.closeStream();
		this.scheduleRefresh(0);
	}

	private fail(error: Error) {
		this.store.setError(error);
		this.onError?.(error);
		this.closeStream();
		this.scheduleReconnect();
	}

	private scheduleReconnect() {
		this.clearReconnect();
		this.scheduleRefresh(750);
	}

	private isCurrent(generation: number, key: string) {
		return (
			generation === this.lifecycle &&
			key === this.scope.key() &&
			this.store.isScopeCurrent(this.storeScope)
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

function chunk<T>(values: T[], size: number) {
	const result: T[][] = [];
	for (let index = 0; index < values.length; index += size)
		result.push(values.slice(index, index + size));
	return result;
}
