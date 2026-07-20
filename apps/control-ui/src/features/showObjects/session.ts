import type {
	ShowObjectsChange,
	ShowObjectsEventMessage,
} from "./contracts";
import { ShowObjectsChangeQueue } from "./changeQueue";
import { ShowObjectsCursors } from "./cursors";
import {
	loadHydration,
	type ShowObjectCollectionLoader,
	type ShowObjectLoader,
} from "./hydration";
import {
	hydrationKey,
	type HydrationTarget,
	ShowObjectsViewScope,
} from "./scope";
import {
	isChangeHydrating,
	isHydratedTarget,
	isNeededTarget,
} from "./sessionPredicates";
import type {
	HydrationRun,
	ShowObjectsSessionOptions,
	SnapshotBoundary,
} from "./sessionTypes";
import { asError, clearSessionTimers } from "./sessionUtils";
import { ShowObjectsStore } from "./store";
import {
	ShowObjectsProtocolError,
	type ShowObjectsEventStream,
	type ShowObjectsEventTransport,
} from "./transport";

export type { ShowObjectCollectionLoader, ShowObjectLoader } from "./hydration";
export type { ShowObjectsSessionOptions } from "./sessionTypes";

export class ShowObjectsSession {
	private readonly showId: string;
	private readonly store: ShowObjectsStore;
	private readonly transport: ShowObjectsEventTransport | null;
	private readonly loadCollection: ShowObjectCollectionLoader;
	private readonly loadObject: ShowObjectLoader;
	private readonly onError?: (error: Error | null) => void;
	private readonly scope = new ShowObjectsViewScope();
	private readonly hydrated = new Set<string>();
	private readonly hydrationCoverage = new Map<string, number>();
	private readonly runs = new Map<string, HydrationRun>();
	private readonly forcedFloors = new Map<string, number>();
	private readonly queued = new ShowObjectsChangeQueue();
	private readonly cursors = new ShowObjectsCursors();
	private stream: ShowObjectsEventStream | null = null;
	private streamScopeKey: string | null = null;
	private reconnectTimer: ReturnType<typeof globalThis.setTimeout> | null = null;
	private hydrationRetryTimer: ReturnType<typeof globalThis.setTimeout> | null = null;
	private streamRefreshScheduled = false;
	private socketGeneration = 0;
	private lifecycleGeneration = 0;
	private pendingBoundary: SnapshotBoundary | null = null;
	private subscribeFromLatest = false;

	constructor(options: ShowObjectsSessionOptions) {
		this.showId = options.showId;
		this.store = options.store;
		this.transport = options.transport;
		this.loadCollection = options.loadCollection;
		this.loadObject = options.loadObject;
		this.onError = options.onError;
	}

	activate(kind: HydrationTarget["kind"], objectId?: string) {
		this.scope.activate(kind, objectId);
		this.ensureHydrations();
		this.reconcileStream();
		let active = true;
		return () => {
			if (!active) return;
			active = false;
			this.deactivate(kind, objectId);
		};
	}

	stop() {
		this.scope.clear();
		this.lifecycleGeneration++;
		this.clearHydrations();
		this.clearTimers();
		this.queued.clear();
		this.closeSocket();
	}

	private deactivate(kind: HydrationTarget["kind"], objectId?: string) {
		const removed = this.scope.deactivate(kind, objectId);
		if (removed) this.invalidateHydration({ kind, objectId });
		if (!this.scope.hasViews()) {
			this.queued.clear();
			this.clearHydrations();
			this.clearTimers();
			this.closeSocket();
			this.store.setReady();
			return;
		}
		this.ensureHydrations();
		this.reconcileStream();
		this.flushQueued();
	}

	private ensureHydrations(floor = this.cursors.resume() ?? 0, force = false) {
		for (const target of this.scope.targets())
			this.requestHydration(target, floor, force);
	}

	private requestHydration(
		target: HydrationTarget,
		floor: number,
		force = false,
	) {
		if (!isNeededTarget(this.scope, target)) return;
		const key = hydrationKey(target);
		if (force && (this.hydrationCoverage.get(key) ?? -1) >= floor) return;
		if (!force && isHydratedTarget(this.hydrated, target)) return;
		if (this.runs.has(key)) {
			if (force)
				this.forcedFloors.set(key, Math.max(this.forcedFloors.get(key) ?? 0, floor));
			return;
		}
		const run = { token: Symbol(key), target, floor };
		this.runs.set(key, run);
		this.store.setLoading(target.objectId === undefined ? target.kind : undefined);
		void this.runHydration(run);
	}

