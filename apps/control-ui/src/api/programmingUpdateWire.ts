import type { ShowObjectBodies } from "../features/showObjects/contracts";
import type {
	ProgrammingUpdateAction,
	ProgrammingUpdateActionOutcome,
	ProgrammingUpdateActionRequest,
	ProgrammingUpdateErrorKind,
	ProgrammingUpdateErrorResponse,
	ProgrammingUpdateProjection,
	ProgrammingUpdateSettings,
	ProgrammingUpdateSettingsProjection,
	ProgrammingUpdateSummary,
} from "./generated/light-wire";
import {
	arrayAt,
	booleanAt,
	enumAt,
	exactRecordAt,
	integerAt,
	recordAt,
} from "./playbackWirePrimitives";
import {
	assertIdentityMatchesRequest,
	assertModeMatchesTarget,
	assertObjectMatchesTarget,
	decodeCueSource,
	decodeObjectIdentity,
	decodeTargetIdentity,
	decodeUpdateMode,
	decodeUpdateTarget,
	plainStringAt,
	programmerRevisionAt,
	requestIdAt,
	scopedUuidAt,
} from "./programmingUpdateWireShared";
import { decodeShowObjectBody } from "./showObjectBodyWire";
import { WireValidationError } from "./wireValidation";

export {
	decodeProgrammingUpdatePreview,
	decodeProgrammingUpdatePreviewResponse,
	decodeProgrammingUpdateTargetsResponse,
	encodeProgrammingUpdatePreviewRequest,
	encodeProgrammingUpdateTargetsRequest,
} from "./programmingUpdatePreviewWire";

const ERROR_KINDS = [
	"invalid",
	"unauthorized",
	"forbidden",
	"not_found",
	"conflict",
	"unavailable",
	"internal",
] as const satisfies readonly ProgrammingUpdateErrorKind[];
const CUE_MODES = [
	"existing_only",
	"existing_in_current_cue",
	"add_to_current_cue",
	"add_new",
] as const;
const EXISTING_MODES = ["update_existing", "add_new"] as const;

export type DecodedProgrammingUpdateProjection = {
	[K in ProgrammingUpdateProjection["kind"]]: Omit<
		ProgrammingUpdateProjection,
		"kind" | "body"
	> & {
		kind: K;
		body: ShowObjectBodies[K];
	};
}[ProgrammingUpdateProjection["kind"]];

export type DecodedProgrammingUpdateActionOutcome = Omit<
	ProgrammingUpdateActionOutcome,
	"projection"
> & {
	projection: DecodedProgrammingUpdateProjection;
};

export function encodeProgrammingUpdateActionRequest(
	request: ProgrammingUpdateActionRequest,
) {
	decodeActionRequest(request, "$request");
	return request;
}

export function encodeProgrammingUpdateSettings(
	settings: ProgrammingUpdateSettings,
) {
	decodeSettings(settings, "$request");
	return settings;
}

export function decodeProgrammingUpdateActionOutcome(
	value: unknown,
	expectedShowId: string,
	expectedShowRevision: number,
	request: ProgrammingUpdateActionRequest,
): DecodedProgrammingUpdateActionOutcome {
	scopedUuidAt(expectedShowId, "$expected.show_id");
	integerAt(expectedShowRevision, "$expected.show_revision");
	decodeActionRequest(request, "$request");
	const outcome = exactRecordAt(value, "$", [
		"status",
		"request_id",
		"correlation_id",
		"replayed",
		"show_id",
		"show_revision",
		"projection",
		"event_sequence",
		"summary",
	]);
	enumAt(outcome.status, "$.status", ["changed"]);
	assertRequestId(outcome.request_id, request.request_id, "$.request_id");
	scopedUuidAt(outcome.correlation_id, "$.correlation_id");
	booleanAt(outcome.replayed, "$.replayed");
	assertScopeUuid(outcome.show_id, expectedShowId, "$.show_id");
	const showRevision = integerAt(outcome.show_revision, "$.show_revision");
	if (showRevision !== expectedShowRevision + 1)
		throw new WireValidationError(
			"$.show_revision",
			`revision ${expectedShowRevision + 1}`,
			showRevision,
		);
	const projection = decodeProjection(outcome.projection, "$.projection");
	integerAt(outcome.event_sequence, "$.event_sequence");
	const summary = decodeSummary(outcome.summary, "$.summary");
	assertIdentityMatchesRequest(
		summary.target,
		request.action.target,
		"$.summary.target",
	);
	assertProjectionMatchesTarget(projection, summary);
	assertOutcomeRevisions(projection, summary, request.action);
	return {
		...(value as ProgrammingUpdateActionOutcome),
		projection,
	};
}

