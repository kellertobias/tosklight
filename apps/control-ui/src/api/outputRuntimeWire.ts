import type {
	OutputRuntimeActionOutcome,
	OutputRuntimeActionRequest,
	OutputRuntimeChange,
	OutputRuntimeEventMessage,
	OutputRuntimeProjection,
	OutputRuntimeSnapshot,
} from "../features/outputRuntime/contracts";
import {
	assertOutputMutation,
	assertOutputRequestId,
} from "../features/outputRuntime/projectionValue";
import type {
	EventActionSource,
	OutputRuntimeActionRequest as WireOutputActionRequest,
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
import {
	outputGrandMasterAt,
	outputTimestampAt,
} from "./outputRuntimeWireValues";
import { WireValidationError } from "./wireValidation";

export {
	decodeOutputRuntimeErrorResponse,
	type OutputRuntimeErrorResponse,
} from "./outputRuntimeErrorWire";

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

export function encodeOutputRuntimeActionRequest(
	request: OutputRuntimeActionRequest,
): WireOutputActionRequest {
	try {
		assertOutputRequestId(request.requestId);
		assertOutputMutation(request.grandMaster, request.blackout);
	} catch {
		throw new WireValidationError("$", "valid Output runtime action", request);
	}
	programmingUuidAt(request.expectedShowId, "$.expectedShowId");
	integerAt(request.expectedRevision, "$.expectedRevision");
	const encoded: WireOutputActionRequest = {
		request_id: request.requestId,
		expected_show_id: request.expectedShowId,
		expected_revision: request.expectedRevision,
	};
	if (request.grandMaster !== undefined)
		encoded.grand_master = request.grandMaster;
	if (request.blackout !== undefined) encoded.blackout = request.blackout;
	return encoded;
}

export function decodeOutputRuntimeSnapshot(
	value: unknown,
	expectedShowId: string,
): OutputRuntimeSnapshot {
	validateExpectedShow(expectedShowId);
	const snapshot = exactRecordAt(value, "$", ["cursor", "projection"]);
	return {
		cursor: cursorAt(snapshot.cursor, "$.cursor"),
		projection: projectionAt(
			snapshot.projection,
			"$.projection",
			expectedShowId,
		),
	};
}

export function decodeOutputRuntimeActionOutcome(
	value: unknown,
	expectedShowId: string,
	request: OutputRuntimeActionRequest,
): OutputRuntimeActionOutcome {
	validateExpectedShow(expectedShowId);
	assertShow(
		request.expectedShowId,
		expectedShowId,
		"$.request.expected_show_id",
	);
	const response = recordAt(value, "$"),
		status = enumAt(response.status, "$.status", ["changed", "no_change"]);
	exactRecordAt(response, "$", outcomeFields(response, status));
	const requestId = stringAt(response.request_id, "$.request_id");
	if (requestId !== request.requestId)
		throw mismatch("$.request_id", request.requestId, requestId);
	const projection = projectionAt(
		response.projection,
		"$.projection",
		expectedShowId,
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
		durability: enumAt(response.durability, "$.durability", [
			"durable",
			"persistence_pending",
		]),
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

export function decodeOutputRuntimeEventMessage(
	value: unknown,
	expectedShowId: string,
): OutputRuntimeEventMessage {
	validateExpectedShow(expectedShowId);
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
	return eventAt(message.event, expectedShowId);
}

function projectionAt(
	value: unknown,
	path: string,
	expectedShowId: string,
): OutputRuntimeProjection {
	const projection = exactRecordAt(value, path, [
		"scope",
		"identity",
		"revision",
		"grand_master",
		"blackout",
	]);
	const scope = exactRecordAt(projection.scope, `${path}.scope`, ["show_id"]);
	const showId = programmingUuidAt(scope.show_id, `${path}.scope.show_id`);
	assertShow(showId, expectedShowId, `${path}.scope.show_id`);
	return {
		showId,
		identity: enumAt(projection.identity, `${path}.identity`, [
			"global_master",
		]),
		revision: integerAt(projection.revision, `${path}.revision`),
		grandMaster: outputGrandMasterAt(
			projection.grand_master,
			`${path}.grand_master`,
		),
		blackout: booleanAt(projection.blackout, `${path}.blackout`),
	};
}

function eventAt(value: unknown, expectedShowId: string) {
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
	outputTimestampAt(event.occurred_at, "$.event.occurred_at");
	if (!("desk_id" in event) || event.desk_id !== null)
		throw new WireValidationError("$.event.desk_id", "null", event.desk_id);
	enumAt(event.class, "$.event.class", ["projection"]);
	enumAt(event.delivery, "$.event.delivery", ["replaceable"]);
	eventObjectAt(event.object);
	noRelatedObjects(event.related_objects);
	actionSourceAt(event.source);
	const payload = exactRecordAt(event.payload, "$.event.payload", [
		"type",
		"change",
	]);
	enumAt(payload.type, "$.event.payload.type", ["output_runtime_changed"]);
	return {
		type: "event" as const,
		sequence,
		correlationId: requiredNullableUuid(event, "correlation_id", "$.event"),
		change: changeAt(payload.change, "$.event.payload.change", expectedShowId),
	};
}

function changeAt(
	value: unknown,
	path: string,
	expectedShowId: string,
): OutputRuntimeChange {
	const change = exactRecordAt(value, path, ["projection"]);
	return {
		projection: projectionAt(
			change.projection,
			`${path}.projection`,
			expectedShowId,
		),
	};
}

function eventObjectAt(value: unknown) {
	const object = exactRecordAt(value, "$.event.object", ["capability", "id"]);
	enumAt(object.capability, "$.event.object.capability", ["output"]);
	enumAt(object.id, "$.event.object.id", ["runtime:global-master"]);
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

function gapAt(value: unknown): OutputRuntimeEventMessage {
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

function validateExpectedShow(showId: string) {
	programmingUuidAt(showId, "$.requested_show_id");
}

function assertShow(actual: string, expected: string, path: string) {
	if (actual.toLowerCase() !== expected.toLowerCase())
		throw mismatch(path, `requested Show ${expected}`, actual);
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
		"durability",
		"warning",
	]);
}

function messageFields(type: OutputRuntimeEventMessage["type"]) {
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

function mismatch(path: string, expected: unknown, actual: unknown) {
	return new WireValidationError(path, String(expected), actual);
}