	private async runHydration(run: HydrationRun) {
		const lifecycle = this.lifecycleGeneration;
		try {
			const loaded = await loadHydration(
				run.target,
				this.showId,
				this.loadCollection,
				this.loadObject,
			);
			if (!this.isCurrentRun(run, lifecycle)) return;
			const previousScope = this.scope.key();
			if (run.target.objectId === undefined)
				this.store.setCollection(
					this.showId,
					run.target.kind,
					loaded.collection as never,
					run.floor,
				);
			else
				this.store.installObjects(this.showId, loaded.installs, run.floor);
			if (run.target.kind === "group" && run.target.objectId)
				this.scope.setGroupDependencies(
					run.target.objectId,
					loaded.groupDependencies ?? [],
				);
			this.hydrated.add(hydrationKey(run.target));
			this.hydrationCoverage.set(
				hydrationKey(run.target),
				Math.max(this.hydrationCoverage.get(hydrationKey(run.target)) ?? 0, run.floor),
			);
			this.onError?.(null);
			if (previousScope !== this.scope.key()) this.reconcileStream();
		} catch (reason) {
			if (this.isCurrentRun(run, lifecycle)) {
				const error = asError(reason);
				this.store.setError(error);
				this.onError?.(error);
				this.scheduleHydrationRetry();
			}
		} finally {
			if (this.runs.get(hydrationKey(run.target))?.token === run.token) {
				this.runs.delete(hydrationKey(run.target));
				const forced = this.forcedFloors.get(hydrationKey(run.target));
				this.forcedFloors.delete(hydrationKey(run.target));
				if (forced != null) this.requestHydration(run.target, forced, true);
			}
			this.completeBoundary();
			this.flushQueued();
		}
	}

	private isCurrentRun(run: HydrationRun, lifecycle: number) {
		return (
			lifecycle === this.lifecycleGeneration &&
			this.runs.get(hydrationKey(run.target))?.token === run.token &&
			isNeededTarget(this.scope, run.target)
		);
	}

	private openStream() {
		if (!this.transport || !this.scope.hasViews() || this.stream) return;
		const generation = ++this.socketGeneration;
		const scope = this.scope.subscription();
		const afterSequence = this.subscribeFromLatest
			? null
			: (this.cursors.resume() ?? 0);
		this.subscribeFromLatest = false;
		this.streamScopeKey = JSON.stringify(scope);
		this.stream = this.transport.subscribe(this.showId, scope, afterSequence, {
			message: (message) => {
				if (generation === this.socketGeneration)
					this.handleMessage(message, generation, afterSequence);
			},
			error: (error) => {
				if (generation !== this.socketGeneration) return;
				if (error instanceof ShowObjectsProtocolError) this.protocolReset(error);
				else this.failTransport(error);
			},
			closed: () => {
				if (generation === this.socketGeneration) this.scheduleReconnect();
			},
		});
	}

	private handleMessage(
		message: ShowObjectsEventMessage,
		generation: number,
		afterSequence: number | null,
	) {
		switch (message.type) {
			case "ready":
				if (afterSequence === null)
					this.beginSnapshotBoundary(generation, message.cursor, false);
				return;
			case "repaired":
				this.onError?.(null);
				return;
			case "event":
				this.routeChange(message.change);
				return;
			case "gap":
				this.beginSnapshotBoundary(generation, message.latestSequence, true);
				return;
			case "error":
				this.protocolReset(new Error(message.error));
		}
	}

	private routeChange(change: ShowObjectsChange) {
		const relevant = change.changes.filter((item) => this.scope.includesChange(item));
		if (!relevant.length) return;
		if (
			this.queued.size ||
			relevant.some((item) =>
				isChangeHydrating(
					this.scope,
					(key) => this.runs.has(key),
					item.kind,
					item.objectId,
				),
			)
		) {
			this.queued.push(change);
			return;
		}
		this.installChange(change, relevant);
	}