export function decodeProgrammingUpdateSettingsProjection(
	value: unknown,
	expectedDeskId: string,
): ProgrammingUpdateSettingsProjection {
	scopedUuidAt(expectedDeskId, "$expected.desk_id");
	const projection = exactRecordAt(value, "$", ["desk_id", "settings"]);
	assertScopeUuid(projection.desk_id, expectedDeskId, "$.desk_id");
	decodeSettings(projection.settings, "$.settings");
	return value as ProgrammingUpdateSettingsProjection;
}

export function decodeProgrammingUpdateErrorResponse(
	value: unknown,
): ProgrammingUpdateErrorResponse {
	const error = exactRecordAt(
		value,
		"$",
		optionalFields(
			value,
			"$",
			["kind", "error", "retryable"],
			["current_object_revision", "current_show_revision"],
		),
	);
	enumAt(error.kind, "$.kind", ERROR_KINDS);
	plainStringAt(error.error, "$.error");
	booleanAt(error.retryable, "$.retryable");
	if (
		"current_object_revision" in error &&
		error.current_object_revision != null
	)
		integerAt(error.current_object_revision, "$.current_object_revision");
	if ("current_show_revision" in error && error.current_show_revision != null)
		integerAt(error.current_show_revision, "$.current_show_revision");
	return value as ProgrammingUpdateErrorResponse;
}

function decodeActionRequest(
	value: unknown,
	path: string,
): ProgrammingUpdateActionRequest {
	const request = exactRecordAt(value, path, ["request_id", "action"]);
	requestIdAt(request.request_id, `${path}.request_id`);
	decodeAction(request.action, `${path}.action`);
	return value as ProgrammingUpdateActionRequest;
}

function decodeAction(value: unknown, path: string): ProgrammingUpdateAction {
	const action = exactRecordAt(value, path, actionFields(value, path));
	const type = enumAt(action.type, `${path}.type`, [
		"confirm_preview",
		"apply_direct",
	]);
	const target = decodeUpdateTarget(action.target, `${path}.target`);
	const mode = decodeUpdateMode(action.mode, `${path}.mode`);
	assertModeMatchesTarget(mode, target, `${path}.mode`);
	if (type === "confirm_preview") {
		integerAt(
			action.expected_object_revision,
			`${path}.expected_object_revision`,
		);
		programmerRevisionAt(
			action.expected_programmer_revision,
			`${path}.expected_programmer_revision`,
		);
	}
	return value as ProgrammingUpdateAction;
}

function decodeProjection(
	value: unknown,
	path: string,
): DecodedProgrammingUpdateProjection {
	const projection = exactRecordAt(value, path, [
		"kind",
		"object_id",
		"object_revision",
		"body",
	]);
	if (!("body" in projection))
		throw new WireValidationError(`${path}.body`, "JSON value", undefined);
	const identity = decodeObjectIdentity(
		{
			kind: projection.kind,
			object_id: projection.object_id,
			object_revision: projection.object_revision,
		},
		path,
	);
	const body = decodeShowObjectBody(
		identity.kind,
		projection.body,
		`${path}.body`,
		identity.object_id,
	);
	return {
		...(value as ProgrammingUpdateProjection),
		body,
	} as DecodedProgrammingUpdateProjection;
}

