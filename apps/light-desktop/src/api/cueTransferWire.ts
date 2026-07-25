import type {
	CueTransferActionOutcome,
	CueTransferActionRequest,
	CueTransferProjection,
	CueTransferSummary,
} from "../features/cueTransfer/contracts";
import {
	arrayAt,
	booleanAt,
	enumAt,
	exactRecordAt,
	integerAt,
	numberAt,
	stringAt,
} from "./playbackWirePrimitives";
import {
	decodeProgrammingCommandLine,
	programmingUuidAt,
} from "./programmingWireProjection";
import { decodeShowObjectBody } from "./showObjectBodyWire";
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

export type CueTransferErrorKind = (typeof ERROR_KINDS)[number];

export interface CueTransferErrorResponse {
	kind: CueTransferErrorKind;
	error: string;
	currentRevision: number | null;
	currentRelatedRevision: number | null;
	retryable: boolean;
}

export function encodeCueTransferActionRequest(
	request: CueTransferActionRequest,
) {
	programmingUuidAt(request.requestId, "$.request_id");
	programmingUuidAt(request.choiceId, "$.choice_id");
	integerAt(
		request.expectedCommandLineRevision,
		"$.expected_command_line_revision",
	);
	return {
		request_id: request.requestId,
		choice_id: request.choiceId,
		mode: request.mode,
		expected_command_line_revision: request.expectedCommandLineRevision,
	};
}

export function decodeCueTransferActionOutcome(
	value: unknown,
	request: CueTransferActionRequest,
	expectedShowId: string,
	expectedShowRevision: number,
): CueTransferActionOutcome {
	const outcome = exactRecordAt(value, "$", [
		"status",
		"request_id",
		"choice_id",
		"correlation_id",
		"replayed",
		"show_id",
		"summary",
		"show_revision",
		"projections",
		"show_event_sequence",
		"command_line",
		"interaction_event_sequence",
		"persistence_warning",
	]);
	enumAt(outcome.status, "$.status", ["changed"]);
	const requestId = programmingUuidAt(outcome.request_id, "$.request_id");
	if (requestId !== request.requestId)
		invalid("$.request_id", `request ${request.requestId}`, requestId);
	const choiceId = programmingUuidAt(outcome.choice_id, "$.choice_id");
	if (choiceId !== request.choiceId)
		invalid("$.choice_id", `choice ${request.choiceId}`, choiceId);
	const showId = programmingUuidAt(outcome.show_id, "$.show_id");
	if (showId !== expectedShowId)
		invalid("$.show_id", `Show ${expectedShowId}`, showId);
	const showRevision = integerAt(outcome.show_revision, "$.show_revision");
	if (showRevision !== expectedShowRevision + 1)
		invalid("$.show_revision", String(expectedShowRevision + 1), showRevision);
	const summary = decodeSummary(outcome.summary, request);
	const projections = decodeProjections(outcome.projections, summary);
	const commandLine = decodeProgrammingCommandLine(
		outcome.command_line,
		"$.command_line",
	);
	if (commandLine.pendingChoice !== null)
		invalid(
			"$.command_line.pending_choice",
			"cleared choice",
			commandLine.pendingChoice,
		);
	if (commandLine.revision !== request.expectedCommandLineRevision + 1)
		invalid(
			"$.command_line.revision",
			String(request.expectedCommandLineRevision + 1),
			commandLine.revision,
		);
	return {
		requestId,
		choiceId,
		correlationId: programmingUuidAt(
			outcome.correlation_id,
			"$.correlation_id",
		),
		replayed: booleanAt(outcome.replayed, "$.replayed"),
		showId,
		summary,
		showRevision,
		projections,
		showEventSequence: integerAt(
			outcome.show_event_sequence,
			"$.show_event_sequence",
		),
		commandLine,
		interactionEventSequence: nullableInteger(
			outcome.interaction_event_sequence,
			"$.interaction_event_sequence",
		),
		persistenceWarning: nullableString(
			outcome.persistence_warning,
			"$.persistence_warning",
		),
	};
}

