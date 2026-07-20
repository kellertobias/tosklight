import type {
	CueRecordingOutcome,
	CueRecordingRequest,
	CueRecordProjections,
	CueRecordTarget,
} from "../features/cueRecording/contracts";
import {
	booleanAt,
	enumAt,
	exactRecordAt,
	integerAt,
	numberAt,
	recordAt,
	stringAt,
} from "./playbackWirePrimitives";
import { decodePlaybackProjection } from "./playbackWireProjection";
import { decodeShowObject } from "./showObjectWire";
import { WireValidationError } from "./wireValidation";

const ERROR_KINDS = [
	"invalid",
	"unauthorized",
	"forbidden",
	"not_found",
	"conflict",
	"unavailable",
	"internal",
] as const;

export type CueRecordErrorKind = (typeof ERROR_KINDS)[number];

export interface CueRecordErrorResponse {
	kind: CueRecordErrorKind;
	error: string;
	currentRevision: number | null;
	retryable: boolean;
}

export function encodeCueRecordingRequest(request: CueRecordingRequest) {
	validateRequest(request);
	return {
		request_id: request.requestId,
		target: encodeTarget(request.target),
		operation: request.operation,
		cue_number: request.cueNumber ?? null,
		timing: {
			...(request.timing.fadeMillis == null
				? {}
				: { fade_millis: request.timing.fadeMillis }),
			...(request.timing.delayMillis == null
				? {}
				: { delay_millis: request.timing.delayMillis }),
		},
		cue_only: request.cueOnly,
		name: request.name ?? null,
		capture_policy: request.capturePolicy,
		activation_policy: request.activationPolicy,
	};
}

export function decodeCueRecordingOutcome(
	value: unknown,
	request: CueRecordingRequest,
	showId: string,
	expectedShowRevision: number,
): CueRecordingOutcome {
	const outcome = recordAt(value, "$.");
	const status = enumAt(outcome.status, "$.status", ["changed", "no_change"]);
	exactRecordAt(outcome, "$", outcomeFields(status));
	const requestId = stringAt(outcome.request_id, "$.request_id");
	if (requestId !== request.requestId)
		invalid("$.request_id", `request ${request.requestId}`, requestId);
	const showRevision = integerAt(outcome.show_revision, "$.show_revision");
	const expectedRevision = expectedShowRevision + (status === "changed" ? 1 : 0);
	if (showRevision !== expectedRevision)
		invalid("$.show_revision", String(expectedRevision), showRevision);
	const projections = decodeProjections(outcome.projections, request);
	const recordedCue = decodeRecordedCue(outcome.recorded_cue, projections, request);
	const common = {
		requestId,
		correlationId: uuidAt(outcome.correlation_id, "$.correlation_id"),
		replayed: booleanAt(outcome.replayed, "$.replayed"),
		capturedSource: enumAt(outcome.captured_source, "$.captured_source", [
			"normal",
			"pending_preload",
			"active_preload",
		]),
		showRevision,
		recordedCue,
		projections,
	};
	if (status === "no_change") return { ...common, status };
	return {
		...common,
		status,
		showEventSequence: integerAt(
			outcome.show_event_sequence,
			"$.show_event_sequence",
		),
		runtime:
			outcome.runtime == null
				? null
				: decodeRuntime(
						outcome.runtime,
						showId,
						showRevision,
						projections,
					),
	};
}

export function decodeCueRecordErrorResponse(
	value: unknown,
): CueRecordErrorResponse {
	const error = recordAt(value, "$.");
	const fields = ["kind", "error", "retryable"];
	if ("current_revision" in error) fields.push("current_revision");
	exactRecordAt(error, "$", fields);
	return {
		kind: enumAt(error.kind, "$.kind", ERROR_KINDS),
		error: stringAt(error.error, "$.error"),
		currentRevision:
			error.current_revision == null
				? null
				: integerAt(error.current_revision, "$.current_revision"),
		retryable: booleanAt(error.retryable, "$.retryable"),
	};
}

