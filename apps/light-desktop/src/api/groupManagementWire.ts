import type {
	GroupManagementOperation,
	GroupManagementOutcome,
	GroupManagementRequest,
	GroupSettingsSnapshot,
	ManagedGroupProjection,
} from "../features/groupManagement/contracts";
import type {
	GroupResolvedSpatialProjection,
	GroupSpatialSelectionMapping,
	GroupManagementErrorKind as WireGroupManagementErrorKind,
	GroupManagementOperation as WireGroupManagementOperation,
	GroupManagementRequest as WireGroupManagementRequest,
} from "./generated/light-wire";
import { decodeRecordedGroupBody } from "./groupRecordingBodyWire";
import {
	arrayAt,
	booleanAt,
	enumAt,
	exactRecordAt,
	integerAt,
	numberAt,
	recordAt,
	stringAt,
} from "./playbackWirePrimitives";
import { WireValidationError } from "./wireValidation";

export const GROUP_MANAGEMENT_ERROR_KINDS = [
	"invalid",
	"unauthorized",
	"forbidden",
	"not_found",
	"conflict",
	"unavailable",
	"internal",
] as const satisfies readonly WireGroupManagementErrorKind[];

export type GroupManagementErrorKind = WireGroupManagementErrorKind;

export interface GroupManagementErrorResponse {
	kind: GroupManagementErrorKind;
	error: string;
	currentRevision: number | null;
	currentRelatedRevision: number | null;
	retryable: boolean;
}

export function encodeGroupManagementRequest(request: GroupManagementRequest) {
	printableAt(request.requestId, "$.requestId", 128, "request ID");
	printableAt(request.groupId, "$.groupId", 256, "Group ID");
	integerAt(request.expectedObjectRevision, "$.expectedObjectRevision");
	integerAt(request.expectedShowRevision, "$.expectedShowRevision");
	return {
		request_id: request.requestId,
		group_id: request.groupId,
		operation: encodeOperation(request.operation),
		expected_object_revision: request.expectedObjectRevision,
		expected_show_revision: request.expectedShowRevision,
	} satisfies WireGroupManagementRequest;
}

function encodeOperation(
	operation: GroupManagementOperation,
): WireGroupManagementOperation {
	if (operation.type === "update_properties") {
		const { name, color, icon } = operation.properties;
		printableAt(name, "$.operation.properties.name", 256, "Group name");
		return {
			type: "update_properties",
			properties: {
				name,
				color: optionalLabel(color, "$.operation.properties.color"),
				icon: optionalLabel(icon, "$.operation.properties.icon"),
			},
		};
	}
	if (operation.type === "undo") return { type: "undo" };
	if (operation.type === "set_spatial_mapping")
		return { type: "set_spatial_mapping", mapping: operation.mapping };
	if (operation.type === "remove_spatial_mapping")
		return { type: "remove_spatial_mapping" };
	const expected_source = operation.expectedSource
		? {
				source_group_id: printableAt(
					operation.expectedSource.sourceGroupId,
					"$.operation.expectedSource.sourceGroupId",
					256,
					"source Group ID",
				),
				expected_source_revision:
					operation.expectedSource.expectedSourceRevision,
			}
		: null;
	return { type: operation.type, expected_source };
}

function optionalLabel(value: string | null, path: string) {
	if (value == null) return null;
	if (new TextEncoder().encode(value).length > 64 || /\p{Cc}/u.test(value))
		invalid(path, "at most 64 printable bytes", value);
	return value;
}

export function decodeGroupManagementOutcome(
	value: unknown,
	expectedRequest: GroupManagementRequest,
): GroupManagementOutcome {
	const response = recordAt(value, "$");
	const status = enumAt(response.status, "$.status", ["changed", "no_change"]);
	const fields = [
		"status",
		"request_id",
		"correlation_id",
		"replayed",
		"show_id",
		"show_revision",
		"group",
	];
	if (status === "changed") fields.push("show_event_sequence");
	if ("persistence_warning" in response) fields.push("persistence_warning");
	exactRecordAt(response, "$", fields);
	const requestId = stringAt(response.request_id, "$.request_id");
	if (requestId !== expectedRequest.requestId)
		invalid("$.request_id", `request ${expectedRequest.requestId}`, requestId);
	const group = decodeProjection(response.group, expectedRequest);
	validateRevision(status, group.revision, expectedRequest);
	const base = {
		requestId,
		correlationId: uuidAt(response.correlation_id, "$.correlation_id"),
		replayed: booleanAt(response.replayed, "$.replayed"),
		showId: uuidAt(response.show_id, "$.show_id"),
		showRevision: integerAt(response.show_revision, "$.show_revision"),
		group,
		persistenceWarning:
			response.persistence_warning == null
				? null
				: stringAt(response.persistence_warning, "$.persistence_warning"),
	};
	if (status === "no_change") return { ...base, status };
	return {
		...base,
		status,
		eventSequence: integerAt(
			response.show_event_sequence,
			"$.show_event_sequence",
		),
	};
}