export function decodeCueTransferErrorResponse(
	value: unknown,
): CueTransferErrorResponse {
	const response = exactRecordAt(value, "$", [
		"kind",
		"error",
		"current_revision",
		"current_related_revision",
		"retryable",
	]);
	return {
		kind: enumAt(response.kind, "$.kind", ERROR_KINDS),
		error: stringAt(response.error, "$.error"),
		currentRevision: nullableInteger(
			response.current_revision,
			"$.current_revision",
		),
		currentRelatedRevision: nullableInteger(
			response.current_related_revision,
			"$.current_related_revision",
		),
		retryable: booleanAt(response.retryable, "$.retryable"),
	};
}

function decodeSummary(
	value: unknown,
	request: CueTransferActionRequest,
): CueTransferSummary {
	const summary = exactRecordAt(value, "$.summary", [
		"operation",
		"mode",
		"source_cue_id",
		"source_cue_number",
		"destination_cue_id",
		"destination_cue_number",
	]);
	const mode = enumAt(summary.mode, "$.summary.mode", ["plain", "status"]);
	if (mode !== request.mode) invalid("$.summary.mode", request.mode, mode);
	const decoded = {
		operation: enumAt(summary.operation, "$.summary.operation", [
			"copy",
			"move",
		]),
		mode,
		sourceCueId: programmingUuidAt(
			summary.source_cue_id,
			"$.summary.source_cue_id",
		),
		sourceCueNumber: positiveNumberAt(
			summary.source_cue_number,
			"$.summary.source_cue_number",
		),
		destinationCueId: programmingUuidAt(
			summary.destination_cue_id,
			"$.summary.destination_cue_id",
		),
		destinationCueNumber: positiveNumberAt(
			summary.destination_cue_number,
			"$.summary.destination_cue_number",
		),
	} satisfies CueTransferSummary;
	if (
		(decoded.operation === "copy") ===
		(decoded.sourceCueId === decoded.destinationCueId)
	)
		invalid(
			"$.summary.destination_cue_id",
			decoded.operation === "copy"
				? "a new Cue ID for Copy"
				: "the source Cue ID for Move",
			decoded.destinationCueId,
		);
	return decoded;
}

function decodeProjections(
	value: unknown,
	summary: CueTransferSummary,
): CueTransferProjection[] {
	const raw = arrayAt(value, "$.projections");
	if (raw.length < 1 || raw.length > 2)
		invalid("$.projections", "one or two Cuelist projections", raw);
	const projections = raw.map((item, index) =>
		decodeProjection(item, `$.projections[${index}]`),
	);
	if (
		new Set(projections.map(({ objectId }) => objectId)).size !==
		projections.length
	)
		invalid("$.projections", "unique Cuelist object IDs", raw);
	const destination = projections
		.flatMap(({ body }) => body.cues)
		.filter(
			(cue) =>
				cue.id === summary.destinationCueId &&
				cue.number === summary.destinationCueNumber,
		);
	if (destination.length !== 1)
		invalid(
			"$.projections",
			"exactly one transferred destination Cue",
			destination,
		);
	return projections;
}

function decodeProjection(value: unknown, path: string): CueTransferProjection {
	const projection = exactRecordAt(value, path, [
		"cue_list_id",
		"object_id",
		"object_revision",
		"body",
	]);
	const cueListId = programmingUuidAt(
		projection.cue_list_id,
		`${path}.cue_list_id`,
	);
	const objectId = stringAt(projection.object_id, `${path}.object_id`);
	const body = decodeShowObjectBody(
		"cue_list",
		projection.body,
		`${path}.body`,
		objectId,
	);
	if (body.id !== cueListId)
		invalid(`${path}.body.id`, `Cuelist ${cueListId}`, body.id);
	return {
		cueListId,
		objectId,
		objectRevision: integerAt(
			projection.object_revision,
			`${path}.object_revision`,
		),
		body,
	};
}

function nullableInteger(value: unknown, path: string) {
	return value == null ? null : integerAt(value, path);
}

function nullableString(value: unknown, path: string) {
	return value == null ? null : stringAt(value, path);
}

function positiveNumberAt(value: unknown, path: string) {
	const decoded = numberAt(value, path);
	if (decoded <= 0) invalid(path, "positive number", value);
	return decoded;
}

function invalid(path: string, expected: string, actual: unknown): never {
	throw new WireValidationError(path, expected, actual);
}
