import type { VisualizationSnapshot } from "../../api/types";

export type VisualizationRuntimeLane = "normal" | "preload";

/** One authenticated server/session authority for the currently active Show. */
export interface VisualizationRuntimeScope {
	showId: string;
	sessionId: string;
	authorityKey: string;
}

export type VisualizationRuntimeStatus =
	| "idle"
	| "loading"
	| "ready"
	| "error";

export interface VisualizationRuntimeLaneState {
	status: VisualizationRuntimeStatus;
	snapshot: VisualizationSnapshot | null;
	error: Error | null;
}

export interface VisualizationRuntimeState {
	scope: VisualizationRuntimeScope | null;
	normal: VisualizationRuntimeLaneState;
	preload: VisualizationRuntimeLaneState;
}

export interface VisualizationRuntimeView {
	status: VisualizationRuntimeStatus;
	snapshot: VisualizationSnapshot | null;
	error: Error | null;
	ready: boolean;
}
