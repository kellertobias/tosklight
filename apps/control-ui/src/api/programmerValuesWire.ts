import type {
	ProgrammerValuesActionOutcome,
	ProgrammerValuesActionRequest,
	ProgrammerValuesMutation,
	ProgrammerValuesSnapshot,
	ProgrammerValueTiming,
} from "../features/programmerValues/contracts";
import type {
	ProgrammingValueMutation as WireProgrammingValueMutation,
	ProgrammingValuesAction as WireProgrammingValuesAction,
	ProgrammingValuesActionRequest as WireProgrammingValuesActionRequest,
	ProgrammingValuesErrorKind as WireProgrammingValuesErrorKind,
	ProgrammingValueTiming as WireProgrammingValueTiming,
} from "./generated/light-wire";
import {
	booleanAt,
	enumAt,
	exactRecordAt,
	integerAt,
	recordAt,
	stringAt,
} from "./playbackWirePrimitives";
import {
	decodeProgrammerValuesProjection,
	programmerValuesUuidAt,
} from "./programmerValuesWireProjection";
import { WireValidationError } from "./wireValidation";

export { decodeProgrammerValuesEventMessage } from "./programmerValuesEventWire";

export const PROGRAMMER_VALUES_ERROR_KINDS = [
	"invalid",
	"unauthorized",
	"forbidden",
	"not_found",
	"conflict",
	"unavailable",
	"internal",
] as const satisfies readonly WireProgrammingValuesErrorKind[];

export type ProgrammerValuesErrorKind = WireProgrammingValuesErrorKind;

export interface ProgrammerValuesErrorResponse {
	kind: ProgrammerValuesErrorKind;
	error: string;
	currentRevision: number | null;
	currentCaptureModeRevision: number | null;
	retryable: boolean;
}

export function decodeProgrammerValuesSnapshot(
	value: unknown,
	expectedUserId: string,
): ProgrammerValuesSnapshot {
	const snapshot = exactRecordAt(value, "$", ["cursor", "projection"]);
	return {
		cursor: decodeCursor(snapshot, "$"),
		projection: decodeProgrammerValuesProjection(
			snapshot.projection,
			"$.projection",
			expectedUserId,
		),
	};
}

export function decodeProgrammerValuesActionOutcome(
	value: unknown,
	expectedUserId: string,
	expectedRequestId?: string,
): ProgrammerValuesActionOutcome {
	const response = recordAt(value, "$");
	const requestId = stringAt(response.request_id, "$.request_id");
	if (expectedRequestId != null && requestId !== expectedRequestId)
		throw new WireValidationError(
			"$.request_id",
			`request ${expectedRequestId}`,
			requestId,
		);
	const base = {
		requestId,
		correlationId: programmerValuesUuidAt(
			response.correlation_id,
			"$.correlation_id",
		),
		revision: integerAt(response.revision, "$.revision"),
		captureModeRevision: integerAt(
			response.capture_mode_revision,
			"$.capture_mode_revision",
		),
		replayed: booleanAt(response.replayed, "$.replayed"),
		warning: optionalString(response, "warning", "$"),
	};
	const status = enumAt(response.status, "$.status", ["changed", "no_change"]);
	if (status === "no_change") {
		assertNoProjection(response);
		assertOutcomeFields(response, false);
		return { ...base, status };
	}
	assertOutcomeFields(response, true);
	const projection = decodeProgrammerValuesProjection(
		response.projection,
		"$.projection",
		expectedUserId,
	);
	if (projection.revision !== base.revision)
		throw new WireValidationError(
			"$.revision",
			`projection revision ${projection.revision}`,
			base.revision,
		);
	return {
		...base,
		status,
		projection,
		eventSequence: integerAt(response.event_sequence, "$.event_sequence"),
	};
}

