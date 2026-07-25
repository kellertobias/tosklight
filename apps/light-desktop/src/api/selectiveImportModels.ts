export interface SelectiveImportObjectKey {
	kind: string;
	id: string;
}

export type SelectiveImportConflictResolution =
	| "keep_destination"
	| "replace_destination"
	| "duplicate";

export type SelectiveImportProfileConflictResolution =
	| "keep_destination"
	| "duplicate";

export type SelectiveImportObjectAction =
	| "import_preserving_id"
	| "skip_identical"
	| "keep_destination"
	| "replace_destination"
	| "duplicate"
	| "blocked_conflict";

export type SelectiveImportProfileAction =
	| "copy"
	| "skip_identical"
	| "keep_destination"
	| "duplicate"
	| "blocked_conflict"
	| "missing";

export const SELECTIVE_IMPORT_BLOCKER_TYPES = [
	"empty_selection",
	"same_show",
	"unsupported_object",
	"missing_object",
	"object_conflict",
	"invalid_resolution",
	"invalid_profile_resolution",
	"invalid_descriptor",
	"missing_profile",
	"profile_conflict",
	"missing_managed_asset",
	"managed_asset_conflict",
	"reference_rewrite",
	"candidate_invalid",
] as const;

export type SelectiveImportBlockerType = typeof SELECTIVE_IMPORT_BLOCKER_TYPES[number];

export interface SelectiveImportProfileKey {
	profileId: string;
	revision: number;
}

export interface SelectiveImportSelection {
	selectedObjects: SelectiveImportObjectKey[];
	conflictResolutions: Array<{
		key: SelectiveImportObjectKey;
		resolution: SelectiveImportConflictResolution;
	}>;
	profileConflictResolutions: Array<{
		key: SelectiveImportProfileKey;
		resolution: SelectiveImportProfileConflictResolution;
	}>;
}

export interface SelectiveImportApplyRequest extends SelectiveImportSelection {
	requestId: string;
	expectedSourceRevision: number;
	expectedTargetRevision: number;
}

export interface SelectiveImportCatalog {
	sourceShowId: string;
	sourceShowName: string;
	sourceRevision: number;
	objects: Array<{
		key: SelectiveImportObjectKey;
		objectRevision: number;
		displayName: string;
	}>;
}

export interface SelectiveImportPreview {
	sourceShowId: string;
	targetShowId: string;
	sourceRevision: number;
	targetRevision: number;
	objects: Array<{
		source: SelectiveImportObjectKey;
		destination: SelectiveImportObjectKey;
		action: SelectiveImportObjectAction;
	}>;
	dependencies: Array<{
		owner: SelectiveImportObjectKey;
		dependency: SelectiveImportObjectKey;
		disposition: "selected" | "included" | "bound_to_destination" | "missing";
	}>;
	conflicts: Array<{
		key: SelectiveImportObjectKey;
		resolution: SelectiveImportConflictResolution | null;
	}>;
	profiles: Array<{
		source: SelectiveImportProfileKey;
		destination: SelectiveImportProfileKey;
		action: SelectiveImportProfileAction;
	}>;
	managedAssets: Array<{
		asset: { assetId: string; revision: number };
		action: "copy" | "skip_identical" | "missing" | "blocked_conflict";
	}>;
	blockers: Array<{ type: SelectiveImportBlockerType; summary: string }>;
	canApply: boolean;
}

export interface SelectiveImportOutcome {
	requestId: string;
	correlationId: string;
	changed: boolean;
	showId: string;
	showRevision: number;
	eventSequence: number | null;
	objectChanges: Array<{
		key: SelectiveImportObjectKey;
		objectRevision: number;
		body: unknown;
	}>;
	outcomes: SelectiveImportPreview["objects"];
	profileChanges: Array<{
		source: SelectiveImportProfileKey;
		destination: SelectiveImportProfileKey;
		digest: string;
	}>;
	managedAssets: Array<{ assetId: string; revision: number }>;
}
