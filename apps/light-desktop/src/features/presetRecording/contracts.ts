import type { PresetAddress } from "../../presetFamilies";
import type { ShowObject } from "../showObjects/contracts";

export type PresetRecordingMode = "merge" | "overwrite";

export interface PresetRecordingRequest {
	requestId: string;
	address: PresetAddress;
	name: string;
	mode: PresetRecordingMode;
	expectedObjectRevision: number;
}

interface PresetRecordingOutcomeBase {
	requestId: string;
	correlationId: string;
	replayed: boolean;
	showRevision: number;
	preset: ShowObject<"preset">;
}

export type PresetRecordingOutcome = PresetRecordingOutcomeBase &
	(
		| { status: "changed"; eventSequence: number }
		| { status: "no_change"; eventSequence?: never }
	);

export interface RecordPresetInput {
	objectId: string;
	address: PresetAddress;
	name: string;
	mode: PresetRecordingMode;
	expectedObjectRevision: number;
}

export interface PresetRecordingActions {
	record(input: RecordPresetInput): Promise<PresetRecordingOutcome | null>;
}

export interface PresetRecordingTransport {
	record(
		showId: string,
		request: PresetRecordingRequest,
	): Promise<PresetRecordingOutcome>;
}