export function decodeGroupSettingsSnapshot(
	value: unknown,
	expectedGroupId: string,
): GroupSettingsSnapshot {
	const response = exactRecordAt(value, "$", [
		"show_id",
		"show_revision",
		"group",
		"resolved_spatial",
	]);
	return {
		showId: uuidAt(response.show_id, "$.show_id"),
		showRevision: integerAt(response.show_revision, "$.show_revision"),
		group: decodeProjection(response.group, expectedGroupId),
		resolvedSpatial: decodeResolvedSpatial(response.resolved_spatial),
	};
}

export function decodeGroupManagementErrorResponse(
	value: unknown,
): GroupManagementErrorResponse {
	const response = recordAt(value, "$");
	const fields = ["kind", "error", "retryable"];
	if ("current_revision" in response) fields.push("current_revision");
	if ("current_related_revision" in response)
		fields.push("current_related_revision");
	exactRecordAt(response, "$", fields);
	return {
		kind: enumAt(response.kind, "$.kind", GROUP_MANAGEMENT_ERROR_KINDS),
		error: stringAt(response.error, "$.error"),
		currentRevision: optionalInteger(
			response.current_revision,
			"$.current_revision",
		),
		currentRelatedRevision: optionalInteger(
			response.current_related_revision,
			"$.current_related_revision",
		),
		retryable: booleanAt(response.retryable, "$.retryable"),
	};
}

function optionalInteger(value: unknown, path: string) {
	return value == null ? null : integerAt(value, path);
}

function decodeProjection(
	value: unknown,
	expected: GroupManagementRequest | string,
): ManagedGroupProjection {
	const projection = recordAt(value, "$.group");
	exactRecordAt(projection, "$.group", [
		"object_id",
		"object_revision",
		"body",
	]);
	const id = printableAt(
		projection.object_id,
		"$.group.object_id",
		256,
		"Group ID",
	);
	const expectedGroupId =
		typeof expected === "string" ? expected : expected.groupId;
	if (id !== expectedGroupId)
		invalid("$.group.object_id", `Group ID ${expectedGroupId}`, id);
	const revision = integerAt(
		projection.object_revision,
		"$.group.object_revision",
	);
	return {
		id,
		revision,
		object: {
			kind: "group",
			id,
			revision,
			updated_at: "",
			body: decodeRecordedGroupBody(projection.body, id),
		},
	};
}

function decodeResolvedSpatial(value: unknown): GroupResolvedSpatialProjection {
	const spatial = exactRecordAt(value, "$.resolved_spatial", [
		"source_order",
		"effective_mapping",
		"mapping_provenance",
		"ordered_fixture_ids",
		"ranks",
		"rank_count",
		"warnings",
	]);
	return {
		source_order: uuidArray(
			spatial.source_order,
			"$.resolved_spatial.source_order",
		),
		effective_mapping:
			spatial.effective_mapping == null
				? null
				: decodeSpatialMapping(
						spatial.effective_mapping,
						"$.resolved_spatial.effective_mapping",
					),
		mapping_provenance: decodeMappingProvenance(
			spatial.mapping_provenance,
			"$.resolved_spatial.mapping_provenance",
		),
		ordered_fixture_ids: uuidArray(
			spatial.ordered_fixture_ids,
			"$.resolved_spatial.ordered_fixture_ids",
		),
		ranks: arrayAt(spatial.ranks, "$.resolved_spatial.ranks").map(
			(value, index) => {
				const rank = exactRecordAt(
					value,
					`$.resolved_spatial.ranks[${index}]`,
					["fixture_id", "rank"],
				);
				return {
					fixture_id: uuidAt(
						rank.fixture_id,
						`$.resolved_spatial.ranks[${index}].fixture_id`,
					),
					rank: integerAt(rank.rank, `$.resolved_spatial.ranks[${index}].rank`),
				};
			},
		),
		rank_count: integerAt(spatial.rank_count, "$.resolved_spatial.rank_count"),
		warnings: arrayAt(spatial.warnings, "$.resolved_spatial.warnings").map(
			(value, index) => {
				const warning = exactRecordAt(
					value,
					`$.resolved_spatial.warnings[${index}]`,
					["type", "fixture_id"],
				);
				if (warning.type !== "missing_position")
					invalid(
						`$.resolved_spatial.warnings[${index}].type`,
						"missing_position",
						warning.type,
					);
				return {
					type: "missing_position" as const,
					fixture_id: uuidAt(
						warning.fixture_id,
						`$.resolved_spatial.warnings[${index}].fixture_id`,
					),
				};
			},
		),
	};
}

