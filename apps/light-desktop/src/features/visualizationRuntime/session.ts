import type { VisualizationSnapshot } from "../../api/types";
import { frontendPerformanceDiagnostics } from "../frontendWarmup/diagnostics";
import type {
	VisualizationRuntimeLane,
	VisualizationRuntimeScope,
} from "./contracts";
import type { VisualizationRuntimeStore } from "./store";
import {
	VisualizationRuntimeProtocolError,
	type VisualizationRuntimeStream,
	type VisualizationRuntimeTransport,
} from "./transport";

interface LaneRuntime {
	claims: Map<
		number,
		{
			intervalMillis: number;
			consumerId: string;
			includeDynamicStack: boolean;
		}
	>;
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
	private stream: VisualizationRuntimeStream | null = null;

	constructor(options: VisualizationRuntimeSessionOptions) {
		this.scope = options.scope;
		this.store = options.store;
		this.transport = options.transport;
		this.onError = options.onError;
	}

	activate(
		lane: VisualizationRuntimeLane,
		intervalMillis: number,
		consumerId = "anonymous",
		includeDynamicStack = false,
	) {
		assertInterval(intervalMillis);
		if (this.stopped || !this.store.matchesScope(this.scope)) return () => {};
		const runtime = this.lanes[lane];
		const first = runtime.claims.size === 0;
		const claimId = ++this.nextClaimId;
		runtime.claims.set(claimId, {
			intervalMillis,
			consumerId,
			includeDynamicStack,
		});
		this.recordClaims();
		if (first) {
			runtime.generation++;
			this.store.setLoading(lane, this.store.captureScope());
			if (!this.transport.openStream) this.scheduleRefresh(lane);
		}
		if (this.transport.openStream) this.syncStream();
		else this.restartTimer(lane);
		let active = true;
		return () => {
			if (!active) return;
			active = false;
			this.release(lane, claimId);
		};
	}

	/**
	 * One-shot authoritative read through the scoped transport without claiming a
	 * polling lane. The caller consumes the returned snapshot directly; the shared
	 * lane projections are not touched.
	 */
	read(
		lane: VisualizationRuntimeLane,
		options?: { dynamicStackOnly?: boolean; fixtureIds?: readonly string[] },
	): Promise<VisualizationSnapshot> {
		if (this.stopped || !this.store.matchesScope(this.scope))
			return Promise.reject(
				new Error("The visualization runtime view is unavailable"),
			);
		return this.transport.loadSnapshot(this.scope, lane, options);
	}

	stop() {
		if (this.stopped) return;
		this.stopped = true;
		this.lifecycle++;
		this.stream?.close();
		this.stream = null;
		for (const lane of lanes()) {
			const runtime = this.lanes[lane];
			runtime.claims.clear();
			runtime.generation++;
			this.clearTimer(runtime);
		}
		this.recordClaims();
	}

	private release(lane: VisualizationRuntimeLane, claimId: number) {
		const runtime = this.lanes[lane];
		if (!runtime.claims.delete(claimId)) return;
		this.recordClaims();
		if (runtime.claims.size) {
			if (this.transport.openStream) this.syncStream();
			else this.restartTimer(lane);
			return;
		}
		runtime.generation++;
		this.clearTimer(runtime);
		if (this.store.matchesScope(this.scope))
			this.store.setIdle(lane, this.store.captureScope());
		if (this.transport.openStream) this.syncStream();
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
		runtime.timer = globalThis.setInterval(
			() => void this.refresh(lane),
			interval,
		);
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

	private syncStream() {
		const claimedLanes = lanes().filter(
			(lane) => this.lanes[lane].claims.size > 0,
		);
		if (!claimedLanes.length) {
			this.stream?.updateClaims([], 10, false);
			this.stream?.close();
			this.stream = null;
			return;
		}
		this.stream ??=
			this.transport.openStream?.(this.scope, {
				snapshot: (lane, snapshot) => {
					if (
						this.stopped ||
						!this.lanes[lane].claims.size ||
						!this.store.matchesScope(this.scope)
					)
						return;
					this.store.installStreamed(lane, snapshot, this.store.captureScope());
					this.onError?.(null);
				},
				error: (error) => {
					for (const lane of lanes()) {
						if (this.lanes[lane].claims.size)
							this.store.setError(lane, error, this.store.captureScope());
					}
					this.onError?.(error);
				},
			}) ?? null;
		const fastest = Math.min(
			...claimedLanes.flatMap((lane) => [
				...[...this.lanes[lane].claims.values()].map(
					({ intervalMillis }) => intervalMillis,
				),
			]),
		);
		this.stream?.updateClaims(
			claimedLanes,
			Math.max(1, Math.min(10, Math.ceil(1_000 / fastest))),
			claimedLanes.some((lane) =>
				[...this.lanes[lane].claims.values()].some(
					(claim) => claim.includeDynamicStack,
				),
			),
		);
	}

	private recordClaims() {
		const owners = (lane: VisualizationRuntimeLane) =>
			[...this.lanes[lane].claims.values()].map(({ consumerId }) => consumerId);
		frontendPerformanceDiagnostics.recordStageLaneClaims(
			owners("normal"),
			owners("preload"),
		);
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
	return runtime.claims.size
		? Math.min(
				...[...runtime.claims.values()].map(
					({ intervalMillis }) => intervalMillis,
				),
			)
		: null;
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
