import type { PresetAddress } from "../../presetFamilies";
import type { ProgrammerValuesProjection } from "../programmerValues/contracts";
import type { ShowObject } from "../showObjects/contracts";

export interface PresetRecallScope {
	showId: string;
	userId: string;
	deskId: string;
}

export interface PresetRecallRequest {
	requestId: string;
	presetId: string;
	address: PresetAddress;
	expectedPresetRevision: number;
	expectedShowRevision: number;
	expectedProgrammerRevision: number;
	expectedCaptureModeRevision: number;
	expectedSelectionRevision: number;
	selectedFixtureCount: number;
}

interface PresetRecallOutcomeBase {
	requestId: string;
	correlationId: string;
	replayed: boolean;
	showRevision: number;
	programmerRevision: number;
	captureModeRevision: number;
	selectionRevision: number;
	interactionEventSequence: number | null;
	appliedFixtures: number;
	activeContext: string;
	preset: ShowObject<"preset">;
	warning: string | null;
}

export type PresetRecallOutcome = PresetRecallOutcomeBase &
	(
		| {
				status: "changed";
				projection: ProgrammerValuesProjection | null;
				eventSequence: number | null;
		  }
		| {
				status: "no_change";
				projection: null;
				eventSequence: null;
		  }
	);

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
