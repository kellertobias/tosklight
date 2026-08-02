import type { ShowObject } from "../showObjects/contracts";
import type {
	ResolvedSpatialMapping,
	SpatialSelectionMapping,
} from "../spatialMapping/contracts";

export interface GroupPropertiesUpdate {
	name: string;
	color: string | null;
	icon: string | null;
}

/** Exact source authority the operator observed, rejected server-side when it no longer holds. */
export interface GroupSourceExpectation {
	sourceGroupId: string;
	expectedSourceRevision: number | null;
}

export type GroupManagementOperation =
	| { type: "update_properties"; properties: GroupPropertiesUpdate }
	| { type: "undo" }
	| { type: "refresh_frozen"; expectedSource: GroupSourceExpectation | null }
	| { type: "detach_derived"; expectedSource: GroupSourceExpectation | null }
	| { type: "set_spatial_mapping"; mapping: SpatialSelectionMapping }
	| { type: "remove_spatial_mapping" };

export interface GroupManagementRequest {
	requestId: string;
	groupId: string;
	operation: GroupManagementOperation;
	expectedObjectRevision: number;
	expectedShowRevision: number;
}

export interface ManagedGroupProjection {
	id: string;
	revision: number;
	object: ShowObject<"group">;
}

export interface GroupSettingsSnapshot {
	showId: string;
	showRevision: number;
	group: ManagedGroupProjection;
	resolvedSpatial: ResolvedSpatialMapping;
}

interface GroupManagementOutcomeBase {
	requestId: string;
	correlationId: string;
	replayed: boolean;
	showId: string;
	showRevision: number;
	group: ManagedGroupProjection;
	persistenceWarning: string | null;
}

export type GroupManagementOutcome = GroupManagementOutcomeBase &
	(
		| { status: "changed"; eventSequence: number }
		| {
				status: "no_change";
				eventSequence?: never;
		  }
	);

export interface ManageGroupInput {
	objectId: string;
	expectedObjectRevision: number;
	operation: GroupManagementOperation;
}

export interface GroupManagementActions {
	manage(input: ManageGroupInput): Promise<GroupManagementOutcome | null>;
	settings(objectId: string): Promise<GroupSettingsSnapshot | null>;
}

export interface GroupManagementTransport {
	manage(
		showId: string,
		request: GroupManagementRequest,
	): Promise<GroupManagementOutcome>;
	settings(showId: string, objectId: string): Promise<GroupSettingsSnapshot>;
}
