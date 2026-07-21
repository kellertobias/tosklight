import {
	type SpeedGroupAction,
	type SpeedGroupActionOutcome,
	type SpeedGroupActionRequest,
	type SpeedGroupAuthorityProjection,
	type SpeedGroupChange,
	type SpeedGroupEventMessage,
	type SpeedGroupId,
	type SpeedGroupProjection,
	type SpeedGroupSnapshot,
	speedGroupIds,
} from "../features/speedGroupRuntime/contracts";
import type { EventActionSource } from "./generated/light-wire";
import { outputTimestampAt } from "./outputRuntimeWireValues";
import {
	arrayAt,
	booleanAt,
	enumAt,
	exactRecordAt,
	integerAt,
	numberAt,
	recordAt,
	stringAt,
} from "./playbackWirePrimitives";
import { programmingUuidAt } from "./programmingWireProjection";
import {
	messageFields,
	outcomeFields,
	presentFields,
} from "./speedGroupRuntimeWireFields";
import { WireValidationError } from "./wireValidation";

export { encodeSpeedGroupActionRequest } from "./speedGroupRuntimeActionWire";
export {
	decodeSpeedGroupErrorResponse,
	type SpeedGroupErrorResponse,
} from "./speedGroupRuntimeErrorWire";

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

export function decodeSpeedGroupSnapshot(value: unknown): SpeedGroupSnapshot {
	const snapshot = exactRecordAt(value, "$", ["cursor", "projection"]);
	return {
		cursor: cursorAt(snapshot.cursor, "$.cursor"),
		projection: authorityAt(snapshot.projection, "$.projection"),
	};
}

