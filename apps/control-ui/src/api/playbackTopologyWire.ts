import type { PlaybackDefinition } from "./types";
import type {
	PlaybackTopologyAction,
	PlaybackTopologyObject,
	PlaybackTopologyOutcome,
	PlaybackTopologyRequest,
	PlaybackTopologyResolution,
} from "../features/playbackTopology/contracts";
import type { ShowObjectKind } from "../features/showObjects/contracts";
import {
	arrayAt,
	booleanAt,
	enumAt,
	exactRecordAt,
	integerAt,
	recordAt,
	stringAt,
} from "./playbackWirePrimitives";
import { decodeShowObjectBody } from "./showObjectBodyWire";
import { WireValidationError } from "./wireValidation";
import { validatePlaybackTopologyObjects } from "./playbackTopologyOutcomeValidation";

const TOPOLOGY_KINDS = [
	"cue_list",
	"playback",
	"playback_page",
] as const satisfies readonly ShowObjectKind[];
const ERROR_KINDS = [
	"invalid",
	"unauthorized",
	"forbidden",
	"not_found",
	"conflict",
	"unavailable",
	"internal",
] as const;

export type PlaybackTopologyErrorKind = (typeof ERROR_KINDS)[number];

export interface PlaybackTopologyErrorResponse {
	kind: PlaybackTopologyErrorKind;
	error: string;
	currentRevision: number | null;
	currentRelatedRevision: number | null;
	retryable: boolean;
}

export function encodePlaybackTopologyRequest(request: PlaybackTopologyRequest) {
	printableAt(request.requestId, "$.requestId", 128);
	return {
		request_id: request.requestId,
		action: encodeAction(request.action),
	};
}

export function decodePlaybackTopologyOutcome(
	value: unknown,
	request: PlaybackTopologyRequest,
	expectedShowRevision: number,
): PlaybackTopologyOutcome {
	const outcome = recordAt(value, "$");
	const status = enumAt(outcome.status, "$.status", ["changed", "no_change"]);
	exactRecordAt(outcome, "$", outcomeFields(status));
	const requestId = stringAt(outcome.request_id, "$.request_id");
	if (requestId !== request.requestId)
		invalid("$.request_id", `request ${request.requestId}`, requestId);
	const showRevision = integerAt(outcome.show_revision, "$.show_revision");
	const expectedRevision = expectedShowRevision + (status === "changed" ? 1 : 0);
	if (showRevision !== expectedRevision)
		invalid("$.show_revision", String(expectedRevision), showRevision);
	const resolution = decodeResolution(outcome.resolution, request.action);
	const objects = decodeObjects(outcome.objects);
	validatePlaybackTopologyObjects(request.action, resolution, objects, status);
	const common = {
		requestId,
		correlationId: uuidAt(outcome.correlation_id, "$.correlation_id"),
		showRevision,
		resolution,
		objects,
		replayed: booleanAt(outcome.replayed, "$.replayed"),
	};
	return status === "changed"
		? {
				...common,
				status,
				eventSequence: integerAt(outcome.event_sequence, "$.event_sequence"),
			}
		: { ...common, status };
}

export function decodePlaybackTopologyErrorResponse(
	value: unknown,
): PlaybackTopologyErrorResponse {
	const error = recordAt(value, "$");
	const fields = ["kind", "error", "retryable"];
	if ("current_revision" in error) fields.push("current_revision");
	if ("current_related_revision" in error)
		fields.push("current_related_revision");
	exactRecordAt(error, "$", fields);
	return {
		kind: enumAt(error.kind, "$.kind", ERROR_KINDS),
		error: stringAt(error.error, "$.error"),
		currentRevision: optionalInteger(error.current_revision, "$.current_revision"),
		currentRelatedRevision: optionalInteger(
			error.current_related_revision,
			"$.current_related_revision",
		),
		retryable: booleanAt(error.retryable, "$.retryable"),
	};
}

function encodeAction(action: PlaybackTopologyAction) {
	if (action.type === "save_cue_list")
		return {
			type: action.type,
			cue_list_id: action.cueListId,
			expected_revision: revisionAt(
				action.expectedRevision,
				"$.action.expectedRevision",
			),
			body: action.body,
		};
	const shared = {
		type: action.type,
		page: boundedPositiveInteger(action.page, "$.action.page", 127),
		slot: boundedPositiveInteger(action.slot, "$.action.slot", 127),
		expected_page_revision: revisionAt(
			action.expectedPageRevision,
			"$.action.expectedPageRevision",
		),
		expected_playback_revision: revisionAt(
			action.expectedPlaybackRevision,
			"$.action.expectedPlaybackRevision",
		),
	};
	return action.type === "configure_slot"
		? { ...shared, playback: encodePlayback(action.playback) }
		: shared;
}

function encodePlayback(playback: PlaybackDefinition) {
	const number = revisionAt(playback.number, "$.action.playback.number");
	if (number > 1000)
		invalid("$.action.playback.number", "integer between 0 and 1000", number);
	return {
		number,
		name: plainStringAt(playback.name, "$.action.playback.name"),
		target: encodeTarget(playback.target),
		buttons: playback.buttons,
		button_count: playback.button_count ?? 3,
		fader: playback.fader,
		has_fader: playback.has_fader ?? true,
		go_activates: playback.go_activates,
		auto_off: playback.auto_off,
		xfade_millis: revisionAt(
			playback.xfade_millis,
			"$.action.playback.xfade_millis",
		),
		color: playback.color ?? "#20c997",
		flash_release: playback.flash_release ?? "release_all",
		protect_from_swap: playback.protect_from_swap ?? false,
		presentation_icon: playback.presentation_icon ?? null,
		presentation_image: playback.presentation_image ?? null,
	};
}

