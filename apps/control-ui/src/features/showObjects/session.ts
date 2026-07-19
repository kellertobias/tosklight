import type {
	ShowObject,
	ShowObjectKind,
	ShowObjectsChange,
	ShowObjectsEventMessage,
} from "./contracts";
import { ShowObjectsStore } from "./store";
import type {
	ShowObjectsEventStream,
	ShowObjectsEventTransport,
} from "./transport";

export type ShowObjectCollectionLoader = (
	showId: string,
	kind: ShowObjectKind,
) => Promise<ShowObject[]>;

export interface ShowObjectsSessionOptions {
	showId: string;
	store: ShowObjectsStore;
	transport: ShowObjectsEventTransport;
	loadCollection: ShowObjectCollectionLoader;
	onError?: (error: Error | null) => void;
}

export class ShowObjectsSession {
	private readonly showId: string;
	private readonly store: ShowObjectsStore;
	private readonly transport: ShowObjectsEventTransport;
	private readonly loadCollection: ShowObjectCollectionLoader;
	private readonly onError?: (error: Error | null) => void;
	private readonly active = new Map<ShowObjectKind, number>();
	private readonly hydrating = new Set<ShowObjectKind>();
	private readonly hydrationEpochs = new Map<ShowObjectKind, number>();
	private readonly queued: ShowObjectsChange[] = [];
	private stream: ShowObjectsEventStream | null = null;
	private reconnectTimer: ReturnType<typeof globalThis.setTimeout> | null = null;
	private cursor: number | null = null;
	private ready = false;
	private generation = 0;

	constructor(options: ShowObjectsSessionOptions) {
		this.showId = options.showId;
		this.store = options.store;
		this.transport = options.transport;
		this.loadCollection = options.loadCollection;
		this.onError = options.onError;
	}

	activate(kind: ShowObjectKind) {
		const previous = this.active.get(kind) ?? 0;
		this.active.set(kind, previous + 1);
		if (!this.stream) this.openStream();
		else if (!previous && this.ready) void this.hydrate(kind);
		let active = true;
		return () => {
			if (!active) return;
			active = false;
			this.deactivate(kind);
		};
	}

	stop() {
		this.active.clear();
		this.closeStream();
	}

	private deactivate(kind: ShowObjectKind) {
		const count = this.active.get(kind) ?? 0;
		if (count > 1) this.active.set(kind, count - 1);
		else {
			this.active.delete(kind);
			this.hydrationEpochs.set(kind, (this.hydrationEpochs.get(kind) ?? 0) + 1);
			this.hydrating.delete(kind);
			this.flushQueued();
		}
		if (!this.active.size) this.closeStream();
	}

	private openStream(afterSequence: number | null = this.cursor) {
		if (!this.active.size || this.stream) return;
		const generation = ++this.generation;
		this.ready = false;
		this.stream = this.transport.subscribe(this.showId, afterSequence, {
			message: (message) => {
				if (generation === this.generation) this.handleMessage(message);
			},
			error: (error) => {
				if (generation === this.generation) this.fail(error, true);
			},
			closed: () => {
				if (generation === this.generation) this.reconnect(false);
			},
		});
	}

	private handleMessage(message: ShowObjectsEventMessage) {
		switch (message.type) {
			case "ready":
			case "repaired":
				this.cursor = message.cursor;
				this.ready = true;
				for (const kind of this.active.keys())
					void this.hydrate(kind, message.cursor);
				return;
			case "event":
				this.cursor = Math.max(this.cursor ?? 0, message.change.eventSequence);
				this.routeChange(message.change);
				return;
			case "gap":
				this.fullResync();
				return;
			case "error": {
				const error = new Error(message.error);
				this.store.setError(error);
				this.onError?.(error);
				this.fullResync();
				return;
			}
		}
	}

	private routeChange(change: ShowObjectsChange) {
		const changes = change.changes.filter((item) => this.active.has(item.kind));
		if (!changes.length) return;
		const scoped = { ...change, changes };
		if (
			this.queued.length ||
			changes.some((item) => this.hydrating.has(item.kind))
		) {
			this.queued.push(scoped);
			return;
		}
		this.store.applyChange(scoped);
	}

	private async hydrate(
		kind: ShowObjectKind,
		eventFloor = this.cursor ?? 0,
	) {
		if (!this.active.has(kind) || this.hydrating.has(kind)) return;
		const generation = this.generation;
		const hydrationEpoch = (this.hydrationEpochs.get(kind) ?? 0) + 1;
		this.hydrationEpochs.set(kind, hydrationEpoch);
		this.hydrating.add(kind);
		this.store.setLoading();
		try {
			const objects = await this.loadCollection(this.showId, kind);
			if (
				generation !== this.generation ||
				this.hydrationEpochs.get(kind) !== hydrationEpoch ||
				!this.active.has(kind)
			)
				return;
			this.store.setCollection(
				this.showId,
				kind,
				objects as never,
				eventFloor,
			);
			this.onError?.(null);
		} catch (reason) {
			if (
				generation === this.generation &&
				this.hydrationEpochs.get(kind) === hydrationEpoch
			)
				this.fail(asError(reason), true);
		} finally {
			if (
				generation === this.generation &&
				this.hydrationEpochs.get(kind) === hydrationEpoch
			) {
				this.hydrating.delete(kind);
				this.flushQueued();
			}
		}
	}

	private flushQueued() {
		if (this.hydrating.size || !this.queued.length) return;
		const queued = this.queued.splice(0).sort(
			(left, right) => left.eventSequence - right.eventSequence,
		);
		for (const change of queued) {
			const changes = change.changes.filter((item) => this.active.has(item.kind));
			if (changes.length) this.store.applyChange({ ...change, changes });
		}
	}

	private fail(error: Error, reconnect: boolean) {
		this.store.setError(error);
		this.onError?.(error);
		if (reconnect) this.reconnect(false);
	}

	private reconnect(rehydrate: boolean) {
		if (!this.active.size) return;
		this.closeSocket();
		if (rehydrate) this.cursor = null;
		if (this.reconnectTimer != null) return;
		this.reconnectTimer = globalThis.setTimeout(() => {
			this.reconnectTimer = null;
			this.openStream();
		}, 750);
	}

	private fullResync() {
		this.cursor = null;
		this.store.beginEventResync();
		this.reconnect(true);
	}

	private closeSocket() {
		this.generation++;
		this.ready = false;
		this.hydrating.clear();
		this.hydrationEpochs.clear();
		this.queued.length = 0;
		this.stream?.close();
		this.stream = null;
	}

	private closeStream() {
		if (this.reconnectTimer != null)
			globalThis.clearTimeout(this.reconnectTimer);
		this.reconnectTimer = null;
		this.closeSocket();
	}
}

function asError(reason: unknown): Error {
	return reason instanceof Error ? reason : new Error(String(reason));
}