export function decodeProgrammerValuesErrorResponse(
	value: unknown,
): ProgrammerValuesErrorResponse {
	const response = exactRecordAt(value, "$", [
		"kind",
		"error",
		"current_revision",
		"current_capture_mode_revision",
		"retryable",
	]);
	return {
		kind: enumAt(response.kind, "$.kind", PROGRAMMER_VALUES_ERROR_KINDS),
		error: stringAt(response.error, "$.error"),
		currentRevision:
			response.current_revision == null
				? null
				: integerAt(response.current_revision, "$.current_revision"),
		currentCaptureModeRevision:
			response.current_capture_mode_revision == null
				? null
				: integerAt(
						response.current_capture_mode_revision,
						"$.current_capture_mode_revision",
					),
		retryable: booleanAt(response.retryable, "$.retryable"),
	};
}

export function encodeProgrammerValuesActionRequest(
	request: ProgrammerValuesActionRequest,
): WireProgrammingValuesActionRequest {
	validateRequest(request);
	return {
		request_id: request.requestId,
		expected_revision: request.expectedRevision,
		expected_capture_mode_revision: request.expectedCaptureModeRevision,
		action: encodeAction(request.action),
	};
}

function decodeCursor(message: Record<string, unknown>, path: string) {
	const cursor = exactRecordAt(message.cursor, `${path}.cursor`, ["sequence"]);
	return integerAt(
		cursor.sequence,
		`${path}.cursor.sequence`,
	);
}

function assertOutcomeFields(
	response: Record<string, unknown>,
	changed: boolean,
) {
	const fields = [
		"request_id",
		"correlation_id",
		"revision",
		"capture_mode_revision",
		"status",
		"replayed",
		"warning",
	];
	if (changed) fields.push("projection", "event_sequence");
	exactRecordAt(response, "$", fields);
}

function optionalString(
	object: Record<string, unknown>,
	key: string,
	path: string,
) {
	return object[key] == null ? null : stringAt(object[key], `${path}.${key}`);
}

function assertNoProjection(response: Record<string, unknown>) {
	if ("projection" in response || "event_sequence" in response)
		throw new WireValidationError(
			"$",
			"no projection or event sequence for no_change",
			response,
		);
}

function validateRequest(request: ProgrammerValuesActionRequest) {
	if (!request.requestId || request.requestId.length > 128)
		throw new WireValidationError(
			"$.requestId",
			"1-128 character string",
			request.requestId,
		);
	integerAt(request.expectedRevision, "$.expectedRevision");
	integerAt(
		request.expectedCaptureModeRevision,
		"$.expectedCaptureModeRevision",
	);
}

function encodeAction(
	action: ProgrammerValuesActionRequest["action"],
): WireProgrammingValuesAction {
	if (action.action === "batch")
		return {
			type: action.action,
			mutations: action.mutations.map(encodeMutation),
		};
	if (action.action === "clear") return { type: action.action };
	return encodeMutation(action);
}

function encodeMutation(
	mutation: ProgrammerValuesMutation,
): WireProgrammingValueMutation {
	if (mutation.action === "set_fixture")
		return {
			type: mutation.action,
			fixture_id: mutation.fixtureId,
			attribute: mutation.attribute,
			value: mutation.value,
			timing: encodeTiming(mutation.timing),
		};
	if (mutation.action === "release_fixture")
		return {
			type: mutation.action,
			fixture_id: mutation.fixtureId,
			attribute: mutation.attribute,
		};
	if (mutation.action === "set_group")
		return {
			type: mutation.action,
			group_id: mutation.groupId,
			attribute: mutation.attribute,
			value: mutation.value,
			timing: encodeTiming(mutation.timing),
		};
	return {
		type: mutation.action,
		group_id: mutation.groupId,
		attribute: mutation.attribute,
	};
}

function encodeTiming(
	timing: ProgrammerValueTiming,
): WireProgrammingValueTiming {
	return {
		fade: timing.fade,
		fade_millis: timing.fadeMillis,
		delay_millis: timing.delayMillis,
	};
}