function decodeMappingProvenance(value: unknown, path: string) {
	const provenance = recordAt(value, path);
	const type = enumAt(provenance.type, `${path}.type`, [
		"none",
		"local",
		"inherited",
		"mixed_source_mappings",
	]);
	if (type === "local") {
		exactRecordAt(provenance, path, ["type", "group_id"]);
		return {
			type,
			group_id: printableAt(
				provenance.group_id,
				`${path}.group_id`,
				256,
				"Group ID",
			),
		} as const;
	}
	if (type === "inherited") {
		exactRecordAt(provenance, path, ["type", "source_group_ids"]);
		return {
			type,
			source_group_ids: arrayAt(
				provenance.source_group_ids,
				`${path}.source_group_ids`,
			).map((id, index) =>
				printableAt(id, `${path}.source_group_ids[${index}]`, 256, "Group ID"),
			),
		} as const;
	}
	exactRecordAt(provenance, path, ["type"]);
	return { type } as const;
}

function decodeSpatialMapping(
	value: unknown,
	path: string,
): GroupSpatialSelectionMapping {
	const mapping = exactRecordAt(value, path, ["projection", "shape"]);
	const projection = exactRecordAt(mapping.projection, `${path}.projection`, [
		"anchor",
		"view_direction",
		"rotation_degrees",
		"preset",
	]);
	const preset =
		projection.preset == null
			? null
			: enumAt(projection.preset, `${path}.projection.preset`, [
					"top",
					"front",
					"back",
					"left",
					"right",
				]);
	return {
		projection: {
			anchor: decodePosition(projection.anchor, `${path}.projection.anchor`),
			view_direction: decodePosition(
				projection.view_direction,
				`${path}.projection.view_direction`,
			),
			rotation_degrees: numberAt(
				projection.rotation_degrees,
				`${path}.projection.rotation_degrees`,
			),
			preset,
		},
		shape: decodeShape(mapping.shape, `${path}.shape`),
	};
}

function decodePosition(value: unknown, path: string) {
	const position = exactRecordAt(value, path, ["x", "y", "z"]);
	return {
		x: numberAt(position.x, `${path}.x`),
		y: numberAt(position.y, `${path}.y`),
		z: numberAt(position.z, `${path}.z`),
	};
}

function decodeShape(
	value: unknown,
	path: string,
): GroupSpatialSelectionMapping["shape"] {
	const shape = recordAt(value, path);
	const type = enumAt(shape.type, `${path}.type`, ["grid", "radial", "radar"]);
	if (type === "grid") {
		exactRecordAt(shape, path, ["type", "angle_degrees", "direction"]);
		return {
			type,
			angle_degrees: numberAt(shape.angle_degrees, `${path}.angle_degrees`),
			direction: enumAt(shape.direction, `${path}.direction`, [
				"ascending",
				"descending",
			]),
		};
	}
	const center = {
		center_u: numberAt(shape.center_u, `${path}.center_u`),
		center_v: numberAt(shape.center_v, `${path}.center_v`),
	};
	if (type === "radial") {
		exactRecordAt(shape, path, ["type", "center_u", "center_v", "direction"]);
		return {
			type,
			...center,
			direction: enumAt(shape.direction, `${path}.direction`, [
				"outward",
				"inward",
			]),
		};
	}
	exactRecordAt(shape, path, [
		"type",
		"center_u",
		"center_v",
		"start_angle_degrees",
		"sweep",
	]);
	return {
		type,
		...center,
		start_angle_degrees: numberAt(
			shape.start_angle_degrees,
			`${path}.start_angle_degrees`,
		),
		sweep: enumAt(shape.sweep, `${path}.sweep`, [
			"clockwise",
			"counter_clockwise",
		]),
	};
}

function uuidArray(value: unknown, path: string) {
	return arrayAt(value, path).map((id, index) =>
		uuidAt(id, `${path}[${index}]`),
	);
}

function validateRevision(
	status: "changed" | "no_change",
	revision: number,
	request: GroupManagementRequest,
) {
	const expected =
		status === "changed"
			? request.expectedObjectRevision + 1
			: request.expectedObjectRevision;
	if (!Number.isSafeInteger(expected) || revision !== expected)
		invalid(
			"$.group.object_revision",
			`${status} revision ${expected}`,
			revision,
		);
}

function printableAt(
	value: unknown,
	path: string,
	byteLimit: number,
	label: string,
) {
	if (
		typeof value !== "string" ||
		!value.trim() ||
		new TextEncoder().encode(value).length > byteLimit ||
		/\p{Cc}/u.test(value)
	)
		invalid(path, `1-${byteLimit} printable ${label} bytes`, value);
	return value as string;
}

function uuidAt(value: unknown, path: string) {
	const decoded = stringAt(value, path);
	if (
		!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
			decoded,
		)
	)
		invalid(path, "hyphenated UUID", value);
	return decoded;
}

function invalid(path: string, expected: string, actual: unknown): never {
	throw new WireValidationError(path, expected, actual);
}
