import type { ShowObject } from "../showObjects/contracts";

export type GroupRecordOperation =
	| "overwrite"
	| "merge"
	| "subtract"
	| "delete";

export interface GroupRecordingRequest {
	requestId: string;
	groupId: string;
	operation: GroupRecordOperation;
	expectedObjectRevision: number;
}

export type RecordedGroupProjection =
	| {
			state: "stored";
			id: string;
			revision: number;
			object: ShowObject<"group">;
	  }
	| {
			state: "deleted";
			id: string;
			revision: number;
			object: null;
	  };

export type RecordedStoredGroupProjection = Extract<
	RecordedGroupProjection,
	{ state: "stored" }
>;

interface GroupRecordingOutcomeBase {
	requestId: string;
	correlationId: string;
	replayed: boolean;
	showRevision: number;
}

export type GroupRecordingOutcome = GroupRecordingOutcomeBase &
	(
		| {
				status: "changed";
				eventSequence: number;
				group: RecordedGroupProjection;
		  }
		| {
				status: "no_change";
				eventSequence?: never;
				group: RecordedStoredGroupProjection;
		  }
	);

export interface RecordGroupInput {
	objectId: string;
	operation: GroupRecordOperation;
	expectedObjectRevision: number;
}

export interface GroupRecordingActions {
	record(input: RecordGroupInput): Promise<GroupRecordingOutcome | null>;
}

export interface GroupRecordingTransport {
	record(
		showId: string,
		request: GroupRecordingRequest,
	): Promise<GroupRecordingOutcome>;
}
