import type {
	CommandLineProjection,
	PendingCommandChoice,
} from "../programmingInteraction/contracts";
import type { ShowObject, ShowObjectBodies } from "../showObjects/contracts";

export type CueTransferMode = "plain" | "status";

export interface CueTransferActionRequest {
	requestId: string;
	choiceId: string;
	mode: CueTransferMode;
	expectedCommandLineRevision: number;
}

export interface CueTransferProjection {
	cueListId: string;
	objectId: string;
	objectRevision: number;
	body: ShowObjectBodies["cue_list"];
}

export interface CueTransferSummary {
	operation: "copy" | "move";
	mode: CueTransferMode;
	sourceCueId: string;
	sourceCueNumber: number;
	destinationCueId: string;
	destinationCueNumber: number;
}

export interface CueTransferActionOutcome {
	requestId: string;
	choiceId: string;
	correlationId: string;
	replayed: boolean;
	showId: string;
	summary: CueTransferSummary;
	showRevision: number;
	projections: CueTransferProjection[];
	showEventSequence: number;
	commandLine: CommandLineProjection;
	interactionEventSequence: number | null;
	persistenceWarning: string | null;
}

export interface CueTransferTransport {
	apply(
		showId: string,
		expectedShowRevision: number,
		request: CueTransferActionRequest,
	): Promise<CueTransferActionOutcome>;
}

/** Exact, on-demand reads used only after a typed revision conflict. */
export interface CueTransferConflictRepair {
	loadCueLists(showId: string): Promise<{
		objects: ShowObject<"cue_list">[];
		showRevision: number;
	}>;
	loadCommandLine(deskId: string): Promise<CommandLineProjection>;
}

export class CueTransferTransportError extends Error {
	constructor(
		message: string,
		readonly status: number,
		readonly currentRevision: number | null,
		readonly currentRelatedRevision: number | null,
		readonly retryable: boolean,
	) {
		super(message);
		this.name = "CueTransferTransportError";
	}
}

export interface CueTransferCapability {
	apply(choice: PendingCommandChoice, mode: CueTransferMode): Promise<boolean>;
}

export interface CueTransferScope {
	showId: string;
	deskId: string;
	userId: string;
}
