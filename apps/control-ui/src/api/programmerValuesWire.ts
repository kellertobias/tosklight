import type {
	ProgrammerValuesActionOutcome,
	ProgrammerValuesActionRequest,
	ProgrammerValuesEventMessage,
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
	arrayAt,
	booleanAt,
	enumAt,
	integerAt,
	recordAt,
	stringAt,
} from "./playbackWirePrimitives";
import {
	decodeProgrammerValuesProjection,
	programmerValuesUuidAt,
} from "./programmerValuesWireProjection";
import { WireValidationError } from "./wireValidation";

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
	const snapshot = recordAt(value, "$");
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
		return { ...base, status };
	}
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
	const response = recordAt(value, "$");
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

export function decodeProgrammerValuesEventMessage(
	value: unknown,
	expectedUserId: string,
): ProgrammerValuesEventMessage {
	programmerValuesUuidAt(expectedUserId, "$.requested_user_id");
	const message = recordAt(value, "$");
	const type = enumAt(message.type, "$.type", [
		"ready",
		"event",
		"gap",
		"repaired",
		"error",
	]);
	if (type === "ready" || type === "repaired")
		return { type, cursor: decodeCursor(message, "$") };
	if (type === "error")
		return { type, error: stringAt(message.error, "$.error") };
	if (type === "gap") return decodeGap(message);
	return decodeValuesEvent(recordAt(message.event, "$.event"), expectedUserId);
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

function decodeValuesEvent(
	event: Record<string, unknown>,
	expectedUserId: string,
): ProgrammerValuesEventMessage {
	const sequence = integerAt(event.sequence, "$.event.sequence");
	validateValuesEnvelope(event, expectedUserId);
	const payload = recordAt(event.payload, "$.event.payload");
	enumAt(payload.type, "$.event.payload.type", ["programming_values_changed"]);
	const change = recordAt(payload.change, "$.event.payload.change");
	return {
		type: "event",
		sequence,
		correlationId: optionalUuid(event, "correlation_id", "$.event"),
		projection: decodeProgrammerValuesProjection(
			change.projection,
			"$.event.payload.change.projection",
			expectedUserId,
		),
	};
}

function validateValuesEnvelope(
	event: Record<string, unknown>,
	expectedUserId: string,
) {
	stringAt(event.occurred_at, "$.event.occurred_at");
	if (!("desk_id" in event) || event.desk_id !== null)
		throw new WireValidationError("$.event.desk_id", "null", event.desk_id);
	enumAt(event.class, "$.event.class", ["projection"]);
	enumAt(event.delivery, "$.event.delivery", ["replaceable"]);
	const object = recordAt(event.object, "$.event.object");
	enumAt(object.capability, "$.event.object.capability", ["programmer"]);
	const expectedObject = `programming-values:${expectedUserId}`;
	const objectId = stringAt(object.id, "$.event.object.id");
	if (objectId.toLowerCase() !== expectedObject.toLowerCase())
		throw new WireValidationError(
			"$.event.object.id",
			expectedObject,
			objectId,
		);
	assertNoRelatedObjects(event);
	validateSource(event.source);
}

function validateSource(value: unknown) {
	const source = recordAt(value, "$.event.source");
	enumAt(source.kind, "$.event.source.kind", ["action"]);
	enumAt(source.source, "$.event.source.source", [
		"user_interface",
		"keyboard",
		"osc",
		"http",
		"midi",
		"matter",
		"cue",
		"timecode",
		"scheduler",
		"macro",
		"system",
	]);
}

function assertNoRelatedObjects(event: Record<string, unknown>) {
	if (!("related_objects" in event) || event.related_objects == null) return;
	const related = arrayAt(event.related_objects, "$.event.related_objects");
	if (related.length > 0)
		throw new WireValidationError(
			"$.event.related_objects",
			"an empty array",
			related,
		);
}

function decodeGap(
	message: Record<string, unknown>,
): ProgrammerValuesEventMessage {
	const gap = recordAt(message.gap, "$.gap");
	return {
		type: "gap",
		afterSequence: integerAt(gap.after_sequence, "$.gap.after_sequence"),
		oldestAvailable: integerAt(gap.oldest_available, "$.gap.oldest_available"),
		latestSequence: integerAt(gap.latest_sequence, "$.gap.latest_sequence"),
	};
}

function decodeCursor(message: Record<string, unknown>, path: string) {
	return integerAt(
		recordAt(message.cursor, `${path}.cursor`).sequence,
		`${path}.cursor.sequence`,
	);
}

function optionalUuid(
	object: Record<string, unknown>,
	key: string,
	path: string,
) {
	if (!(key in object))
		throw new WireValidationError(`${path}.${key}`, "UUID or null", undefined);
	return object[key] == null
		? null
		: programmerValuesUuidAt(object[key], `${path}.${key}`);
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
