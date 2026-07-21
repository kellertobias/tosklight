import type {
	ProgrammerPreloadPlaybackQueueEventMessage,
	ProgrammerPreloadPlaybackQueueProjection,
	ProgrammerPreloadPlaybackQueueSnapshot,
} from "../features/programmerPreloadPlaybackQueue/contracts";
import {
	arrayAt,
	enumAt,
	exactRecordAt,
	integerAt,
	recordAt,
	stringAt,
} from "./playbackWirePrimitives";
import { programmerPreloadValuesUuidAt } from "./programmerPreloadValuesWireProjection";
import { WireValidationError } from "./wireValidation";

const ACTIONS = [
	"toggle",
	"go",
	"back",
	"off",
	"on",
	"temporary_on",
	"temporary_off",
] as const;
const SURFACES = ["physical", "virtual", "osc", "matter"] as const;
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
] as const;

export function decodeProgrammerPreloadPlaybackQueueSnapshot(
	value: unknown,
	expectedUserId: string,
): ProgrammerPreloadPlaybackQueueSnapshot {
	const snapshot = exactRecordAt(value, "$", ["cursor", "projection"]);
	return {
		cursor: decodeCursor(snapshot.cursor, "$.cursor"),
		projection: decodeProgrammerPreloadPlaybackQueueProjection(
			snapshot.projection,
			"$.projection",
			expectedUserId,
		),
	};
}

export function decodeProgrammerPreloadPlaybackQueueEventMessage(
	value: unknown,
	expectedUserId: string,
): ProgrammerPreloadPlaybackQueueEventMessage {
	programmerPreloadValuesUuidAt(expectedUserId, "$.requested_user_id");
	const message = recordAt(value, "$");
	const type = enumAt(message.type, "$.type", [
		"ready",
		"event",
		"gap",
		"repaired",
		"error",
	]);
	if (type === "ready" || type === "repaired") {
		exactRecordAt(message, "$", ["type", "cursor"]);
		return { type, cursor: decodeCursor(message.cursor, "$.cursor") };
	}
	if (type === "error") {
		exactRecordAt(message, "$", ["type", "error"]);
		return { type, error: stringAt(message.error, "$.error") };
	}
	if (type === "gap") return decodeGap(message);
	exactRecordAt(message, "$", ["type", "event"]);
	return decodeEvent(message.event, expectedUserId);
}

export function decodeProgrammerPreloadPlaybackQueueProjection(
	value: unknown,
	path: string,
	expectedUserId: string,
): ProgrammerPreloadPlaybackQueueProjection {
	const projection = exactRecordAt(value, path, [
		"user_id",
		"revision",
		"actions",
	]);
	const userId = programmerPreloadValuesUuidAt(
		projection.user_id,
		`${path}.user_id`,
	);
	if (userId.toLowerCase() !== expectedUserId.toLowerCase())
		throw new WireValidationError(
			`${path}.user_id`,
			`requested user ${expectedUserId}`,
			userId,
		);
	return {
		userId,
		revision: integerAt(projection.revision, `${path}.revision`),
		actions: arrayAt(projection.actions, `${path}.actions`).map(
			(entry, index) => decodeEntry(entry, `${path}.actions[${index}]`),
		),
	};
}

function decodeEntry(value: unknown, path: string) {
	const entry = exactRecordAt(value, path, [
		"playback_number",
		"page",
		"action",
		"surface",
	]);
	const playbackNumber = integerAt(
		entry.playback_number,
		`${path}.playback_number`,
	);
	if (playbackNumber > 65_535)
		throw new WireValidationError(
			`${path}.playback_number`,
			"16-bit playback number",
			playbackNumber,
		);
	const page = entry.page == null ? null : integerAt(entry.page, `${path}.page`);
	if (page !== null && page > 255)
		throw new WireValidationError(`${path}.page`, "8-bit page", page);
	return {
		playbackNumber,
		page,
		action: enumAt(entry.action, `${path}.action`, ACTIONS),
		surface: enumAt(entry.surface, `${path}.surface`, SURFACES),
	};
}

function decodeEvent(
	value: unknown,
	expectedUserId: string,
): ProgrammerPreloadPlaybackQueueEventMessage {
	const event = exactRecordAt(value, "$.event", [
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
	validateEnvelope(event, expectedUserId);
	const payload = exactRecordAt(event.payload, "$.event.payload", [
		"type",
		"change",
	]);
	enumAt(payload.type, "$.event.payload.type", [
		"programming_preload_playback_queue_changed",
	]);
	const change = exactRecordAt(payload.change, "$.event.payload.change", [
		"projection",
	]);
	return {
		type: "event",
		sequence,
		correlationId: nullableUuid(event.correlation_id, "$.event.correlation_id"),
		projection: decodeProgrammerPreloadPlaybackQueueProjection(
			change.projection,
			"$.event.payload.change.projection",
			expectedUserId,
		),
	};
}

function validateEnvelope(
	event: Record<string, unknown>,
	expectedUserId: string,
) {
	stringAt(event.occurred_at, "$.event.occurred_at");
	if (!("desk_id" in event) || event.desk_id !== null)
		throw new WireValidationError("$.event.desk_id", "null", event.desk_id);
	if (!("correlation_id" in event))
		throw new WireValidationError(
			"$.event.correlation_id",
			"UUID or null",
			undefined,
		);
	enumAt(event.class, "$.event.class", ["projection"]);
	enumAt(event.delivery, "$.event.delivery", ["replaceable"]);
	validateObject(event.object, expectedUserId);
	validateRelatedObjects(event.related_objects);
	validateSource(event.source);
}

function validateObject(value: unknown, expectedUserId: string) {
	const object = exactRecordAt(value, "$.event.object", ["capability", "id"]);
	enumAt(object.capability, "$.event.object.capability", ["programmer"]);
	const expected = `programming-preload-playback-queue:${expectedUserId}`;
	const id = stringAt(object.id, "$.event.object.id");
	if (id.toLowerCase() !== expected.toLowerCase())
		throw new WireValidationError("$.event.object.id", expected, id);
}

function validateRelatedObjects(value: unknown) {
	if (value == null) return;
	const related = arrayAt(value, "$.event.related_objects");
	if (related.length > 0)
		throw new WireValidationError(
			"$.event.related_objects",
			"an empty array",
			related,
		);
}

function validateSource(value: unknown) {
	const source = exactRecordAt(value, "$.event.source", ["kind", "source"]);
	enumAt(source.kind, "$.event.source.kind", ["action"]);
	enumAt(source.source, "$.event.source.source", ACTION_SOURCES);
}

function decodeGap(
	message: Record<string, unknown>,
): ProgrammerPreloadPlaybackQueueEventMessage {
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

function decodeCursor(value: unknown, path: string) {
	const cursor = exactRecordAt(value, path, ["sequence"]);
	return integerAt(cursor.sequence, `${path}.sequence`);
}

function nullableUuid(value: unknown, path: string) {
	return value == null ? null : programmerPreloadValuesUuidAt(value, path);
}