function encodeTarget(target: PlaybackDefinition["target"]) {
	if (target.type === "cue_list")
		return {
			type: target.type,
			cue_list_id: printableAt(
				target.cue_list_id,
				"$.action.playback.target.cue_list_id",
				128,
			),
		};
	if (target.type === "group")
		return {
			type: target.type,
			group_id: printableAt(
				target.group_id,
				"$.action.playback.target.group_id",
				128,
			),
		};
	if (target.type === "speed_group")
		return {
			type: target.type,
			group: printableAt(
				target.group,
				"$.action.playback.target.group",
				16,
			),
		};
	return { type: target.type };
}

function decodeResolution(
	value: unknown,
	action: PlaybackTopologyAction,
): PlaybackTopologyResolution {
	const resolution = recordAt(value, "$.resolution");
	const kind = enumAt(resolution.kind, "$.resolution.kind", [
		"cue_list",
		"page_slot",
	]);
	if (kind === "cue_list") {
		exactRecordAt(resolution, "$.resolution", ["kind", "cue_list_id"]);
		const cueListId = stringAt(
			resolution.cue_list_id,
			"$.resolution.cue_list_id",
		);
		if (action.type !== "save_cue_list" || action.cueListId !== cueListId)
			invalid("$.resolution", "the requested Cuelist", resolution);
		return { kind, cueListId };
	}
	exactRecordAt(resolution, "$.resolution", [
		"kind",
		"page",
		"slot",
		"playback_number",
	]);
	const page = boundedPositiveInteger(resolution.page, "$.resolution.page", 127);
	const slot = boundedPositiveInteger(resolution.slot, "$.resolution.slot", 127);
	const playbackNumber =
		resolution.playback_number == null
			? null
			: boundedPositiveInteger(
					resolution.playback_number,
					"$.resolution.playback_number",
					1000,
				);
	if (action.type === "save_cue_list" || action.page !== page || action.slot !== slot)
		invalid("$.resolution", "the requested page slot", resolution);
	return { kind, page, slot, playbackNumber };
}

function decodeObjects(value: unknown): PlaybackTopologyObject[] {
	const objects = arrayAt(value, "$.objects").map((item, index) =>
		decodeObject(item, `$.objects[${index}]`),
	);
	const keys = new Set<string>();
	for (const object of objects) {
		const key = `${object.kind}:${object.objectId}`;
		if (keys.has(key)) invalid("$.objects", "unique object identities", key);
		keys.add(key);
	}
	return objects;
}

function decodeObject(value: unknown, path: string): PlaybackTopologyObject {
	const object = recordAt(value, path);
	const state = enumAt(object.state, `${path}.state`, ["present", "deleted"]);
	const fields = ["state", "kind", "object_id", "object_revision"];
	if (state === "present") fields.push("body");
	exactRecordAt(object, path, fields);
	const kind = enumAt(object.kind, `${path}.kind`, TOPOLOGY_KINDS);
	const objectId = stringAt(object.object_id, `${path}.object_id`);
	const objectRevision = integerAt(
		object.object_revision,
		`${path}.object_revision`,
	);
	if (state === "deleted") return { state, kind, objectId, objectRevision };
	return {
		state,
		kind,
		objectId,
		objectRevision,
		body: decodeShowObjectBody(kind, object.body, `${path}.body`, objectId),
	} as PlaybackTopologyObject;
}

function outcomeFields(status: "changed" | "no_change") {
	const fields = [
		"request_id",
		"correlation_id",
		"show_revision",
		"resolution",
		"status",
		"objects",
		"replayed",
	];
	if (status === "changed") fields.push("event_sequence");
	return fields;
}

function optionalInteger(value: unknown, path: string) {
	return value == null ? null : integerAt(value, path);
}

function revisionAt(value: unknown, path: string) {
	return integerAt(value, path);
}

function boundedPositiveInteger(value: unknown, path: string, maximum: number) {
	const integer = integerAt(value, path);
	if (integer < 1 || integer > maximum)
		invalid(path, `integer between 1 and ${maximum}`, value);
	return integer;
}

function plainStringAt(value: unknown, path: string) {
	if (typeof value !== "string") invalid(path, "string", value);
	return value;
}

function printableAt(value: unknown, path: string, maximumBytes: number) {
	if (
		typeof value !== "string" ||
		!value.trim() ||
		new TextEncoder().encode(value).length > maximumBytes ||
		/\p{Cc}/u.test(value)
	)
		invalid(path, `1-${maximumBytes} printable bytes`, value);
	return value;
}

function uuidAt(value: unknown, path: string) {
	const uuid = stringAt(value, path);
	if (
		!/^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(
			uuid,
		)
	)
		invalid(path, "non-nil hyphenated UUID", value);
	return uuid;
}

function invalid(path: string, expected: string, actual: unknown): never {
	throw new WireValidationError(path, expected, actual);
}
