import type { ProgrammerValuesEventMessage } from "../features/programmerValues/contracts";
import {
	arrayAt,
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
		return decodeCursorMessage(message, type);
	if (type === "error") return decodeErrorMessage(message);
	if (type === "gap") return decodeGap(message);
	exactRecordAt(message, "$", ["type", "event"]);
	return decodeValuesEvent(recordAt(message.event, "$.event"), expectedUserId);
}

function decodeCursorMessage(
	message: Record<string, unknown>,
	type: "ready" | "repaired",
): ProgrammerValuesEventMessage {
	exactRecordAt(message, "$", ["type", "cursor"]);
	return { type, cursor: decodeCursor(message, "$") };
}

function decodeErrorMessage(
	message: Record<string, unknown>,
): ProgrammerValuesEventMessage {
	exactRecordAt(message, "$", ["type", "error"]);
	return { type: "error", error: stringAt(message.error, "$.error") };
}

function decodeValuesEvent(
	event: Record<string, unknown>,
	expectedUserId: string,
): ProgrammerValuesEventMessage {
	exactRecordAt(event, "$.event", [
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
	]);
	const sequence = integerAt(event.sequence, "$.event.sequence");
	validateValuesEnvelope(event, expectedUserId);
	const change = decodeChange(event.payload);
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

function decodeChange(value: unknown) {
	const payload = exactRecordAt(value, "$.event.payload", ["type", "change"]);
	enumAt(payload.type, "$.event.payload.type", ["programming_values_changed"]);
	return exactRecordAt(payload.change, "$.event.payload.change", [
		"projection",
	]);
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
	validateObject(event.object, expectedUserId);
	assertNoRelatedObjects(event);
	validateSource(event.source);
}

function validateObject(value: unknown, expectedUserId: string) {
	const object = exactRecordAt(value, "$.event.object", ["capability", "id"]);
	enumAt(object.capability, "$.event.object.capability", ["programmer"]);
	const expectedObject = `programming-values:${expectedUserId}`;
	const objectId = stringAt(object.id, "$.event.object.id");
	if (objectId.toLowerCase() !== expectedObject.toLowerCase())
		throw new WireValidationError(
			"$.event.object.id",
			expectedObject,
			objectId,
		);
}

function validateSource(value: unknown) {
	const source = exactRecordAt(value, "$.event.source", ["kind", "source"]);
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
	exactRecordAt(message, "$", ["type", "gap"]);
	const gap = exactRecordAt(message.gap, "$.gap", [
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

function decodeCursor(message: Record<string, unknown>, path: string) {
	const cursor = exactRecordAt(message.cursor, `${path}.cursor`, ["sequence"]);
	return integerAt(cursor.sequence, `${path}.cursor.sequence`);
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