function decodeProjections(
	value: unknown,
	request: CueRecordingRequest,
): CueRecordProjections {
	const projections = exactRecordAt(value, "$.projections", [
		"cue_list",
		"playback",
		"page",
	]);
	const cueList = decodeProjection(
		projections.cue_list,
		"cue_list",
		"$.projections.cue_list",
	);
	const playback =
		projections.playback == null
			? null
			: decodeProjection(
					projections.playback,
					"playback",
					"$.projections.playback",
				);
	const page =
		projections.page == null
			? null
			: decodeProjection(
					projections.page,
					"playback_page",
					"$.projections.page",
				);
	if (
		playback?.body.target.type === "cue_list" &&
		playback.body.target.cue_list_id !== cueList.id
	)
		invalid(
			"$.projections.playback.body.target.cue_list_id",
			`recorded Cuelist ${cueList.id}`,
			playback.body.target.cue_list_id,
		);
	if (request.target.kind === "cue_list" && cueList.id !== request.target.cueListId)
		invalid("$.projections.cue_list.id", request.target.cueListId, cueList.id);
	validateTargetTopology(request.target, playback, page);
	return { cueList, playback, page };
}

function validateTargetTopology(
	target: CueRecordTarget,
	playback: CueRecordProjections["playback"],
	page: CueRecordProjections["page"],
) {
	if (target.kind === "cue_list") {
		if (playback || page)
			invalid("$.projections", "no Playback topology for a direct Cuelist", {
				playback,
				page,
			});
		return;
	}
	if (!playback)
		invalid("$.projections.playback", "resolved Playback projection", playback);
	if (target.kind === "pool" && playback.body.number !== target.playbackNumber)
		invalid(
			"$.projections.playback.body.number",
			String(target.playbackNumber),
			playback.body.number,
		);
	if (target.kind !== "page_slot") {
		if (page) invalid("$.projections.page", "no Playback page projection", page);
		return;
	}
	if (!page)
		invalid("$.projections.page", "resolved Playback page projection", page);
	if (page.body.number !== target.page)
		invalid("$.projections.page.body.number", String(target.page), page.body.number);
	if (page.body.slots[String(target.slot)] !== playback.body.number)
		invalid(
			`$.projections.page.body.slots.${target.slot}`,
			`Playback ${playback.body.number}`,
			page.body.slots[String(target.slot)],
		);
}

function decodeProjection<K extends "cue_list" | "playback" | "playback_page">(
	value: unknown,
	kind: K,
	path: string,
) {
	const projection = exactRecordAt(value, path, ["id", "revision", "body"]);
	return decodeShowObject(
		{
			kind,
			id: stringAt(projection.id, `${path}.id`),
			revision: integerAt(projection.revision, `${path}.revision`),
			updated_at: "",
			body: projection.body,
		},
		kind,
		path,
	);
}

function decodeRecordedCue(
	value: unknown,
	projections: CueRecordProjections,
	request: CueRecordingRequest,
) {
	const cue = exactRecordAt(value, "$.recorded_cue", [
		"id",
		"number",
		"deleted",
	]);
	const recorded = {
		id: uuidAt(cue.id, "$.recorded_cue.id"),
		number: positiveNumberAt(cue.number, "$.recorded_cue.number"),
		deleted: booleanAt(cue.deleted, "$.recorded_cue.deleted"),
	};
	if (request.cueNumber != null && recorded.number !== request.cueNumber)
		invalid("$.recorded_cue.number", String(request.cueNumber), recorded.number);
	const stored = projections.cueList.body.cues.find(
		(candidate) => candidate.id === recorded.id,
	);
	if (recorded.deleted && request.operation !== "subtract")
		invalid("$.recorded_cue.deleted", "false outside Subtract", true);
	if (recorded.deleted && stored)
		invalid(
			"$.recorded_cue",
			"deleted Cue absent from authoritative Cuelist",
			recorded,
		);
	if (!recorded.deleted && (!stored || stored.number !== recorded.number))
		invalid("$.recorded_cue", "Cue present in authoritative Cuelist", recorded);
	return recorded;
}

