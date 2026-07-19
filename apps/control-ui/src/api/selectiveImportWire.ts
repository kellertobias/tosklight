import type {
	SelectiveImportApplyRequest as WireApplyRequest,
	SelectiveImportSelection as WireSelection,
} from "./generated/light-wire";
import type {
	SelectiveImportApplyRequest,
	SelectiveImportCatalog,
	SelectiveImportObjectKey,
	SelectiveImportOutcome,
	SelectiveImportPreview,
	SelectiveImportProfileKey,
	SelectiveImportSelection,
} from "./selectiveImportModels";
import {
	SELECTIVE_IMPORT_BLOCKER_TYPES,
	type SelectiveImportBlockerType,
} from "./selectiveImportModels";

export function selectiveImportSelectionToWire(
	selection: SelectiveImportSelection,
): WireSelection {
	return {
		selected_objects: selection.selectedObjects,
		conflict_resolutions: selection.conflictResolutions,
		profile_conflict_resolutions: selection.profileConflictResolutions.map(
			(choice) => ({
				key: {
					profile_id: choice.key.profileId,
					revision: choice.key.revision,
				},
				resolution: choice.resolution,
			}),
		),
	};
}

export function selectiveImportApplyToWire(
	request: SelectiveImportApplyRequest,
): WireApplyRequest {
	return {
		request_id: request.requestId,
		expected_source_revision: request.expectedSourceRevision,
		expected_target_revision: request.expectedTargetRevision,
		...selectiveImportSelectionToWire(request),
	};
}

export function selectiveImportCatalogFromWire(
	value: unknown,
): SelectiveImportCatalog {
	const body = record(value, "Selective Import catalog");
	return {
		sourceShowId: text(body.source_show_id, "source_show_id"),
		sourceShowName: text(body.source_show_name, "source_show_name"),
		sourceRevision: integer(body.source_revision, "source_revision"),
		objects: array(body.objects, "objects").map((entry, index) => {
			const object = record(entry, `objects[${index}]`);
			return {
				key: objectKey(object.key, `objects[${index}].key`),
				objectRevision: integer(
					object.object_revision,
					`objects[${index}].object_revision`,
				),
				displayName: text(
					object.display_name,
					`objects[${index}].display_name`,
				),
			};
		}),
	};
}

export function selectiveImportPreviewFromWire(
	value: unknown,
): SelectiveImportPreview {
	const body = record(value, "Selective Import preview");
	return {
		sourceShowId: text(body.source_show_id, "source_show_id"),
		targetShowId: text(body.target_show_id, "target_show_id"),
		sourceRevision: integer(body.source_revision, "source_revision"),
		targetRevision: integer(body.target_revision, "target_revision"),
		objects: array(body.objects, "objects").map(mapObjectPreview),
		dependencies: array(body.dependencies, "dependencies").map(mapDependency),
		conflicts: array(body.conflicts, "conflicts").map(mapConflict),
		profiles: array(body.profiles, "profiles").map(mapProfile),
		managedAssets: array(body.managed_assets, "managed_assets").map(mapAsset),
		blockers: array(body.blockers, "blockers").map(mapBlocker),
		canApply: flag(body.can_apply, "can_apply"),
	};
}

