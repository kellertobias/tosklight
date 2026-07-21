import type { PlaybackDesk, PlaybackProjection } from "./contracts";

export interface PlaybackRuntimeState {
	showId: string | null;
	deskId: string | null;
	showRevision: number | null;
	eventSequence: number | null;
	desk: PlaybackDesk | null;
	projections: ReadonlyMap<string, readonly PlaybackProjection[]>;
	pendingKeys: ReadonlySet<string>;
	status: "idle" | "loading" | "ready" | "error";
	error: Error | null;
}