function decodeSummary(value: unknown, path: string): ProgrammingUpdateSummary {
	const summary = exactRecordAt(value, path, [
		"target",
		"revision_before",
		"revision_after",
		"eligible_count",
		"changed_count",
		"added_count",
		"ignored_count",
		"changed_cues",
		"programmer_values_retained",
	]);
	decodeTargetIdentity(summary.target, `${path}.target`);
	for (const field of [
		"revision_before",
		"revision_after",
		"eligible_count",
		"changed_count",
		"added_count",
		"ignored_count",
	] as const)
		integerAt(summary[field], `${path}.${field}`);
	arrayAt(summary.changed_cues, `${path}.changed_cues`).forEach(
		(cue, index) => {
			decodeCueSource(cue, `${path}.changed_cues[${index}]`);
		},
	);
	booleanAt(
		summary.programmer_values_retained,
		`${path}.programmer_values_retained`,
	);
	return value as ProgrammingUpdateSummary;
}

function decodeSettings(
	value: unknown,
	path: string,
): ProgrammingUpdateSettings {
	const settings = exactRecordAt(value, path, [
		"cue_mode",
		"preset_mode",
		"group_mode",
		"show_update_modal_on_touch",
	]);
	enumAt(settings.cue_mode, `${path}.cue_mode`, CUE_MODES);
	enumAt(settings.preset_mode, `${path}.preset_mode`, EXISTING_MODES);
	enumAt(settings.group_mode, `${path}.group_mode`, EXISTING_MODES);
	booleanAt(
		settings.show_update_modal_on_touch,
		`${path}.show_update_modal_on_touch`,
	);
	return value as ProgrammingUpdateSettings;
}

function assertOutcomeRevisions(
	projection: DecodedProgrammingUpdateProjection,
	summary: ProgrammingUpdateSummary,
	action: ProgrammingUpdateAction,
) {
	if (
		summary.revision_after !== summary.revision_before + 1 ||
		projection.object_revision !== summary.revision_after
	)
		throw new WireValidationError(
			"$.summary.revision_after",
			"one committed object revision matching the projection",
			summary.revision_after,
		);
	if (
		action.type === "confirm_preview" &&
		summary.revision_before !== action.expected_object_revision
	)
		throw new WireValidationError(
			"$.summary.revision_before",
			`revision ${action.expected_object_revision}`,
			summary.revision_before,
		);
}

function assertProjectionMatchesTarget(
	projection: DecodedProgrammingUpdateProjection,
	summary: ProgrammingUpdateSummary,
) {
	assertObjectMatchesTarget(projection, summary.target, "$.projection");
	if (summary.target.family.type !== "cue") return;
	const semanticId = plainStringAt(
		recordAt(projection.body, "$.projection.body").id,
		"$.projection.body.id",
	);
	if (semanticId !== summary.target.object_id)
		throw new WireValidationError(
			"$.projection.body.id",
			`semantic Cuelist ${summary.target.object_id}`,
			semanticId,
		);
}

function actionFields(value: unknown, path: string) {
	const action = exactRecordAt(value, path, [
		"type",
		"target",
		"mode",
		"expected_object_revision",
		"expected_programmer_revision",
	]);
	return action.type === "confirm_preview"
		? [
				"type",
				"target",
				"mode",
				"expected_object_revision",
				"expected_programmer_revision",
			]
		: ["type", "target", "mode"];
}

function optionalFields(
	value: unknown,
	path: string,
	required: string[],
	optional: string[],
) {
	const record = exactRecordAt(value, path, [...required, ...optional]);
	return [...required, ...optional.filter((key) => key in record)];
}

function assertRequestId(value: unknown, expected: string, path: string) {
	const actual = requestIdAt(value, path);
	if (actual !== expected)
		throw new WireValidationError(path, `request ${expected}`, actual);
}

function assertScopeUuid(value: unknown, expected: string, path: string) {
	const actual = scopedUuidAt(value, path);
	if (actual.toLowerCase() !== expected.toLowerCase())
		throw new WireValidationError(path, `scope ${expected}`, actual);
}