	private installChange(
		change: ShowObjectsChange,
		relevant = change.changes.filter((item) => this.scope.includesChange(item)),
	) {
		if (!relevant.length) return;
		this.store.applyChange({ ...change, changes: relevant });
		this.cursors.installEvent(change.eventSequence);
		const changedGroups = new Set(
			relevant.filter((item) => item.kind === "group").map((item) => item.objectId),
		);
		for (const targetId of this.scope.affectedExactGroups(changedGroups))
			this.requestHydration(
				{ kind: "group", objectId: targetId },
				change.eventSequence,
				true,
			);
	}

	private flushQueued() {
		while (!this.runs.size && this.queued.size) {
			const change = this.queued.shift();
			if (change) this.installChange(change);
		}
	}

	private beginSnapshotBoundary(generation: number, cursor: number, repair: boolean) {
		this.clearHydrations();
		this.queued.clear();
		this.store.beginEventResync();
		this.pendingBoundary = {
			generation,
			cursor,
			repair,
			targets: new Set(this.scope.targets().map(hydrationKey)),
		};
		this.ensureHydrations(cursor, true);
		this.completeBoundary();
	}

	private completeBoundary() {
		const boundary = this.pendingBoundary;
		if (!boundary || boundary.generation !== this.socketGeneration) return;
		for (const key of boundary.targets)
			if (
				this.scope.targets().some((target) => hydrationKey(target) === key) &&
				(this.hydrationCoverage.get(key) ?? -1) < boundary.cursor
			)
				return;
		this.pendingBoundary = null;
		this.cursors.installSnapshot(boundary.cursor);
		this.store.setReady();
		this.onError?.(null);
		if (boundary.repair) this.stream?.repair(boundary.cursor);
	}

	private protocolReset(error: Error) {
		this.store.setError(error);
		this.onError?.(error);
		this.cursors.reset();
		this.pendingBoundary = null;
		this.subscribeFromLatest = true;
		this.queued.clear();
		this.clearHydrations();
		this.store.beginEventResync();
		this.closeSocket();
		this.ensureHydrations(0, true);
		this.scheduleReconnect(0);
	}

	private failTransport(error: Error) {
		this.store.setError(error);
		this.onError?.(error);
		this.scheduleReconnect();
	}

	private reconcileStream() {
		if (!this.transport || !this.scope.hasViews()) return;
		if (!this.stream) {
			if (this.reconnectTimer == null) this.openStream();
			return;
		}
		if (this.streamScopeKey === this.scope.key() || this.streamRefreshScheduled)
			return;
		this.streamRefreshScheduled = true;
		globalThis.queueMicrotask(() => {
			this.streamRefreshScheduled = false;
			if (!this.scope.hasViews() || this.streamScopeKey === this.scope.key()) return;
			this.closeSocket();
			this.openStream();
		});
	}

	private scheduleReconnect(delay = 750) {
		if (!this.scope.hasViews()) return;
		this.closeSocket();
		if (this.reconnectTimer != null) return;
		this.reconnectTimer = globalThis.setTimeout(() => {
			this.reconnectTimer = null;
			this.openStream();
		}, delay);
	}

	private scheduleHydrationRetry() {
		if (this.hydrationRetryTimer != null || !this.scope.hasViews()) return;
		this.hydrationRetryTimer = globalThis.setTimeout(() => {
			this.hydrationRetryTimer = null;
			this.ensureHydrations();
		}, 750);
	}

	private invalidateHydration(target: HydrationTarget) {
		const key = hydrationKey(target);
		this.hydrated.delete(key);
		this.hydrationCoverage.delete(key);
		this.runs.delete(key);
		this.forcedFloors.delete(key);
	}

	private clearHydrations() {
		this.runs.clear();
		this.forcedFloors.clear();
		this.hydrated.clear();
		this.hydrationCoverage.clear();
	}

	private closeSocket() {
		this.socketGeneration++;
		this.pendingBoundary = null;
		this.stream?.close();
		this.stream = null;
		this.streamScopeKey = null;
	}

	private clearTimers() {
		clearSessionTimers(this.reconnectTimer, this.hydrationRetryTimer);
		this.reconnectTimer = null;
		this.hydrationRetryTimer = null;
	}
}
