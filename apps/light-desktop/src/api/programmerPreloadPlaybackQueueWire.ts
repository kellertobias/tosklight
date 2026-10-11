import { decodeExactAddress } from "./playbackWireProjection";
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
const ACTION_SOURCES = [
	"user_interface",
	"keyboard",
	"osc",
	"http",
	"extension",
	"matter",
	"cue",
	"timecode",
	"scheduler",
	"macro",
	"system",
] as const;

export function decodeProgrammerPreloadPlaybackQueueSnapshot(
	value: unknown,
): ProgrammerPreloadPlaybackQueueSnapshot {
	const snapshot = exactRecordAt(value, "$", ["cursor", "projection"]);
	return {
		cursor: decodeCursor(snapshot.cursor, "$.cursor"),
		projection: decodeProgrammerPreloadPlaybackQueueProjection(
			snapshot.projection,
			"$.projection",
		),
	};
}

export function decodeProgrammerPreloadPlaybackQueueEventMessage(
	value: unknown,
): ProgrammerPreloadPlaybackQueueEventMessage {
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
	return decodeEvent(message.event);
}

export function decodeProgrammerPreloadPlaybackQueueProjection(
	value: unknown,
	path: string,
): ProgrammerPreloadPlaybackQueueProjection {
	const projection = exactRecordAt(value, path, ["revision", "actions"]);
	return {
		revision: integerAt(projection.revision, `${path}.revision`),
		actions: arrayAt(projection.actions, `${path}.actions`).map(
			(entry, index) => decodePreloadQueueEntry(entry, `${path}.actions[${index}]`),
		),
	};
}

export function decodePreloadQueueEntry(value: unknown, path: string) {
	const entry = exactRecordAt(value, path, ["address", "action", "control_source"]);
	const source = exactRecordAt(entry.control_source, `${path}.control_source`, ["surface_id", "control_id"]);
	const controlId = integerAt(source.control_id, `${path}.control_source.control_id`);
	if (controlId > 0xffffffff) throw new WireValidationError(`${path}.control_source.control_id`, "32-bit control ID", controlId);
	return {
		address: decodeExactAddress(entry.address, `${path}.address`),
		action: enumAt(entry.action, `${path}.action`, ACTIONS),
		controlSource: {surface_id: stringAt(source.surface_id, `${path}.control_source.surface_id`), control_id: controlId},
	};
}

function decodeEvent(
	value: unknown,
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
	validateEnvelope(event);
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
		),
	};
}

function validateEnvelope(event: Record<string, unknown>) {
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
	validateObject(event.object);
	validateRelatedObjects(event.related_objects);
	validateSource(event.source);
}

function validateObject(value: unknown) {
	const object = exactRecordAt(value, "$.event.object", ["capability", "id"]);
	enumAt(object.capability, "$.event.object.capability", ["programmer"]);
	const expected = "programming-preload-playback-queue";
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