export function decodeSpeedGroupActionOutcome(
	value: unknown,
	request: SpeedGroupActionRequest,
): SpeedGroupActionOutcome {
	const response = recordAt(value, "$"),
		status = enumAt(response.status, "$.status", ["changed", "no_change"]);
	exactRecordAt(response, "$", outcomeFields(response, status));
	const requestId = stringAt(response.request_id, "$.request_id");
	if (requestId !== request.requestId)
		throw mismatch("$.request_id", request.requestId, requestId);
	const authorityId = programmingUuidAt(
		response.authority_id,
		"$.authority_id",
	);
	if (!sameId(authorityId, request.expectedAuthorityId))
		throw mismatch("$.authority_id", request.expectedAuthorityId, authorityId);
	const revision = integerAt(response.revision, "$.revision");
	const expectedRevision =
		status === "changed"
			? request.expectedRevision + 1
			: request.expectedRevision;
	if (revision !== expectedRevision)
		throw mismatch("$.revision", expectedRevision, revision);
	const groups = groupsAt(response.groups, "$.groups");
	assertOutcomeGroups(groups, request.action);
	const base = {
		requestId,
		correlationId: programmingUuidAt(
			response.correlation_id,
			"$.correlation_id",
		),
		authorityId,
		revision,
		appliedAtMillis: integerAt(
			response.applied_at_millis,
			"$.applied_at_millis",
		),
		groups,
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

export function decodeSpeedGroupEventMessage(
	value: unknown,
): SpeedGroupEventMessage {
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
	return eventAt(message.event);
}

function authorityAt(
	value: unknown,
	path: string,
): SpeedGroupAuthorityProjection {
	const projection = exactRecordAt(value, path, [
		"authority_id",
		"revision",
		"groups",
	]);
	const groups = groupsAt(projection.groups, `${path}.groups`);
	if (
		groups.length !== speedGroupIds.length ||
		groups.some((group, index) => group.group !== speedGroupIds[index])
	)
		throw new WireValidationError(
			`${path}.groups`,
			"exactly A, B, C, D, E in order",
			projection.groups,
		);
	return {
		authorityId: programmingUuidAt(
			projection.authority_id,
			`${path}.authority_id`,
		),
		revision: integerAt(projection.revision, `${path}.revision`),
		groups,
	};
}

function groupsAt(value: unknown, path: string) {
	const seen = new Set<SpeedGroupId>();
	return arrayAt(value, path).map((group, index) => {
		const decoded = groupAt(group, `${path}[${index}]`);
		if (seen.has(decoded.group))
			throw new WireValidationError(
				`${path}[${index}].group`,
				"unique Speed Group",
				decoded.group,
			);
		seen.add(decoded.group);
		return decoded;
	});
}

function groupAt(value: unknown, path: string): SpeedGroupProjection {
	const group = exactRecordAt(
		value,
		path,
		presentFields(recordAt(value, path), [
			"group",
			"manual_bpm",
			"paused",
			"speed_master_scale",
			"synchronized_with",
			"phase_origin_millis",
		]),
	);
	const scale = numberAt(
		group.speed_master_scale,
		`${path}.speed_master_scale`,
	);
	if (scale < 0 || scale > 4)
		throw new WireValidationError(
			`${path}.speed_master_scale`,
			"number from 0 through 4",
			scale,
		);
	const synchronizedWith = optionalGroup(group, "synchronized_with", path);
	const decoded = {
		group: enumAt(group.group, `${path}.group`, speedGroupIds),
		manualBpm: boundedBpm(group.manual_bpm, `${path}.manual_bpm`),
		paused: booleanAt(group.paused, `${path}.paused`),
		speedMasterScale: scale,
		synchronizedWith,
		phaseOriginMillis: integerAt(
			group.phase_origin_millis,
			`${path}.phase_origin_millis`,
		),
	};
	if (decoded.synchronizedWith === decoded.group)
		throw new WireValidationError(
			`${path}.synchronized_with`,
			"another Speed Group or null",
			decoded.synchronizedWith,
		);
	return decoded;
}

function eventAt(value: unknown): SpeedGroupEventMessage {
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
	enumAt(event.delivery, "$.event.delivery", ["lossless"]);
	eventObjectAt(event.object);
	noRelatedObjects(event.related_objects);
	actionSourceAt(event.source);
	const payload = exactRecordAt(event.payload, "$.event.payload", [
		"type",
		"change",
	]);
	enumAt(payload.type, "$.event.payload.type", ["speed_groups_changed"]);
	return {
		type: "event",
		sequence,
		correlationId: requiredNullableUuid(event, "correlation_id", "$.event"),
		change: changeAt(payload.change, "$.event.payload.change"),
	};
}

function changeAt(value: unknown, path: string): SpeedGroupChange {
	const change = exactRecordAt(value, path, [
		"authority_id",
		"revision",
		"applied_at_millis",
		"groups",
	]);
	const groups = groupsAt(change.groups, `${path}.groups`);
	if (groups.length === 0)
		throw new WireValidationError(`${path}.groups`, "non-empty array", groups);
	return {
		authorityId: programmingUuidAt(change.authority_id, `${path}.authority_id`),
		revision: integerAt(change.revision, `${path}.revision`),
		appliedAtMillis: integerAt(
			change.applied_at_millis,
			`${path}.applied_at_millis`,
		),
		groups,
	};
}

function assertOutcomeGroups(
	groups: readonly SpeedGroupProjection[],
	action: SpeedGroupAction,
) {
	const expected =
		action.type === "synchronize"
			? [action.source, action.target].sort()
			: [action.group];
	if (
		groups.length !== expected.length ||
		groups.some((group, index) => group.group !== expected[index])
	)
		throw new WireValidationError(
			"$.groups",
			`authoritative groups ${expected.join(", ")}`,
			groups,
		);
}

function eventObjectAt(value: unknown) {
	const object = exactRecordAt(value, "$.event.object", ["capability", "id"]);
	enumAt(object.capability, "$.event.object.capability", ["playback"]);
	enumAt(object.id, "$.event.object.id", ["speed-groups:manual"]);
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

function gapAt(value: unknown): SpeedGroupEventMessage {
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

function boundedBpm(value: unknown, path: string) {
	const bpm = numberAt(value, path);
	if (bpm < 0.1 || bpm > 999)
		throw new WireValidationError(path, "number from 0.1 through 999", value);
	return bpm;
}

function optionalGroup(
	object: Record<string, unknown>,
	key: string,
	path: string,
) {
	return object[key] == null
		? null
		: enumAt(object[key], `${path}.${key}`, speedGroupIds);
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

function sameId(left: string, right: string) {
	return left.toLowerCase() === right.toLowerCase();
}

function mismatch(path: string, expected: unknown, actual: unknown) {
	return new WireValidationError(path, String(expected), actual);
}
