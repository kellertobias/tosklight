import type {
	ProgrammerCaptureModeEventMessage,
	ProgrammerCaptureModeProjection,
	ProgrammerCaptureModeSnapshot,
} from "../features/programmerCaptureMode/contracts";
import type { EventActionSource } from "./generated/light-wire";
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

export function decodeProgrammerCaptureModeSnapshot(
	value: unknown,
	expectedUserId: string,
): ProgrammerCaptureModeSnapshot {
	programmingUuidAt(expectedUserId, "$.requested_user_id");
	const snapshot = recordAt(value, "$");
	return {
		cursor: decodeCursor(snapshot, "$"),
		projection: decodeCaptureModeProjection(
			snapshot.projection,
			"$.projection",
			expectedUserId,
		),
	};
}

export function decodeProgrammerCaptureModeEventMessage(
	value: unknown,
	expectedUserId: string,
): ProgrammerCaptureModeEventMessage {
	programmingUuidAt(expectedUserId, "$.requested_user_id");
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
	return decodeCaptureModeEvent(message, expectedUserId);
}

export function decodeCaptureModeProjection(
	value: unknown,
	path: string,
	expectedUserId: string,
): ProgrammerCaptureModeProjection {
	const projection = exactRecordAt(value, path, [
		"user_id",
		"revision",
		"blind",
		"preview",
		"preload_capture_programmer",
	]);
	const userId = programmingUuidAt(projection.user_id, `${path}.user_id`);
	assertExpectedUser(userId, expectedUserId, `${path}.user_id`);
	return {
		userId,
		revision: integerAt(projection.revision, `${path}.revision`),
		blind: booleanAt(projection.blind, `${path}.blind`),
		preview: booleanAt(projection.preview, `${path}.preview`),
		preloadCaptureProgrammer: booleanAt(
			projection.preload_capture_programmer,
			`${path}.preload_capture_programmer`,
		),
	};
}

function decodeCaptureModeEvent(
	message: Record<string, unknown>,
	expectedUserId: string,
): ProgrammerCaptureModeEventMessage {
	const event = recordAt(message.event, "$.event");
	const sequence = integerAt(event.sequence, "$.event.sequence");
	validateCaptureModeEnvelope(event, expectedUserId);
	const payload = recordAt(event.payload, "$.event.payload");
	enumAt(payload.type, "$.event.payload.type", [
		"programming_capture_mode_changed",
	]);
	const change = recordAt(payload.change, "$.event.payload.change");
	return {
		type: "event",
		sequence,
		correlationId: nullableUuid(event, "correlation_id", "$.event"),
		projection: decodeCaptureModeProjection(
			change.projection,
			"$.event.payload.change.projection",
			expectedUserId,
		),
	};
}

function validateCaptureModeEnvelope(
	event: Record<string, unknown>,
	expectedUserId: string,
) {
	stringAt(event.occurred_at, "$.event.occurred_at");
	if (!("desk_id" in event) || event.desk_id !== null)
		throw new WireValidationError("$.event.desk_id", "null", event.desk_id);
	enumAt(event.class, "$.event.class", ["projection"]);
	enumAt(event.delivery, "$.event.delivery", ["replaceable"]);
	validateCaptureModeObject(event.object, expectedUserId);
	validateNoRelatedObjects(event);
	validateActionSource(event.source);
}

function validateCaptureModeObject(value: unknown, expectedUserId: string) {
	const object = recordAt(value, "$.event.object");
	enumAt(object.capability, "$.event.object.capability", ["programmer"]);
	const expectedObject = `programming-capture-mode:${expectedUserId}`;
	const objectId = stringAt(object.id, "$.event.object.id");
	if (objectId.toLowerCase() !== expectedObject.toLowerCase())
		throw new WireValidationError(
			"$.event.object.id",
			expectedObject,
			objectId,
		);
}

function validateNoRelatedObjects(event: Record<string, unknown>) {
	if (!("related_objects" in event) || event.related_objects == null) return;
	const related = arrayAt(event.related_objects, "$.event.related_objects");
	if (related.length > 0)
		throw new WireValidationError(
			"$.event.related_objects",
			"an empty array",
			related,
		);
}

function validateActionSource(value: unknown) {
	const source = recordAt(value, "$.event.source");
	enumAt(source.kind, "$.event.source.kind", ["action"]);
	enumAt(source.source, "$.event.source.source", ACTION_SOURCES);
}

function decodeGap(
	message: Record<string, unknown>,
): ProgrammerCaptureModeEventMessage {
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

function nullableUuid(
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

function assertExpectedUser(actual: string, expected: string, path: string) {
	if (actual.toLowerCase() !== expected.toLowerCase())
		throw new WireValidationError(path, `requested user ${expected}`, actual);
}
