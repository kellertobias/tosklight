import type { PlaybackProjection } from "../playbackRuntime/contracts";
import type { ShowObject } from "../showObjects/contracts";

export type CueRecordTarget =
	| { kind: "pool"; playbackNumber: number }
	| { kind: "selected_playback" }
	| { kind: "page_slot"; page: number; slot: number }
	| { kind: "cue_list"; cueListId: string };

export type CueRecordOperation = "overwrite" | "merge" | "subtract";
export type CueRecordCapturePolicy =
	| "current_capture"
	| "pending_or_active_preload";
export type CueRecordActivationPolicy = "hold" | "go_to_if_normal";
export type CueRecordCapturedSource =
	| "normal"
	| "pending_preload"
	| "active_preload";

export interface CueRecordTiming {
	fadeMillis?: number;
	delayMillis?: number;
}

export interface CueRecordingRequest {
	requestId: string;
	target: CueRecordTarget;
	operation: CueRecordOperation;
	cueNumber?: number;
	timing: CueRecordTiming;
	cueOnly: boolean;
	name?: string;
	capturePolicy: CueRecordCapturePolicy;
	activationPolicy: CueRecordActivationPolicy;
}

export interface CueRecordProjections {
	cueList: ShowObject<"cue_list">;
	playback: ShowObject<"playback"> | null;
	page: ShowObject<"playback_page"> | null;
}

interface CueRecordingOutcomeBase {
	requestId: string;
	correlationId: string;
	replayed: boolean;
	capturedSource: CueRecordCapturedSource;
	showRevision: number;
	recordedCue: { id: string; number: number; deleted: boolean };
	projections: CueRecordProjections;
}

export type CueRecordingOutcome = CueRecordingOutcomeBase &
	(
		| {
				status: "changed";
				showEventSequence: number;
				runtime: {
					projection: PlaybackProjection;
					eventSequence: number;
				} | null;
		  }
		| {
				status: "no_change";
				showEventSequence?: never;
				runtime?: never;
		  }
	);

export type RecordCueInput = Omit<CueRecordingRequest, "requestId">;

export interface CueRecordingActions {
	record(input: RecordCueInput): Promise<CueRecordingOutcome | null>;
}

export interface CueRecordingTransport {
	record(
		showId: string,
		expectedShowRevision: number,
		request: CueRecordingRequest,
	): Promise<CueRecordingOutcome>;
}
