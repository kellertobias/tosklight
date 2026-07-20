import type {
	GroupRecordingOutcome,
	GroupRecordingRequest,
	RecordedGroupProjection,
} from "../features/groupRecording/contracts";
import type {
	GroupRecordErrorKind as WireGroupRecordErrorKind,
	GroupRecordRequest as WireGroupRecordRequest,
} from "./generated/light-wire";
import {
	booleanAt,
	enumAt,
	exactRecordAt,
	integerAt,
	recordAt,
	stringAt,
} from "./playbackWirePrimitives";
import { WireValidationError } from "./wireValidation";
import { decodeRecordedGroupBody } from "./groupRecordingBodyWire";

export const GROUP_RECORD_ERROR_KINDS = [
	"invalid",
	"unauthorized",
	"forbidden",
	"not_found",
	"conflict",
	"unavailable",
	"internal",
] as const satisfies readonly WireGroupRecordErrorKind[];

export type GroupRecordErrorKind = WireGroupRecordErrorKind;

export interface GroupRecordErrorResponse {
	kind: GroupRecordErrorKind;
	error: string;
	currentRevision: number | null;
	retryable: boolean;
}

export function encodeGroupRecordingRequest(request: GroupRecordingRequest) {
	validateRequest(request);
	return {
		request_id: request.requestId,
		group_id: request.groupId,
		operation: request.operation,
		expected_object_revision: request.expectedObjectRevision,
	} satisfies WireGroupRecordRequest;
}

export function decodeGroupRecordingOutcome(
	value: unknown,
	expectedRequest: GroupRecordingRequest,
): GroupRecordingOutcome {
	const response = recordAt(value, "$");
	const status = enumAt(response.status, "$.status", ["changed", "no_change"]);
	const fields = [
		"request_id",
		"correlation_id",
		"replayed",
		"status",
		"show_revision",
		"group",
	];
	if (status === "changed") fields.push("event_sequence");
	exactRecordAt(response, "$", fields);
	const requestId = stringAt(response.request_id, "$.request_id");
	if (requestId !== expectedRequest.requestId)
		invalid("$.request_id", `request ${expectedRequest.requestId}`, requestId);
	const group = decodeProjection(response.group, expectedRequest, status);
	validateRevision(status, group.revision, expectedRequest);
	const base = {
		requestId,
		correlationId: uuidAt(response.correlation_id, "$.correlation_id"),
		replayed: booleanAt(response.replayed, "$.replayed"),
		showRevision: integerAt(response.show_revision, "$.show_revision"),
		group,
	};
	if (status === "changed")
		return {
			...base,
			status,
			eventSequence: integerAt(response.event_sequence, "$.event_sequence"),
		};
	if (group.state !== "stored")
		invalid("$.group.state", "stored for no_change", group.state);
	return { ...base, group, status };
}

export function decodeGroupRecordErrorResponse(
	value: unknown,
): GroupRecordErrorResponse {
	const response = recordAt(value, "$");
	const fields = ["kind", "error", "retryable"];
	if ("current_revision" in response) fields.push("current_revision");
	exactRecordAt(response, "$", fields);
	return {
		kind: enumAt(response.kind, "$.kind", GROUP_RECORD_ERROR_KINDS),
		error: stringAt(response.error, "$.error"),
		currentRevision:
			response.current_revision == null
				? null
				: integerAt(response.current_revision, "$.current_revision"),
		retryable: booleanAt(response.retryable, "$.retryable"),
	};
}

function decodeProjection(
	value: unknown,
	request: GroupRecordingRequest,
	status: "changed" | "no_change",
): RecordedGroupProjection {
	const projection = recordAt(value, "$.group");
	const state = enumAt(projection.state, "$.group.state", [
		"stored",
		"deleted",
	]);
	const fields =
		state === "stored"
			? ["state", "id", "revision", "body"]
			: ["state", "id", "revision"];
	exactRecordAt(projection, "$.group", fields);
	const id = printableAt(projection.id, "$.group.id", 256, "Group ID");
	if (id !== request.groupId)
		invalid("$.group.id", `Group ID ${request.groupId}`, id);
	const revision = integerAt(projection.revision, "$.group.revision");
	validateProjectionState(status, state, request.operation);
	if (state === "deleted") return { state, id, revision, object: null };
	const object = {
		kind: "group",
		id,
		revision,
		updated_at: "",
		body: decodeRecordedGroupBody(projection.body, id),
	} as const;
	return { state, id, revision, object };
}

function validateRequest(request: GroupRecordingRequest) {
	printableAt(request.requestId, "$.requestId", 128, "request ID");
	printableAt(request.groupId, "$.groupId", 256, "Group ID");
	enumAt(request.operation, "$.operation", [
		"overwrite",
		"merge",
		"subtract",
		"delete",
	]);
	integerAt(request.expectedObjectRevision, "$.expectedObjectRevision");
}

function validateRevision(
	status: "changed" | "no_change",
	revision: number,
	request: GroupRecordingRequest,
) {
	const expected =
		status === "changed"
			? request.expectedObjectRevision + 1
			: request.expectedObjectRevision;
	if (!Number.isSafeInteger(expected) || revision !== expected)
		invalid("$.group.revision", `${status} revision ${expected}`, revision);
}

function validateProjectionState(
	status: "changed" | "no_change",
	state: "stored" | "deleted",
	operation: GroupRecordingRequest["operation"],
) {
	if (status === "no_change") {
		if (state !== "stored")
			invalid("$.group.state", "stored for no_change", state);
		if (operation === "delete")
			invalid("$.status", "changed for delete", status);
		return;
	}
	if (operation === "subtract") return;
	const expected = operation === "delete" ? "deleted" : "stored";
	if (state !== expected)
		invalid("$.group.state", `${expected} for ${operation}`, state);
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
	return value;
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
