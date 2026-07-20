import type {
	ProgrammerPreloadValuesActionOutcome,
	ProgrammerPreloadValuesActionRequest,
	ProgrammerPreloadValuesMutation,
	ProgrammerPreloadValuesSnapshot,
	ProgrammerPreloadValueTiming,
} from "../features/programmerPreloadValues/contracts";
import type {
	ProgrammingPreloadValueMutation as WirePreloadValueMutation,
	ProgrammingPreloadValuesAction as WirePreloadValuesAction,
	ProgrammingPreloadValuesActionRequest as WirePreloadValuesActionRequest,
	ProgrammingPreloadValuesErrorKind as WirePreloadValuesErrorKind,
	ProgrammingPreloadValueTiming as WirePreloadValueTiming,
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
	decodeProgrammerPreloadValuesProjection,
	programmerPreloadValuesUuidAt,
} from "./programmerPreloadValuesWireProjection";
import { WireValidationError } from "./wireValidation";

export { decodeProgrammerPreloadValuesEventMessage } from "./programmerPreloadValuesEventWire";

export const PROGRAMMER_PRELOAD_VALUES_ERROR_KINDS = [
	"invalid",
	"unauthorized",
	"forbidden",
	"not_found",
	"conflict",
	"unavailable",
	"internal",
] as const satisfies readonly WirePreloadValuesErrorKind[];

export type ProgrammerPreloadValuesErrorKind = WirePreloadValuesErrorKind;

export interface ProgrammerPreloadValuesErrorResponse {
	kind: ProgrammerPreloadValuesErrorKind;
	error: string;
	currentPreloadRevision: number | null;
	currentCaptureModeRevision: number | null;
	retryable: boolean;
}

export function decodeProgrammerPreloadValuesSnapshot(
	value: unknown,
	expectedUserId: string,
): ProgrammerPreloadValuesSnapshot {
	const snapshot = exactRecordAt(value, "$", ["cursor", "projection"]);
	return {
		cursor: decodeCursor(snapshot, "$"),
		projection: decodeProgrammerPreloadValuesProjection(
			snapshot.projection,
			"$.projection",
			expectedUserId,
		),
	};
}

export function decodeProgrammerPreloadValuesActionOutcome(
	value: unknown,
	expectedUserId: string,
	expectedRequestId?: string,
): ProgrammerPreloadValuesActionOutcome {
	const response = recordAt(value, "$");
	const requestId = stringAt(response.request_id, "$.request_id");
	if (expectedRequestId != null && requestId !== expectedRequestId)
		throw new WireValidationError(
			"$.request_id",
			`request ${expectedRequestId}`,
			requestId,
		);
	const base = decodeOutcomeBase(response, requestId);
	const status = enumAt(response.status, "$.status", ["changed", "no_change"]);
	if (status === "no_change") {
		assertNoProjection(response);
		assertOutcomeFields(response, false);
		return { ...base, status };
	}
	assertOutcomeFields(response, true);
	const projection = decodeProgrammerPreloadValuesProjection(
		response.projection,
		"$.projection",
		expectedUserId,
	);
	if (projection.revision !== base.preloadRevision)
		throw new WireValidationError(
			"$.revision",
			`projection revision ${projection.revision}`,
			base.preloadRevision,
		);
	return {
		...base,
		status,
		projection,
		eventSequence: integerAt(response.event_sequence, "$.event_sequence"),
	};
}

export function decodeProgrammerPreloadValuesErrorResponse(
	value: unknown,
): ProgrammerPreloadValuesErrorResponse {
	const response = exactRecordAt(value, "$", [
		"kind",
		"error",
		"current_revision",
		"current_capture_mode_revision",
		"retryable",
	]);
	return {
		kind: enumAt(
			response.kind,
			"$.kind",
			PROGRAMMER_PRELOAD_VALUES_ERROR_KINDS,
		),
		error: stringAt(response.error, "$.error"),
		currentPreloadRevision: optionalInteger(response, "current_revision", "$"),
		currentCaptureModeRevision: optionalInteger(
			response,
			"current_capture_mode_revision",
			"$",
		),
		retryable: booleanAt(response.retryable, "$.retryable"),
	};
}

export function encodeProgrammerPreloadValuesActionRequest(
	request: ProgrammerPreloadValuesActionRequest,
): WirePreloadValuesActionRequest {
	validateRequest(request);
	return {
		request_id: request.requestId,
		expected_revision: request.expectedPreloadRevision,
		expected_capture_mode_revision: request.expectedCaptureModeRevision,
		action: encodeAction(request.action),
	};
}

function decodeOutcomeBase(
	response: Record<string, unknown>,
	requestId: string,
) {
	return {
		requestId,
		correlationId: programmerPreloadValuesUuidAt(
			response.correlation_id,
			"$.correlation_id",
		),
		preloadRevision: integerAt(response.revision, "$.revision"),
		captureModeRevision: integerAt(
			response.capture_mode_revision,
			"$.capture_mode_revision",
		),
		replayed: booleanAt(response.replayed, "$.replayed"),
		warning: optionalString(response, "warning", "$"),
	};
}

function decodeCursor(message: Record<string, unknown>, path: string) {
	const cursor = exactRecordAt(message.cursor, `${path}.cursor`, ["sequence"]);
	return integerAt(cursor.sequence, `${path}.cursor.sequence`);
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

function assertNoProjection(response: Record<string, unknown>) {
	if ("projection" in response || "event_sequence" in response)
		throw new WireValidationError(
			"$",
			"no projection or event sequence for no_change",
			response,
		);
}

function optionalString(
	object: Record<string, unknown>,
	key: string,
	path: string,
) {
	return object[key] == null ? null : stringAt(object[key], `${path}.${key}`);
}

function optionalInteger(
	object: Record<string, unknown>,
	key: string,
	path: string,
) {
	return object[key] == null ? null : integerAt(object[key], `${path}.${key}`);
}

function validateRequest(request: ProgrammerPreloadValuesActionRequest) {
	if (!request.requestId || request.requestId.length > 128)
		throw new WireValidationError(
			"$.requestId",
			"1-128 character string",
			request.requestId,
		);
	integerAt(request.expectedPreloadRevision, "$.expectedPreloadRevision");
	integerAt(
		request.expectedCaptureModeRevision,
		"$.expectedCaptureModeRevision",
	);
}

function encodeAction(
	action: ProgrammerPreloadValuesActionRequest["action"],
): WirePreloadValuesAction {
	if (action.action === "batch")
		return {
			type: action.action,
			mutations: action.mutations.map(encodeMutation),
		};
	return encodeMutation(action);
}

function encodeMutation(
	mutation: ProgrammerPreloadValuesMutation,
): WirePreloadValueMutation {
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
	timing: ProgrammerPreloadValueTiming,
): WirePreloadValueTiming {
	return {
		fade: timing.fade,
		fade_millis: timing.fadeMillis,
		delay_millis: timing.delayMillis,
	};
}
