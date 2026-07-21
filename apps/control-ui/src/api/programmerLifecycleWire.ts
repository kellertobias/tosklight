import type {
	ProgrammerLifecycleChange,
	ProgrammerLifecycleEventMessage,
	ProgrammerLifecycleProjection,
	ProgrammerLifecycleRow,
	ProgrammerLifecycleSnapshot,
} from "../features/programmerLifecycle/contracts";
import type { EventActionSource } from "./generated/light-wire";
import {
	arrayAt,
	booleanAt,
	enumAt,
	exactRecordAt,
	integerAt,
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

export function decodeProgrammerLifecycleSnapshot(
	value: unknown,
): ProgrammerLifecycleSnapshot {
	const snapshot = exactRecordAt(value, "$", ["cursor", "projection"]);
	return {
		cursor: decodeCursor(snapshot.cursor, "$.cursor"),
		projection: decodeProjection(snapshot.projection, "$.projection"),
	};
}

export function decodeProgrammerLifecycleEventMessage(
	value: unknown,
): ProgrammerLifecycleEventMessage {
	const message = exactRecordAt(value, "$", messageFields(value));
	const type = enumAt(message.type, "$.type", [
		"ready",
		"event",
		"gap",
		"repaired",
		"error",
	]);
	if (type === "ready" || type === "repaired")
		return { type, cursor: decodeCursor(message.cursor, "$.cursor") };
	if (type === "error")
		return { type, error: stringAt(message.error, "$.error") };
	if (type === "gap") return decodeGap(message.gap);
	return decodeEvent(message.event);
}

function decodeProjection(
	value: unknown,
	path: string,
): ProgrammerLifecycleProjection {
	const projection = exactRecordAt(value, path, ["revision", "programmers"]);
	return {
		revision: integerAt(projection.revision, `${path}.revision`),
		programmers: arrayAt(projection.programmers, `${path}.programmers`).map(
			(programmer, index) =>
				decodeProgrammer(programmer, `${path}.programmers[${index}]`),
		),
	};
}

function decodeProgrammer(
	value: unknown,
	path: string,
): ProgrammerLifecycleRow {
	const row = exactRecordAt(value, path, [
		"programmer_id",
		"user_id",
		"connected",
		"selected_fixture_count",
		"normal_value_count",
		"preload_active",
		"sessions",
	]);
	return {
		programmerId: programmingUuidAt(row.programmer_id, `${path}.programmer_id`),
		userId: programmingUuidAt(row.user_id, `${path}.user_id`),
		connected: booleanAt(row.connected, `${path}.connected`),
		selectedFixtureCount: integerAt(
			row.selected_fixture_count,
			`${path}.selected_fixture_count`,
		),
		normalValueCount: integerAt(
			row.normal_value_count,
			`${path}.normal_value_count`,
		),
		preloadActive: booleanAt(row.preload_active, `${path}.preload_active`),
		sessions: arrayAt(row.sessions, `${path}.sessions`).map((session, index) =>
			decodeSession(session, `${path}.sessions[${index}]`),
		),
	};
}

function decodeSession(value: unknown, path: string) {
	const session = exactRecordAt(value, path, ["session_id"]);
	return {
		sessionId: programmingUuidAt(session.session_id, `${path}.session_id`),
	};
}

function decodeEvent(value: unknown): ProgrammerLifecycleEventMessage {
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
		"programming_lifecycle_changed",
	]);
	return {
		type: "event",
		sequence,
		correlationId: nullableUuid(event.correlation_id, "$.event.correlation_id"),
		change: decodeChange(payload.change, "$.event.payload.change"),
	};
}

function decodeChange(value: unknown, path: string): ProgrammerLifecycleChange {
	const change = exactRecordAt(value, path, ["revision", "delta"]);
	const deltaPath = `${path}.delta`;
	const tagged = exactRecordAt(valueAt(change, "delta"), deltaPath, [
		"type",
		"programmer",
		"programmer_id",
	]);
	const type = enumAt(tagged.type, `${deltaPath}.type`, ["upsert", "remove"]);
	if (type === "upsert") {
		assertAbsent(tagged, "programmer_id", deltaPath);
		return {
			revision: integerAt(change.revision, `${path}.revision`),
			delta: {
				type,
				programmer: decodeProgrammer(
					tagged.programmer,
					`${deltaPath}.programmer`,
				),
			},
		};
	}
	assertAbsent(tagged, "programmer", deltaPath);
	return {
		revision: integerAt(change.revision, `${path}.revision`),
		delta: {
			type,
			programmerId: programmingUuidAt(
				tagged.programmer_id,
				`${deltaPath}.programmer_id`,
			),
		},
	};
}

function validateEnvelope(event: Record<string, unknown>) {
	stringAt(event.occurred_at, "$.event.occurred_at");
	if (event.desk_id !== null)
		throw new WireValidationError("$.event.desk_id", "null", event.desk_id);
	if (!("correlation_id" in event))
		throw new WireValidationError(
			"$.event.correlation_id",
			"UUID or null",
			undefined,
		);
	enumAt(event.class, "$.event.class", ["projection"]);
	enumAt(event.delivery, "$.event.delivery", ["lossless"]);
	validateObject(event.object);
	validateRelatedObjects(event.related_objects);
	validateSource(event.source);
}

function validateObject(value: unknown) {
	const object = exactRecordAt(value, "$.event.object", ["capability", "id"]);
	enumAt(object.capability, "$.event.object.capability", ["programmer"]);
	const id = stringAt(object.id, "$.event.object.id");
	if (id !== "programming-lifecycle")
		throw new WireValidationError(
			"$.event.object.id",
			"programming-lifecycle",
			id,
		);
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
	const kind = enumAt(source.kind, "$.event.source.kind", [
		"runtime",
		"action",
	]);
	if (kind === "runtime") {
		assertAbsent(source, "source", "$.event.source");
		return;
	}
	enumAt(source.source, "$.event.source.source", ACTION_SOURCES);
}

function decodeGap(value: unknown): ProgrammerLifecycleEventMessage {
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

function decodeCursor(value: unknown, path: string) {
	const cursor = exactRecordAt(value, path, ["sequence"]);
	return integerAt(cursor.sequence, `${path}.sequence`);
}

function nullableUuid(value: unknown, path: string) {
	return value == null ? null : programmingUuidAt(value, path);
}

function messageFields(value: unknown) {
	const record = exactRecordAt(value, "$", [
		"type",
		"cursor",
		"event",
		"gap",
		"error",
	]);
	const type = enumAt(record.type, "$.type", [
		"ready",
		"event",
		"gap",
		"repaired",
		"error",
	]);
	if (type === "ready" || type === "repaired") return ["type", "cursor"];
	if (type === "event") return ["type", "event"];
	if (type === "gap") return ["type", "gap"];
	return ["type", "error"];
}

function assertAbsent(
	object: Record<string, unknown>,
	key: string,
	path: string,
) {
	if (key in object)
		throw new WireValidationError(`${path}.${key}`, "absent", object[key]);
}

function valueAt(object: Record<string, unknown>, key: string) {
	return object[key];
}
