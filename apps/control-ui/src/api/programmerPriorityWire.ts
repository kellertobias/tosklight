import type {
	ProgrammerPriorityActionOutcome,
	ProgrammerPriorityActionRequest,
	ProgrammerPriorityChange,
	ProgrammerPriorityEventMessage,
	ProgrammerPriorityProjection,
	ProgrammerPrioritySnapshot,
} from "../features/programmerPriority/contracts";
import {
	assertPriorityTimestamp,
	assertProgrammerPriority,
} from "../features/programmerPriority/projectionValue";
import type {
	EventActionSource,
	ProgrammerPriorityActionRequest as WirePriorityActionRequest,
} from "./generated/light-wire";
import {
	arrayAt,
	booleanAt,
	enumAt,
	exactRecordAt,
	integerAt,
	recordAt,
	stringAt,
} from "./playbackWirePrimitives";
import { programmingUuidAt } from "./programmingWireProjection";
import { WireValidationError } from "./wireValidation";

export {
	decodeProgrammerPriorityErrorResponse,
	type ProgrammerPriorityErrorResponse,
} from "./programmerPriorityErrorWire";

const ACTION_SOURCES = [
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
] as const satisfies readonly EventActionSource[];

export function encodeProgrammerPriorityActionRequest(
	request: ProgrammerPriorityActionRequest,
): WirePriorityActionRequest {
	printableAt(request.requestId, "$.requestId", 128);
	integerAt(request.expectedRevision, "$.expectedRevision");
	priorityAt(request.priority, "$.priority");
	return {
		request_id: request.requestId,
		expected_revision: request.expectedRevision,
		priority: request.priority,
	};
}

export function decodeProgrammerPrioritySnapshot(
	value: unknown,
	expectedUserId: string,
): ProgrammerPrioritySnapshot {
	validateExpectedUser(expectedUserId);
	const snapshot = exactRecordAt(value, "$", ["cursor", "projection"]);
	return {
		cursor: cursorAt(snapshot.cursor, "$.cursor"),
		projection: projectionAt(
			snapshot.projection,
			"$.projection",
			expectedUserId,
		),
	};
}

export function decodeProgrammerPriorityActionOutcome(
	value: unknown,
	expectedUserId: string,
	request: ProgrammerPriorityActionRequest,
): ProgrammerPriorityActionOutcome {
	validateExpectedUser(expectedUserId);
	const response = recordAt(value, "$"),
		status = enumAt(response.status, "$.status", ["changed", "no_change"]);
	exactRecordAt(response, "$", outcomeFields(response, status));
	const requestId = stringAt(response.request_id, "$.request_id");
	if (requestId !== request.requestId)
		throw mismatch("$.request_id", request.requestId, requestId);
	const projection = projectionAt(
		response.projection,
		"$.projection",
		expectedUserId,
	);
	const expectedRevision =
		status === "changed"
			? request.expectedRevision + 1
			: request.expectedRevision;
	if (projection.revision !== expectedRevision)
		throw mismatch(
			"$.projection.revision",
			expectedRevision,
			projection.revision,
		);
	const base = {
		requestId,
		correlationId: programmingUuidAt(
			response.correlation_id,
			"$.correlation_id",
		),
		projection,
		replayed: booleanAt(response.replayed, "$.replayed"),
		warning: optionalString(response, "warning", "$"),
	};
	return status === "changed"
		? {
				...base,
				status,
				eventSequence: integerAt(response.event_sequence, "$.event_sequence"),
			}
		: { ...base, status, eventSequence: null };
}

export function decodeProgrammerPriorityEventMessage(
	value: unknown,
	expectedUserId: string,
): ProgrammerPriorityEventMessage {
	validateExpectedUser(expectedUserId);
	const message = recordAt(value, "$"),
		type = enumAt(message.type, "$.type", [
			"ready",
			"event",
			"gap",
			"repaired",
			"error",
		]);
	exactRecordAt(message, "$", messageFields(type));
	if (type === "ready" || type === "repaired")
		return { type, cursor: cursorAt(message.cursor, "$.cursor") };
	if (type === "error")
		return { type, error: stringAt(message.error, "$.error") };
	if (type === "gap") return gapAt(message.gap);
	return eventAt(message.event, expectedUserId);
}

function projectionAt(
	value: unknown,
	path: string,
	expectedUserId: string,
): ProgrammerPriorityProjection {
	const projection = exactRecordAt(value, path, [
		"user_id",
		"revision",
		"priority",
		"changed_at",
	]);
	const userId = programmingUuidAt(projection.user_id, `${path}.user_id`);
	assertUser(userId, expectedUserId, `${path}.user_id`);
	return {
		userId,
		revision: integerAt(projection.revision, `${path}.revision`),
		priority: priorityAt(projection.priority, `${path}.priority`),
		changedAt: timestampAt(projection.changed_at, `${path}.changed_at`),
	};
}

function eventAt(value: unknown, expectedUserId: string) {
	const event = recordAt(value, "$.event");
	exactRecordAt(
		event,
		"$.event",
		presentFields(event, [
			"sequence",
			"occurred_at",
			"desk_id",
			"class",
			"object",
			"related_objects",
			"source",
			"correlation_id",
			"delivery",
			"payload",
		]),
	);
	const sequence = integerAt(event.sequence, "$.event.sequence");
	timestampAt(event.occurred_at, "$.event.occurred_at");
	if (!("desk_id" in event) || event.desk_id !== null)
		throw new WireValidationError("$.event.desk_id", "null", event.desk_id);
	enumAt(event.class, "$.event.class", ["projection"]);
	enumAt(event.delivery, "$.event.delivery", ["replaceable"]);
	eventObjectAt(event.object, expectedUserId);
	noRelatedObjects(event.related_objects);
	actionSourceAt(event.source);
	const payload = exactRecordAt(event.payload, "$.event.payload", [
		"type",
		"change",
	]);
	enumAt(payload.type, "$.event.payload.type", ["programmer_priority_changed"]);
	return {
		type: "event" as const,
		sequence,
		correlationId: requiredNullableUuid(event, "correlation_id", "$.event"),
		change: changeAt(payload.change, "$.event.payload.change", expectedUserId),
	};
}

