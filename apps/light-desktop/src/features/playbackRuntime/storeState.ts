import type {
	PlaybackDesk,
	PlaybackProjection,
	PlaybackTelemetry,
} from "./contracts";

export interface PlaybackRuntimeState {
	showId: string | null;
	deskId: string | null;
	showRevision: number | null;
	eventSequence: number | null;
	desk: PlaybackDesk | null;
	projections: ReadonlyMap<string, readonly PlaybackProjection[]>;
	/// Desk-lifetime sampled telemetry rows, retained across window mounts.
	telemetry: ReadonlyMap<number, PlaybackTelemetry>;
	pendingKeys: ReadonlySet<string>;
	status: "idle" | "loading" | "ready" | "error";
	error: Error | null;
}
