import type {
	GroupManagementOperation,
	GroupManagementOutcome,
	GroupManagementRequest,
	ManagedGroupProjection,
} from "../features/groupManagement/contracts";
import type {
	GroupManagementErrorKind as WireGroupManagementErrorKind,
	GroupManagementOperation as WireGroupManagementOperation,
	GroupManagementRequest as WireGroupManagementRequest,
} from "./generated/light-wire";
import { decodeRecordedGroupBody } from "./groupRecordingBodyWire";
import {
	booleanAt,
	enumAt,
	exactRecordAt,
	integerAt,
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
	return {
		request_id: request.requestId,
		group_id: request.groupId,
		operation: encodeOperation(request.operation),
		expected_object_revision: request.expectedObjectRevision,
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
	request: GroupManagementRequest,
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
	if (id !== request.groupId)
		invalid("$.group.object_id", `Group ID ${request.groupId}`, id);
	const revision = integerAt(projection.object_revision, "$.group.object_revision");
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
		invalid("$.group.object_revision", `${status} revision ${expected}`, revision);
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
