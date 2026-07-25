import type {
	ProgrammerPreloadCommitOutcome,
	ProgrammerPreloadLifecycleOutcome,
	ProgrammerPreloadLifecycleRequest,
} from "../features/programmerPreloadLifecycle/contracts";
import type { ProgrammingPreloadLifecycleRequest as WireRequest } from "./generated/light-wire";
import { decodeCaptureModeProjection } from "./programmerCaptureModeWire";
import {
	arrayAt,
	booleanAt,
	enumAt,
	exactRecordAt,
	integerAt,
	printableStringAt,
	recordAt,
	stringAt,
} from "./playbackWirePrimitives";
import {
	decodeProgrammerPreloadPlaybackQueueProjection,
} from "./programmerPreloadPlaybackQueueWire";
import {
	decodePreloadExecutedAction,
	decodeStrictPlaybackProjection,
} from "./programmerPreloadLifecycleRuntimeWire";
import { decodeProgrammerPreloadValuesProjection } from "./programmerPreloadValuesWireProjection";
import { programmerValuesUuidAt } from "./programmerValuesWireProjection";
import { WireValidationError } from "./wireValidation";

export {
	decodeProgrammerPreloadLifecycleErrorResponse,
	PROGRAMMER_PRELOAD_LIFECYCLE_ERROR_KINDS,
	type ProgrammerPreloadLifecycleErrorResponse,
} from "./programmerPreloadLifecycleErrorWire";

const OPTIONAL_OUTCOME_FIELDS = [
	"capture_mode_event_sequence",
	"values_projection",
	"values_event_sequence",
	"queue_projection",
	"queue_event_sequence",
	"interaction_event_sequence",
	"commit",
	"warning",
] as const;

export function encodeProgrammerPreloadLifecycleRequest(
	request: ProgrammerPreloadLifecycleRequest,
): WireRequest {
	validateRequest(request);
	return {
		request_id: request.requestId,
		expected_capture_mode_revision: request.expectedCaptureModeRevision,
		expected_values_revision: request.expectedValuesRevision,
		expected_queue_revision: request.expectedQueueRevision,
		expected_selection_revision: request.expectedSelectionRevision,
		action:
			request.action.type === "go"
				? {
						type: "go",
						show_id: request.action.showId,
						expected_show_revision:
							request.action.expectedShowRevision,
						expected_playback_event_sequence:
							request.action.expectedPlaybackEventSequence,
					}
				: { type: request.action.type },
	};
}

export function decodeProgrammerPreloadLifecycleOutcome(
	value: unknown,
	expectedUserId: string,
	request: ProgrammerPreloadLifecycleRequest,
): ProgrammerPreloadLifecycleOutcome {
	const candidate = recordAt(value, "$");
	const response = exactRecordAt(value, "$", [
		"request_id",
		"correlation_id",
		"replayed",
		"status",
		"active",
		"capture_mode",
		"values_revision",
		"queue_revision",
		"selection_revision",
		...OPTIONAL_OUTCOME_FIELDS.filter((key) => key in candidate),
	]);
	const requestId = stringAt(response.request_id, "$.request_id");
	if (requestId !== request.requestId)
		throw mismatch("$.request_id", request.requestId, requestId);
	const status = enumAt(response.status, "$.status", [
		"changed",
		"no_change",
	]);
	const captureMode = decodeCaptureModeProjection(
		response.capture_mode,
		"$.capture_mode",
		expectedUserId,
	);
	const outcome: ProgrammerPreloadLifecycleOutcome = {
		requestId,
		correlationId: programmerValuesUuidAt(
			response.correlation_id,
			"$.correlation_id",
		),
		replayed: booleanAt(response.replayed, "$.replayed"),
		status,
		active: booleanAt(response.active, "$.active"),
		captureMode,
		captureModeEventSequence: optionalInteger(
			response,
			"capture_mode_event_sequence",
		),
		valuesRevision: integerAt(response.values_revision, "$.values_revision"),
		valuesProjection:
			response.values_projection == null
				? null
				: decodeProgrammerPreloadValuesProjection(
						response.values_projection,
						"$.values_projection",
						expectedUserId,
					),
		valuesEventSequence: optionalInteger(response, "values_event_sequence"),
		queueRevision: integerAt(response.queue_revision, "$.queue_revision"),
		queueProjection:
			response.queue_projection == null
				? null
				: decodeProgrammerPreloadPlaybackQueueProjection(
						response.queue_projection,
						"$.queue_projection",
						expectedUserId,
					),
		queueEventSequence: optionalInteger(response, "queue_event_sequence"),
		interactionEventSequence: optionalInteger(
			response,
			"interaction_event_sequence",
		),
		selectionRevision: integerAt(
			response.selection_revision,
			"$.selection_revision",
		),
		commit:
			response.commit == null ? null : decodeCommit(response.commit, request),
		warning: optionalString(response, "warning"),
	};
	assertOutcome(outcome, request);
	return outcome;
}