export function selectiveImportOutcomeFromWire(
	value: unknown,
): SelectiveImportOutcome {
	const body = record(value, "Selective Import outcome");
	return {
		requestId: text(body.request_id, "request_id"),
		correlationId: text(body.correlation_id, "correlation_id"),
		changed: flag(body.changed, "changed"),
		showId: text(body.show_id, "show_id"),
		showRevision: integer(body.show_revision, "show_revision"),
		eventSequence:
			body.event_sequence === null || body.event_sequence === undefined
				? null
				: integer(body.event_sequence, "event_sequence"),
		objectChanges: array(body.objects, "objects").map((entry, index) => {
			const object = record(entry, `objects[${index}]`);
			return {
				key: objectKey(object.key, `objects[${index}].key`),
				objectRevision: integer(
					object.object_revision,
					`objects[${index}].object_revision`,
				),
				body: object.body,
			};
		}),
		outcomes: array(body.outcomes, "outcomes").map(mapObjectPreview),
		profileChanges: array(body.profiles, "profiles").map((entry, index) => {
			const profile = record(entry, `profiles[${index}]`);
			return {
				source: profileKey(profile.source, `profiles[${index}].source`),
				destination: profileKey(
					profile.destination,
					`profiles[${index}].destination`,
				),
				digest: text(profile.digest, `profiles[${index}].digest`),
			};
		}),
		managedAssets: array(body.managed_assets, "managed_assets").map(
			(entry, index) => {
				const asset = record(entry, `managed_assets[${index}]`);
				return {
					assetId: text(asset.asset_id, `managed_assets[${index}].asset_id`),
					revision: integer(asset.revision, `managed_assets[${index}].revision`),
				};
			},
		),
	};
}

function mapObjectPreview(value: unknown, index: number) {
	const item = record(value, `objects[${index}]`);
	const action = record(item.action, `objects[${index}].action`);
	const actionType = oneOf(
		action.type,
		`objects[${index}].action.type`,
		[
			"import_preserving_id",
			"skip_identical",
			"keep_destination",
			"replace_destination",
			"duplicate",
			"blocked_conflict",
		] as const,
	);
	if (actionType === "duplicate") {
		objectKey(action.destination, `objects[${index}].action.destination`);
	}
	return {
		source: objectKey(item.source, `objects[${index}].source`),
		destination: objectKey(item.destination, `objects[${index}].destination`),
		action: actionType,
	};
}

function mapDependency(value: unknown, index: number) {
	const item = record(value, `dependencies[${index}]`);
	return {
		owner: objectKey(item.owner, `dependencies[${index}].owner`),
		dependency: objectKey(item.dependency, `dependencies[${index}].dependency`),
		disposition: oneOf(item.disposition, `dependencies[${index}].disposition`, [
			"selected",
			"included",
			"bound_to_destination",
			"missing",
		] as const),
	};
}

function mapConflict(value: unknown, index: number) {
	const item = record(value, `conflicts[${index}]`);
	return {
		key: objectKey(item.key, `conflicts[${index}].key`),
		resolution:
			item.resolution === null
				? null
				: oneOf(item.resolution, `conflicts[${index}].resolution`, [
						"keep_destination",
						"replace_destination",
						"duplicate",
					] as const),
	};
}

function mapProfile(value: unknown, index: number) {
	const item = record(value, `profiles[${index}]`);
	const action = record(item.action, `profiles[${index}].action`);
	const actionType = oneOf(action.type, `profiles[${index}].action.type`, [
		"copy",
		"skip_identical",
		"keep_destination",
		"duplicate",
		"blocked_conflict",
		"missing",
	] as const);
	if (actionType === "duplicate") {
		profileKey(action.destination, `profiles[${index}].action.destination`);
	}
	return {
		source: profileKey(item.source, `profiles[${index}].source`),
		destination: profileKey(item.destination, `profiles[${index}].destination`),
		action: actionType,
	};
}

function mapAsset(value: unknown, index: number) {
	const item = record(value, `managed_assets[${index}]`);
	const asset = record(item.asset, `managed_assets[${index}].asset`);
	return {
		asset: {
			assetId: text(asset.asset_id, `managed_assets[${index}].asset.asset_id`),
			revision: integer(asset.revision, `managed_assets[${index}].asset.revision`),
		},
		action: oneOf(item.action, `managed_assets[${index}].action`, [
			"copy",
			"skip_identical",
			"missing",
			"blocked_conflict",
		] as const),
	};
}

