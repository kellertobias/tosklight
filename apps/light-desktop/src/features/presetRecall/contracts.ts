import type { PresetAddress } from "../../presetFamilies";
import type { ProgrammerPreloadValuesProjection } from "../programmerPreloadValues/contracts";
import type { ProgrammerValuesProjection } from "../programmerValues/contracts";
import type { ShowObject } from "../showObjects/contracts";

export interface PresetRecallScope {
	showId: string;
	sessionId: string;
	deskId: string;
}

export interface PresetRecallRequest {
	/** WebSocket envelope correlation only; never sent as HTTP action identity. */
	requestId: string;
	presetId: string;
	address: PresetAddress;
	expectedPresetRevision: number;
	expectedShowRevision: number;
	expectedProgrammerRevision: number;
	expectedPreloadValuesRevision: number | null;
	expectedCaptureModeRevision: number;
	expectedSelectionRevision: number;
	selectedFixtureCount: number;
}

interface PresetRecallOutcomeBase {
	correlationId: string;
	disposition: "recalled" | "targets_selected";
	showRevision: number;
	programmerRevision: number;
	preloadValuesRevision: number | null;
	captureModeRevision: number;
	selectionRevision: number;
	interactionEventSequence: number | null;
	appliedFixtures: number;
	selectedTargets: number;
	activeContext: string | null;
	preset: ShowObject<"preset">;
	warning: string | null;
}

type PresetRecallChangedOutcome =
	| {
			target: "programmer";
			status: "changed";
			projection: ProgrammerValuesProjection | null;
			eventSequence: number | null;
	  }
	| {
			target: "preload";
			status: "changed";
			projection: ProgrammerPreloadValuesProjection | null;
			eventSequence: number | null;
	  };

type PresetRecallNoChangeOutcome = {
	target: "programmer" | "preload";
	status: "no_change";
	projection: null;
	eventSequence: null;
};

/** Server-authored values authority selected by the captured capture mode. */
export type PresetRecallOutcome = PresetRecallOutcomeBase &
	(PresetRecallChangedOutcome | PresetRecallNoChangeOutcome);

export interface RecallPresetInput {
	objectId: string;
	address: PresetAddress;
}

export interface PresetRecallActions {
	recall(input: RecallPresetInput): Promise<PresetRecallOutcome | null>;
}

export interface PresetRecallTransport {
	recall(
		scope: PresetRecallScope,
		request: PresetRecallRequest,
	): Promise<PresetRecallOutcome>;
}

export type PresetRecallErrorKind =
	| "invalid"
	| "unauthorized"
	| "forbidden"
	| "not_found"
	| "conflict"
	| "unavailable"
	| "internal";

export class PresetRecallTransportError extends Error {
	constructor(
		message: string,
		readonly kind: PresetRecallErrorKind,
		readonly status: number,
		readonly currentRevision: number | null,
		readonly currentRelatedRevision: number | null,
		readonly retryable: boolean,
	) {
		super(message);
		this.name = "PresetRecallTransportError";
	}
}