function changeAt(
	value: unknown,
	path: string,
	expectedUserId: string,
): ProgrammerPriorityChange {
	const change = recordAt(value, path),
		type = enumAt(change.type, `${path}.type`, ["upsert", "remove"]);
	if (type === "upsert") {
		exactRecordAt(change, path, ["type", "projection"]);
		return {
			type,
			projection: projectionAt(
				change.projection,
				`${path}.projection`,
				expectedUserId,
			),
		};
	}
	exactRecordAt(change, path, ["type", "user_id", "revision"]);
	const userId = programmingUuidAt(change.user_id, `${path}.user_id`);
	assertUser(userId, expectedUserId, `${path}.user_id`);
	return {
		type,
		userId,
		revision: integerAt(change.revision, `${path}.revision`),
	};
}

function eventObjectAt(value: unknown, expectedUserId: string) {
	const object = exactRecordAt(value, "$.event.object", ["capability", "id"]);
	enumAt(object.capability, "$.event.object.capability", ["programmer"]);
	const expected = `programming-priority:${expectedUserId}`;
	const id = stringAt(object.id, "$.event.object.id");
	if (id.toLowerCase() !== expected.toLowerCase())
		throw mismatch("$.event.object.id", expected, id);
}

function actionSourceAt(value: unknown) {
	const source = exactRecordAt(value, "$.event.source", ["kind", "source"]);
	enumAt(source.kind, "$.event.source.kind", ["action"]);
	enumAt(source.source, "$.event.source.source", ACTION_SOURCES);
}

function noRelatedObjects(value: unknown) {
	if (value == null) return;
	const related = arrayAt(value, "$.event.related_objects");
	if (related.length)
		throw new WireValidationError(
			"$.event.related_objects",
			"an empty array",
			related,
		);
}

function gapAt(value: unknown): ProgrammerPriorityEventMessage {
	const gap = exactRecordAt(value, "$.gap", [
		"after_sequence",
		"oldest_available",
		"latest_sequence",
	]);
	return {
		type: "gap",
		afterSequence: integerAt(gap.after_sequence, "$.gap.after_sequence"),
		oldestAvailable: integerAt(gap.oldest_available, "$.gap.oldest_available"),
		latestSequence: integerAt(gap.latest_sequence, "$.gap.latest_sequence"),
	};
}

function cursorAt(value: unknown, path: string) {
	const cursor = exactRecordAt(value, path, ["sequence"]);
	return integerAt(cursor.sequence, `${path}.sequence`);
}

function priorityAt(value: unknown, path: string) {
	if (typeof value !== "number")
		throw new WireValidationError(path, "signed 16-bit integer", value);
	try {
		assertProgrammerPriority(value);
		return value;
	} catch {
		throw new WireValidationError(path, "signed 16-bit integer", value);
	}
}

function timestampAt(value: unknown, path: string) {
	const timestamp = stringAt(value, path);
	try {
		assertPriorityTimestamp(timestamp);
		return timestamp;
	} catch {
		throw new WireValidationError(path, "RFC 3339 calendar timestamp", value);
	}
}

function validateExpectedUser(userId: string) {
	programmingUuidAt(userId, "$.requested_user_id");
}

function assertUser(actual: string, expected: string, path: string) {
	if (actual.toLowerCase() !== expected.toLowerCase())
		throw mismatch(path, `requested user ${expected}`, actual);
}

function requiredNullableUuid(
	object: Record<string, unknown>,
	key: string,
	path: string,
) {
	if (!(key in object))
		throw new WireValidationError(`${path}.${key}`, "UUID or null", undefined);
	return object[key] == null
		? null
		: programmingUuidAt(object[key], `${path}.${key}`);
}

function optionalString(
	object: Record<string, unknown>,
	key: string,
	path: string,
) {
	return object[key] == null ? null : stringAt(object[key], `${path}.${key}`);
}

function outcomeFields(
	response: Record<string, unknown>,
	status: "changed" | "no_change",
) {
	return presentFields(response, [
		"request_id",
		"correlation_id",
		"projection",
		"status",
		...(status === "changed" ? ["event_sequence"] : []),
		"replayed",
		"warning",
	]);
}

function messageFields(type: ProgrammerPriorityEventMessage["type"]) {
	if (type === "ready" || type === "repaired") return ["type", "cursor"];
	if (type === "error") return ["type", "error"];
	if (type === "gap") return ["type", "gap"];
	return ["type", "event"];
}

function presentFields(
	value: Record<string, unknown>,
	fields: readonly string[],
) {
	return fields.filter((field) => field in value);
}

function printableAt(value: unknown, path: string, byteLimit: number) {
	const text = stringAt(value, path);
	if (
		!text.trim() ||
		new TextEncoder().encode(text).length > byteLimit ||
		/\p{Cc}/u.test(text)
	)
		throw new WireValidationError(
			path,
			`1-${byteLimit} printable bytes`,
			value,
		);
	return text;
}

function mismatch(path: string, expected: unknown, actual: unknown) {
	return new WireValidationError(path, String(expected), actual);
}
