import type { PlaybackProjection } from "../playbackRuntime/contracts";
import type { ProgrammerCaptureModeProjection } from "../programmerCaptureMode/contracts";
import type {
	ProgrammerPreloadPlaybackQueueEntry,
	ProgrammerPreloadPlaybackQueueProjection,
} from "../programmerPreloadPlaybackQueue/contracts";
import type { ProgrammerPreloadValuesProjection } from "../programmerPreloadValues/contracts";

export interface ProgrammerPreloadLifecycleScope {
	showId: string;
	userId: string;
	deskId: string;
}

export type ProgrammerPreloadLifecycleAction =
	| { type: "enter" }
	| {
			type: "go";
			showId: string;
			expectedShowRevision: number;
			expectedPlaybackEventSequence: number;
	  }
	| { type: "clear_pending" }
	| { type: "release" };

export interface ProgrammerPreloadLifecycleRequest {
	requestId: string;
	expectedCaptureModeRevision: number;
	expectedValuesRevision: number;
	expectedQueueRevision: number;
	expectedSelectionRevision: number;
	action: ProgrammerPreloadLifecycleAction;
}

export interface ProgrammerPreloadRuntimeChange {
	projection: PlaybackProjection;
	eventSequence: number;
}

export interface ProgrammerPreloadCommitOutcome {
	showId: string;
	showRevision: number;
	playbackEventSequenceBefore: number;
	playbackEventSequenceAfter: number;
	committedAt: string;
	programmerFadeMillis: number;
	executedPlaybackActions: number;
	executed: readonly ProgrammerPreloadPlaybackQueueEntry[];
	runtimeChanges: readonly ProgrammerPreloadRuntimeChange[];
}

export interface ProgrammerPreloadLifecycleOutcome {
	requestId: string;
	correlationId: string;
	replayed: boolean;
	status: "changed" | "no_change";
	active: boolean;
	captureMode: ProgrammerCaptureModeProjection;
	captureModeEventSequence: number | null;
	valuesRevision: number;
	valuesProjection: ProgrammerPreloadValuesProjection | null;
	valuesEventSequence: number | null;
	queueRevision: number;
	queueProjection: ProgrammerPreloadPlaybackQueueProjection | null;
	queueEventSequence: number | null;
	interactionEventSequence: number | null;
	selectionRevision: number;
	commit: ProgrammerPreloadCommitOutcome | null;
	warning: string | null;
}

export interface ProgrammerPreloadLifecycleTransport {
	applyAction(
		scope: ProgrammerPreloadLifecycleScope,
		request: ProgrammerPreloadLifecycleRequest,
	): Promise<ProgrammerPreloadLifecycleOutcome>;
}

export interface ProgrammerPreloadLifecycleActions {
	enter(requestId?: string): Promise<ProgrammerPreloadLifecycleOutcome | null>;
	go(requestId?: string): Promise<ProgrammerPreloadLifecycleOutcome | null>;
	clearPending(
		requestId?: string,
	): Promise<ProgrammerPreloadLifecycleOutcome | null>;
	release(requestId?: string): Promise<ProgrammerPreloadLifecycleOutcome | null>;
}

export type ProgrammerPreloadLifecycleErrorKind =
	| "invalid"
	| "unauthorized"
	| "forbidden"
	| "not_found"
	| "conflict"
	| "unavailable"
	| "internal";

export class ProgrammerPreloadLifecycleTransportError extends Error {
	constructor(
		message: string,
		readonly kind: ProgrammerPreloadLifecycleErrorKind,
		readonly status: number,
		readonly currentRevision: number | null,
		readonly currentRelatedRevision: number | null,
		readonly retryable: boolean,
	) {
		super(message);
		this.name = "ProgrammerPreloadLifecycleTransportError";
	}
}