function mapBlocker(value: unknown, index: number) {
	const item = record(value, `blockers[${index}]`);
	const type = oneOf(
		item.type,
		`blockers[${index}].type`,
		SELECTIVE_IMPORT_BLOCKER_TYPES,
	);
	return {
		type,
		summary: [label(type), blockerDetail(type, item, index)].filter(Boolean).join(": "),
	};
}

function blockerDetail(
	type: SelectiveImportBlockerType,
	item: Record<string, unknown>,
	index: number,
) {
	const path = `blockers[${index}]`;
	switch (type) {
		case "empty_selection":
		case "same_show":
			return null;
		case "unsupported_object":
		case "object_conflict":
			return objectIdentity(item.key, `${path}.key`);
		case "missing_object":
			objectKeyOrNull(item.required_by, `${path}.required_by`);
			return objectIdentity(item.key, `${path}.key`);
		case "invalid_resolution":
		case "invalid_descriptor":
			objectKey(item.key, `${path}.key`);
			return text(item.message, `${path}.message`);
		case "invalid_profile_resolution":
			profileKey(item.key, `${path}.key`);
			return text(item.message, `${path}.message`);
		case "missing_profile":
			objectKey(item.required_by, `${path}.required_by`);
			return profileIdentity(item.key, `${path}.key`);
		case "profile_conflict":
			return profileIdentity(item.key, `${path}.key`);
		case "missing_managed_asset":
		case "managed_asset_conflict":
			return assetIdentity(item.asset, `${path}.asset`);
		case "reference_rewrite":
			objectKey(item.owner, `${path}.owner`);
			return text(item.message, `${path}.message`);
		case "candidate_invalid":
			return text(item.message, `${path}.message`);
	}
}

function objectKey(value: unknown, name: string): SelectiveImportObjectKey {
	const key = record(value, name);
	return { kind: text(key.kind, `${name}.kind`), id: text(key.id, `${name}.id`) };
}

function objectKeyOrNull(value: unknown, name: string) {
	if (value !== null) objectKey(value, name);
}

function objectIdentity(value: unknown, name: string) {
	const key = objectKey(value, name);
	return `${key.kind}/${key.id}`;
}

function profileKey(value: unknown, name: string): SelectiveImportProfileKey {
	const key = record(value, name);
	return {
		profileId: text(key.profile_id, `${name}.profile_id`),
		revision: integer(key.revision, `${name}.revision`),
	};
}

function profileIdentity(value: unknown, name: string) {
	const key = profileKey(value, name);
	return `${key.profileId} Revision ${key.revision}`;
}

function assetIdentity(value: unknown, name: string) {
	const asset = record(value, name);
	const id = text(asset.asset_id, `${name}.asset_id`);
	const revision = integer(asset.revision, `${name}.revision`);
	return `${id} Revision ${revision}`;
}

function record(value: unknown, name: string): Record<string, unknown> {
	if (!value || typeof value !== "object" || Array.isArray(value)) {
		throw new Error(`${name} is not an object`);
	}
	return value as Record<string, unknown>;
}

function array(value: unknown, name: string): unknown[] {
	if (!Array.isArray(value)) throw new Error(`${name} is not an array`);
	return value;
}

function text(value: unknown, name: string): string {
	if (typeof value !== "string") throw new Error(`${name} is not a string`);
	return value;
}

function integer(value: unknown, name: string): number {
	if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
		throw new Error(`${name} is not a non-negative safe integer`);
	}
	return value;
}

function flag(value: unknown, name: string): boolean {
	if (typeof value !== "boolean") throw new Error(`${name} is not a boolean`);
	return value;
}

function oneOf<const T extends readonly string[]>(
	value: unknown,
	name: string,
	allowed: T,
): T[number] {
	const candidate = text(value, name);
	if (!allowed.includes(candidate)) {
		throw new Error(`${name} has an unsupported value`);
	}
	return candidate as T[number];
}

function label(value: string) {
	return value.replaceAll("_", " ").replace(/\b\w/g, (letter) => letter.toUpperCase());
}
