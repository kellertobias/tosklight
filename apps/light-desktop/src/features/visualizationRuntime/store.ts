import type { VisualizationSnapshot } from "../../api/types";
import type {
	VisualizationRuntimeLane,
	VisualizationRuntimeLaneState,
	VisualizationRuntimeScope,
	VisualizationRuntimeState,
} from "./contracts";
import { VisualizationRuntimeProtocolError } from "./transport";

export class VisualizationRuntimeStore {
	private readonly listeners = new Set<() => void>();
	private scopeGeneration = 0;
	private state = emptyState();

	readonly subscribe = (listener: () => void) => {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	};

	readonly getSnapshot = () => this.state;

	reset(scope: VisualizationRuntimeScope | null) {
		if (sameScope(this.state.scope, scope)) return;
		this.scopeGeneration++;
		this.state = { scope, normal: emptyLane(), preload: emptyLane() };
		this.emit();
	}

	setLoading(lane: VisualizationRuntimeLane, expectedScope = this.scopeGeneration) {
		return this.updateLane(
			lane,
			(current) => ({ ...current, status: "loading", error: null }),
			expectedScope,
		);
	}

	setIdle(lane: VisualizationRuntimeLane, expectedScope = this.scopeGeneration) {
		return this.updateLane(
			lane,
			(current) => ({ ...current, status: "idle", error: null }),
			expectedScope,
		);
	}

	install(
		lane: VisualizationRuntimeLane,
		snapshot: VisualizationSnapshot,
		expectedScope = this.scopeGeneration,
	) {
		if (!this.isScopeCurrent(expectedScope)) return false;
		assertLane(snapshot, lane);
		return this.updateLane(
			lane,
			() => ({ status: "ready", snapshot, error: null }),
			expectedScope,
		);
	}

	setError(
		lane: VisualizationRuntimeLane,
		error: Error,
		expectedScope = this.scopeGeneration,
	) {
		return this.updateLane(
			lane,
			(current) => ({
				...current,
				status: current.snapshot ? "ready" : "error",
				error,
			}),
			expectedScope,
		);
	}

	captureScope() {
		return this.scopeGeneration;
	}

	isScopeCurrent(scope: number) {
		return scope === this.scopeGeneration;
	}

	matchesScope(scope: VisualizationRuntimeScope) {
		return sameScope(this.state.scope, scope);
	}

	private updateLane(
		lane: VisualizationRuntimeLane,
		update: (
			current: VisualizationRuntimeLaneState,
		) => VisualizationRuntimeLaneState,
		expectedScope: number,
	) {
		if (!this.isScopeCurrent(expectedScope)) return false;
		const next = update(this.state[lane]);
		this.state = { ...this.state, [lane]: next };
		this.emit();
		return true;
	}

	private emit() {
		for (const listener of this.listeners) listener();
	}
}

function assertLane(
	snapshot: VisualizationSnapshot,
	lane: VisualizationRuntimeLane,
) {
	const expected = lane === "preload";
	if (snapshot.preload !== expected)
		throw new VisualizationRuntimeProtocolError(
			`Visualization snapshot lane mismatch: expected ${lane}`,
		);
}

function sameScope(
	left: VisualizationRuntimeScope | null,
	right: VisualizationRuntimeScope | null,
) {
	return (
		left?.showId === right?.showId &&
		left?.sessionId === right?.sessionId &&
		left?.authorityKey === right?.authorityKey
	);
}

function emptyLane(): VisualizationRuntimeLaneState {
	return { status: "idle", snapshot: null, error: null };
}

function emptyState(): VisualizationRuntimeState {
	return { scope: null, normal: emptyLane(), preload: emptyLane() };
}