function decodeCommit(
	value: unknown,
	request: ProgrammerPreloadLifecycleRequest,
): ProgrammerPreloadCommitOutcome {
	const commit = exactRecordAt(value, "$.commit", [
		"show_id",
		"show_revision",
		"playback_event_sequence_before",
		"playback_event_sequence_after",
		"committed_at",
		"programmer_fade_millis",
		"executed_playback_actions",
		"executed",
		"runtime_changes",
	]);
	const executed = arrayAt(commit.executed, "$.commit.executed").map(
		(item, index) =>
			decodePreloadExecutedAction(item, `$.commit.executed[${index}]`),
	);
	const runtimeChanges = arrayAt(
		commit.runtime_changes,
		"$.commit.runtime_changes",
	).map((value, index) => {
		const path = `$.commit.runtime_changes[${index}]`;
		const change = exactRecordAt(value, path, ["projection", "event_sequence"]);
		return {
			projection: decodeStrictPlaybackProjection(
				change.projection,
				`${path}.projection`,
			),
			eventSequence: integerAt(change.event_sequence, `${path}.event_sequence`),
		};
	});
	const committedAt = stringAt(commit.committed_at, "$.commit.committed_at");
	if (!committedAt || Number.isNaN(Date.parse(committedAt)))
		throw mismatch("$.commit.committed_at", "an ISO timestamp", committedAt);
	const result = {
		showId: programmerValuesUuidAt(commit.show_id, "$.commit.show_id"),
		showRevision: integerAt(commit.show_revision, "$.commit.show_revision"),
		playbackEventSequenceBefore: integerAt(
			commit.playback_event_sequence_before,
			"$.commit.playback_event_sequence_before",
		),
		playbackEventSequenceAfter: integerAt(
			commit.playback_event_sequence_after,
			"$.commit.playback_event_sequence_after",
		),
		committedAt,
		programmerFadeMillis: integerAt(
			commit.programmer_fade_millis,
			"$.commit.programmer_fade_millis",
		),
		executedPlaybackActions: integerAt(
			commit.executed_playback_actions,
			"$.commit.executed_playback_actions",
		),
		executed,
		runtimeChanges,
	};
	assertCommit(result, request);
	return result;
}

function assertOutcome(
	outcome: ProgrammerPreloadLifecycleOutcome,
	request: ProgrammerPreloadLifecycleRequest,
) {
	assertRevisionEventPair(
		"capture_mode",
		outcome.captureMode.revision,
		outcome.captureModeEventSequence,
		request.expectedCaptureModeRevision,
	);
	assertProjectionPair(
		"values",
		outcome.valuesRevision,
		outcome.valuesEventSequence,
		request.expectedValuesRevision,
		outcome.valuesProjection !== null,
	);
	if (outcome.valuesProjection?.revision !== undefined &&
		outcome.valuesProjection.revision !== outcome.valuesRevision)
		throw mismatch("$.values_revision", outcome.valuesProjection.revision, outcome.valuesRevision);
	assertProjectionPair(
		"queue",
		outcome.queueRevision,
		outcome.queueEventSequence,
		request.expectedQueueRevision,
		outcome.queueProjection !== null,
	);
	if (outcome.queueProjection?.revision !== undefined &&
		outcome.queueProjection.revision !== outcome.queueRevision)
		throw mismatch("$.queue_revision", outcome.queueProjection.revision, outcome.queueRevision);
	assertRevisionEventPair(
		"selection",
		outcome.selectionRevision,
		outcome.interactionEventSequence,
		request.expectedSelectionRevision,
	);
	if (outcome.status === "no_change") assertSparseNoChange(outcome, request);
	if ((request.action.type === "go") !== (outcome.commit !== null))
		throw mismatch(
			"$.commit",
			request.action.type === "go" ? "a GO commit" : "absent",
			outcome.commit,
		);
}

function assertProjectionPair(
	name: string,
	revision: number,
	eventSequence: number | null,
	expectedRevision: number,
	projectionPresent: boolean,
) {
	if (projectionPresent !== (eventSequence !== null))
		throw mismatch(`$.${name}`, "a paired projection and event sequence", {
			projectionPresent,
			eventSequence,
		});
	assertRevisionEventPair(name, revision, eventSequence, expectedRevision);
}