function decodeRuntime(
	value: unknown,
	showId: string,
	showRevision: number,
	projections: CueRecordProjections,
) {
	const runtime = exactRecordAt(value, "$.runtime", [
		"projection",
		"event_sequence",
	]);
	const projection = decodePlaybackProjection(
		runtime.projection,
		"$.runtime.projection",
	);
	if (projection.scope.show_id !== showId)
		invalid("$.runtime.projection.scope.show_id", showId, projection.scope.show_id);
	if (projection.scope.show_revision !== showRevision)
		invalid(
			"$.runtime.projection.scope.show_revision",
			String(showRevision),
			projection.scope.show_revision,
		);
	validateRuntimePlayback(projection, projections);
	return {
		projection,
		eventSequence: integerAt(runtime.event_sequence, "$.runtime.event_sequence"),
	};
}

function validateRuntimePlayback(
	projection: ReturnType<typeof decodePlaybackProjection>,
	projections: CueRecordProjections,
) {
	const playback = projections.playback;
	if (!playback)
		invalid("$.runtime", "no runtime for a direct Cuelist action", projection);
	if (projection.playback_number !== playback.body.number)
		invalid(
			"$.runtime.projection.playback_number",
			String(playback.body.number),
			projection.playback_number,
		);
	if (projection.target !== "cue_list" || projection.cue_list_id !== projections.cueList.id)
		invalid(
			"$.runtime.projection.cue_list_id",
			projections.cueList.id,
			projection.target === "cue_list" ? projection.cue_list_id : projection.target,
		);
}

function encodeTarget(target: CueRecordTarget) {
	if (target.kind === "pool")
		return { kind: target.kind, playback_number: target.playbackNumber };
	if (target.kind === "page_slot")
		return { kind: target.kind, page: target.page, slot: target.slot };
	if (target.kind === "cue_list")
		return { kind: target.kind, cue_list_id: target.cueListId };
	return { kind: target.kind };
}

function validateRequest(request: CueRecordingRequest) {
	printableAt(request.requestId, "$.requestId", 128);
	if (request.target.kind === "pool")
		boundedPositiveInteger(request.target.playbackNumber, "$.target.playbackNumber", 1000);
	if (request.target.kind === "page_slot") {
		boundedPositiveInteger(request.target.page, "$.target.page", 127);
		boundedPositiveInteger(request.target.slot, "$.target.slot", 127);
	}
	if (request.target.kind === "cue_list") uuidAt(request.target.cueListId, "$.target.cueListId");
	if (request.cueNumber != null) positiveNumberAt(request.cueNumber, "$.cueNumber");
	if (request.name != null) printableAt(request.name, "$.name", 256);
	if (request.timing.fadeMillis != null)
		integerAt(request.timing.fadeMillis, "$.timing.fadeMillis");
	if (request.timing.delayMillis != null)
		integerAt(request.timing.delayMillis, "$.timing.delayMillis");
}

function outcomeFields(status: "changed" | "no_change") {
	const fields = [
		"status",
		"request_id",
		"correlation_id",
		"replayed",
		"captured_source",
		"show_revision",
		"recorded_cue",
		"projections",
	];
	if (status === "changed") fields.push("show_event_sequence", "runtime");
	return fields;
}

function boundedPositiveInteger(value: unknown, path: string, maximum: number) {
	const integer = integerAt(value, path);
	if (integer < 1 || integer > maximum)
		invalid(path, `integer between 1 and ${maximum}`, value);
	return integer;
}

function positiveNumberAt(value: unknown, path: string) {
	const number = numberAt(value, path);
	if (number <= 0) invalid(path, "positive number", value);
	return number;
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
