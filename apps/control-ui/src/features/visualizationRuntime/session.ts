import type {
	VisualizationRuntimeLane,
	VisualizationRuntimeScope,
} from "./contracts";
import type { VisualizationRuntimeStore } from "./store";
import {
	type VisualizationRuntimeTransport,
	VisualizationRuntimeProtocolError,
} from "./transport";

interface LaneRuntime {
	claims: Map<number, number>;
	generation: number;
	inFlight: boolean;
	queued: boolean;
	timer: ReturnType<typeof globalThis.setInterval> | null;
}

export interface VisualizationRuntimeSessionOptions {
	scope: VisualizationRuntimeScope;
	store: VisualizationRuntimeStore;
	transport: VisualizationRuntimeTransport;
	onError?: (error: Error | null) => void;
}

/** Owns one non-overlapping polling loop per independently claimed lane. */
export class VisualizationRuntimeSession {
	private readonly scope: VisualizationRuntimeScope;
	private readonly store: VisualizationRuntimeStore;
	private readonly transport: VisualizationRuntimeTransport;
	private readonly onError?: (error: Error | null) => void;
	private readonly lanes: Record<VisualizationRuntimeLane, LaneRuntime> = {
		normal: laneRuntime(),
		preload: laneRuntime(),
	};
	private nextClaimId = 0;
	private lifecycle = 0;
	private stopped = false;

	constructor(options: VisualizationRuntimeSessionOptions) {
		this.scope = options.scope;
		this.store = options.store;
		this.transport = options.transport;
		this.onError = options.onError;
	}

	activate(lane: VisualizationRuntimeLane, intervalMillis: number) {
		assertInterval(intervalMillis);
		if (this.stopped || !this.store.matchesScope(this.scope)) return () => {};
		const runtime = this.lanes[lane];
		const first = runtime.claims.size === 0;
		const claimId = ++this.nextClaimId;
		runtime.claims.set(claimId, intervalMillis);
		if (first) {
			runtime.generation++;
			this.store.setLoading(lane, this.store.captureScope());
			this.scheduleRefresh(lane);
		}
		this.restartTimer(lane);
		let active = true;
		return () => {
			if (!active) return;
			active = false;
			this.release(lane, claimId);
		};
	}

	stop() {
		if (this.stopped) return;
		this.stopped = true;
		this.lifecycle++;
		for (const lane of lanes()) {
			const runtime = this.lanes[lane];
			runtime.claims.clear();
			runtime.generation++;
			this.clearTimer(runtime);
		}
	}

	private release(lane: VisualizationRuntimeLane, claimId: number) {
		const runtime = this.lanes[lane];
		if (!runtime.claims.delete(claimId)) return;
		if (runtime.claims.size) {
			this.restartTimer(lane);
			return;
		}
		runtime.generation++;
		this.clearTimer(runtime);
		if (this.store.matchesScope(this.scope))
			this.store.setIdle(lane, this.store.captureScope());
	}

	private scheduleRefresh(lane: VisualizationRuntimeLane) {
		const runtime = this.lanes[lane];
		if (runtime.queued || this.stopped) return;
		runtime.queued = true;
		globalThis.queueMicrotask(() => {
			runtime.queued = false;
			if (runtime.claims.size && !this.stopped) void this.refresh(lane);
		});
	}

	private restartTimer(lane: VisualizationRuntimeLane) {
		const runtime = this.lanes[lane];
		this.clearTimer(runtime);
		const interval = minimumInterval(runtime);
		if (interval === null) return;
		runtime.timer = globalThis.setInterval(() => void this.refresh(lane), interval);
	}

	private async refresh(lane: VisualizationRuntimeLane) {
		const runtime = this.lanes[lane];
		if (runtime.inFlight || !runtime.claims.size || this.stopped) return;
		const lifecycle = this.lifecycle;
		const laneGeneration = runtime.generation;
		const storeScope = this.store.captureScope();
		runtime.inFlight = true;
		try {
			const snapshot = await this.transport.loadSnapshot(this.scope, lane);
			if (!this.isCurrent(lane, lifecycle, laneGeneration, storeScope)) return;
			this.store.install(lane, snapshot, storeScope);
			this.onError?.(null);
		} catch (reason) {
			if (this.isCurrent(lane, lifecycle, laneGeneration, storeScope)) {
				const error = asError(reason);
				this.store.setError(lane, error, storeScope);
				this.onError?.(error);
			}
		} finally {
			runtime.inFlight = false;
			if (
				runtime.claims.size &&
				!this.stopped &&
				runtime.generation !== laneGeneration
			)
				this.scheduleRefresh(lane);
		}
	}

	private isCurrent(
		lane: VisualizationRuntimeLane,
		lifecycle: number,
		laneGeneration: number,
		storeScope: number,
	) {
		const runtime = this.lanes[lane];
		return (
			!this.stopped &&
			this.lifecycle === lifecycle &&
			runtime.generation === laneGeneration &&
			runtime.claims.size > 0 &&
			this.store.isScopeCurrent(storeScope) &&
			this.store.matchesScope(this.scope)
		);
	}

	private clearTimer(runtime: LaneRuntime) {
		if (runtime.timer !== null) globalThis.clearInterval(runtime.timer);
		runtime.timer = null;
	}
}

function laneRuntime(): LaneRuntime {
	return {
		claims: new Map(),
		generation: 0,
		inFlight: false,
		queued: false,
		timer: null,
	};
}

function minimumInterval(runtime: LaneRuntime) {
	return runtime.claims.size ? Math.min(...runtime.claims.values()) : null;
}

function assertInterval(value: number) {
	if (!Number.isSafeInteger(value) || value < 50 || value > 60_000)
		throw new VisualizationRuntimeProtocolError(
			"Visualization polling interval must be 50-60000 milliseconds",
		);
}

function lanes(): VisualizationRuntimeLane[] {
	return ["normal", "preload"];
}

function asError(reason: unknown) {
	return reason instanceof Error ? reason : new Error(String(reason));
}