function assertRevisionEventPair(
	name: string,
	revision: number,
	eventSequence: number | null,
	expectedRevision: number,
) {
	const expected = eventSequence === null ? expectedRevision : expectedRevision + 1;
	if (revision !== expected)
		throw mismatch(`$.${name}_revision`, expected, revision);
}

function assertSparseNoChange(
	outcome: ProgrammerPreloadLifecycleOutcome,
	request: ProgrammerPreloadLifecycleRequest,
) {
	if (
		outcome.captureModeEventSequence !== null ||
		outcome.valuesProjection !== null ||
		outcome.valuesEventSequence !== null ||
		outcome.queueProjection !== null ||
		outcome.queueEventSequence !== null ||
		outcome.interactionEventSequence !== null ||
		outcome.commit !== null
	)
		throw mismatch("$", "a sparse no_change outcome", outcome);
	if (request.action.type === "go")
		throw mismatch("$.status", "changed for GO", outcome.status);
}

function assertCommit(
	commit: ProgrammerPreloadCommitOutcome,
	request: ProgrammerPreloadLifecycleRequest,
) {
	if (request.action.type !== "go") return;
	if (commit.showId.toLowerCase() !== request.action.showId.toLowerCase())
		throw mismatch("$.commit.show_id", request.action.showId, commit.showId);
	if (commit.showRevision !== request.action.expectedShowRevision)
		throw mismatch(
			"$.commit.show_revision",
			request.action.expectedShowRevision,
			commit.showRevision,
		);
	if (
		commit.playbackEventSequenceBefore !==
		request.action.expectedPlaybackEventSequence
	)
		throw mismatch(
			"$.commit.playback_event_sequence_before",
			request.action.expectedPlaybackEventSequence,
			commit.playbackEventSequenceBefore,
		);
	if (commit.playbackEventSequenceAfter < commit.playbackEventSequenceBefore)
		throw mismatch(
			"$.commit.playback_event_sequence_after",
			`at least ${commit.playbackEventSequenceBefore}`,
			commit.playbackEventSequenceAfter,
		);
	if (commit.executedPlaybackActions !== commit.executed.length)
		throw mismatch(
			"$.commit.executed_playback_actions",
			commit.executed.length,
			commit.executedPlaybackActions,
		);
	for (const [index, change] of commit.runtimeChanges.entries()) {
		if (change.projection.scope.show_id.toLowerCase() !== commit.showId.toLowerCase())
			throw mismatch(
				`$.commit.runtime_changes[${index}].projection.scope.show_id`,
				commit.showId,
				change.projection.scope.show_id,
			);
		if (change.projection.scope.show_revision !== commit.showRevision)
			throw mismatch(
				`$.commit.runtime_changes[${index}].projection.scope.show_revision`,
				commit.showRevision,
				change.projection.scope.show_revision,
			);
		if (
			change.eventSequence <= commit.playbackEventSequenceBefore ||
			change.eventSequence > commit.playbackEventSequenceAfter
		)
			throw mismatch(
				`$.commit.runtime_changes[${index}].event_sequence`,
				`between ${commit.playbackEventSequenceBefore + 1} and ${commit.playbackEventSequenceAfter}`,
				change.eventSequence,
			);
	}
}

function validateRequest(request: ProgrammerPreloadLifecycleRequest) {
	printableStringAt(request.requestId, "$.requestId", 128);
	for (const [key, value] of [
		["expectedCaptureModeRevision", request.expectedCaptureModeRevision],
		["expectedValuesRevision", request.expectedValuesRevision],
		["expectedQueueRevision", request.expectedQueueRevision],
		["expectedSelectionRevision", request.expectedSelectionRevision],
	] as const)
		integerAt(value, `$.${key}`);
	if (request.action.type !== "go") return;
	programmerValuesUuidAt(request.action.showId, "$.action.showId");
	integerAt(request.action.expectedShowRevision, "$.action.expectedShowRevision");
	integerAt(
		request.action.expectedPlaybackEventSequence,
		"$.action.expectedPlaybackEventSequence",
	);
}

function optionalInteger(object: Record<string, unknown>, key: string) {
	return object[key] == null ? null : integerAt(object[key], `$.${key}`);
}

function optionalString(object: Record<string, unknown>, key: string) {
	return object[key] == null ? null : stringAt(object[key], `$.${key}`);
}

function mismatch(path: string, expected: unknown, actual: unknown) {
	return new WireValidationError(path, String(expected), actual);
}
